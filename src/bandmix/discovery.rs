use std::{
    collections::{BTreeSet, HashSet},
    sync::{
        atomic::{
            AtomicBool, AtomicUsize,
            Ordering::{Relaxed, SeqCst},
        },
        Arc, Mutex,
    },
    thread::{self, sleep, JoinHandle},
    time::Duration,
};

use crossbeam::queue::ArrayQueue;
use dashmap::DashMap;
use localsavefile::{localsavefile, LocalSaveFilePersistent};
use once_cell::sync::Lazy;
use sharded_slab::Slab;
use tracing::{debug, error, info, trace, warn};

use crate::bandcamp::{
    self,
    api::{DiscoveryType, Format, Function, Genre, RecommendedType},
    models::{Album, AlbumID, Track, TrackID},
};

type AlbumListens = BTreeSet<TrackID>;

static DISCOVERY_STATE: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

static ALBUM_URL_QUEUE: Lazy<ArrayQueue<String>> = Lazy::new(|| ArrayQueue::new(32));
static ALBUM_QUEUE: Lazy<ArrayQueue<AlbumID>> = Lazy::new(|| ArrayQueue::new(4));
static ALBUM_MAP: Lazy<DashMap<AlbumID, Album>> = Lazy::new(DashMap::new);
static ALBUM_LISTENS: Lazy<DashMap<AlbumID, AlbumListens>> = Lazy::new(DashMap::new);

// TODO: make reference to album.track using id's instead of cloning
static MASTER_TRACK_LIST: Lazy<Arc<Slab<(AlbumID, TrackID)>>> =
    Lazy::new(|| Arc::new(Slab::<(AlbumID, TrackID)>::new()));
static FILTERED_TRACK_INDEX: Lazy<Arc<Slab<usize>>> = Lazy::new(|| Arc::new(Slab::<usize>::new()));
static FILTERED_TRACK_INDEX_CAP: Lazy<Arc<AtomicUsize>> =
    Lazy::new(|| Arc::new(AtomicUsize::new(0)));

static THREADS: Lazy<ArrayQueue<JoinHandle<()>>> = Lazy::new(|| ArrayQueue::new(3));

// TODO: Expose cache and only add to it when a song has been 'listened' to
static mut DATA_CACHE: Lazy<Mutex<TrackCache>> =
    Lazy::new(|| Mutex::new(TrackCache::load_default()));
pub static TRACK_CURSOR: AtomicUsize = AtomicUsize::new(0);

#[localsavefile(persist = true, version = 1)]
#[derive(Default)]
struct TrackCache {
    last_cursor: usize,
    track_ids: HashSet<u32>,
    #[savefile_versions = "1.."]
    album_ids: HashSet<u32>,
}

fn album_listened(album: &Album) -> bool {
    let Some(listens) = ALBUM_LISTENS.get(&album.id) else {
        warn!("Failed to get album listen entry");
        return false;
    };
    if listens.len() < album.tracks.len() {
        return false;
    }
    album.tracks.values().all(|t| listens.contains(&t.id))
}

fn add_listened_track(track: &Track) {
    // FIXME: will get_mut cause deadlock issues here?
    if let Some(mut listens) = ALBUM_LISTENS.get_mut(&track.album_id) {
        listens.insert(track.id);
    } else {
        warn!("Failed to get album listen entry for track");
    };
    if let Ok(mut tc) = unsafe { DATA_CACHE.lock() } {
        tc.track_ids.insert(track.id);
        if tc.save().is_err() {
            warn!("Failed to save track cache");
        }
    } else {
        warn!("Failed to lock data cache");
    }
}

fn add_listened_album(album: &Album) {
    if let Ok(mut tc) = unsafe { DATA_CACHE.lock() } {
        tc.album_ids.insert(album.id);
        if tc.save().is_err() {
            warn!("Failed to save album cache");
        }
    } else {
        warn!("Failed to lock data cache");
    }
}

fn filtered_track(track: &Track) -> bool {
    // TODO: genre blacklist
    if let Ok(tc) = unsafe { DATA_CACHE.lock() } {
        tc.track_ids.contains(&track.id)
    } else {
        warn!("Failed to lock data cache");
        false
    }
}

fn filtered_album(album: &Album) -> bool {
    if let Ok(tc) = unsafe { DATA_CACHE.lock() } {
        tc.album_ids.contains(&album.id)
    } else {
        warn!("Failed to lock data cache");
        false
    }
}

fn discovery_load_album_urls_task(function: &Function) -> Option<()> {
    let query = bandcamp::api::DISCOVER_API.build_query(function).ok()?;
    debug!("Obtaining URLs : {}", query);
    let result = bandcamp::api::Api::request(query).ok()?;
    let item = gjson::get(&result, "@this.items.#(type=a)#.url_hints");

    item.each(|_, value: gjson::Value| {
        let subdomain = value.get("subdomain").to_string();
        let slug = value.get("slug").to_string();
        if ALBUM_URL_QUEUE
            .push(format!("https://{}.bandcamp.com/album/{}", subdomain, slug))
            .is_err()
        {
            warn!("Error pushing to Album URL queue");
        }
        if ALBUM_URL_QUEUE.is_full() {
            trace!("Album URL queue full, waiting");
            while ALBUM_URL_QUEUE.is_full() {
                if !DISCOVERY_STATE.load(Relaxed) {
                    break;
                }
                sleep(Duration::from_millis(1000));
            }
        }
        DISCOVERY_STATE.load(Relaxed) // keep iterating
    });
    Some(())
}

fn discovery_load_albums_job() {
    while DISCOVERY_STATE.load(Relaxed) {
        if ALBUM_URL_QUEUE.is_empty() {
            trace!("Album URL Queue empty, waiting");
        }
        while ALBUM_URL_QUEUE.is_empty() {
            if !DISCOVERY_STATE.load(Relaxed) {
                return;
            }
            sleep(Duration::from_millis(100));
        }
        let url = match ALBUM_URL_QUEUE.pop() {
            Some(u) => u,
            None => {
                warn!("Failed to pop album url");
                continue;
            }
        };
        let album = bandcamp::spider::fetch_album(&url);
        if let Some(album) = album {
            trace!("Processing Album : {}", album.name);
            if filtered_album(&album) {
                // TODO: Latently prepend filtered albums onto master track list
                debug!("Filtered Album: {}", album.name);
            } else {
                let id = album.id;
                if ALBUM_MAP.insert(album.id, album).is_some() {
                    warn!("Album collision pushing to map : {}", &id);
                }
                if let Err(error) = ALBUM_QUEUE.push(id) {
                    warn!("Album error pushing to queue : {}", error);
                }
            }
        } else {
            warn!("Failed to fetch Album : {}", url);
        }
        // TODO: How much should we wait?
        if DISCOVERY_STATE.load(Relaxed) && (ALBUM_QUEUE.len() > 1) {
            sleep(Duration::from_millis(2500));
        }
        if ALBUM_QUEUE.is_full() {
            trace!("Album Queue full, waiting");
            while ALBUM_QUEUE.is_full() && DISCOVERY_STATE.load(Relaxed) {
                sleep(Duration::from_millis(500));
            }
        }
    }
    trace!("Stopping albums job");
}

fn discovery_load_tracks_job() {
    while DISCOVERY_STATE.load(Relaxed) {
        if ALBUM_QUEUE.is_empty() {
            trace!("Album Queue empty, waiting");
        }
        while ALBUM_QUEUE.is_empty() {
            if !DISCOVERY_STATE.load(Relaxed) {
                return;
            }
            sleep(Duration::from_millis(100));
        }
        let Some(album) = ALBUM_QUEUE.pop() else {
            warn!("Failed to pop album for tracks");
            continue;
        };
        let Some(album) = ALBUM_MAP.get(&album) else {
            warn!("Failed to get album map for tracks");
            continue;
        };

        trace!("Processing Tracks : {}", album.name);
        let track_count: usize = album.tracks.len();
        let mut filtered_count: usize = 0;

        for track in album.tracks.values() {
            if track.valid() {
                let filtered = filtered_track(track);
                if filtered {
                    filtered_count += 1;
                    debug!("Filtered Track: {}", track.name);
                }
                if let Some(index) = MASTER_TRACK_LIST.insert((album.id, track.id)) {
                    if !filtered {
                        if let Some(i) = FILTERED_TRACK_INDEX.insert(index) {
                            FILTERED_TRACK_INDEX_CAP.store(i, Relaxed);
                        } else {
                            warn!("Failed to insert into filtered track list");
                        }
                    }
                } else {
                    warn!("Failed to insert into master track list");
                }
            }
            if !DISCOVERY_STATE.load(Relaxed) {
                break;
            }
        }

        if track_count == filtered_count {
            debug!("Added filtered Album: {}", album.name);
            add_listened_album(&album);
            info!(
                "Filtered all tracks from {} by {}",
                &album.name, &album.artist
            );
        } else if filtered_count > 0 {
            info!(
                "Filtered {} track{} from {} by {}",
                &filtered_count,
                if filtered_count == 1 { "" } else { "s" },
                &album.name,
                &album.artist
            );
        }

        let cap = FILTERED_TRACK_INDEX_CAP.load(Relaxed);
        let mut cur = TRACK_CURSOR.load(Relaxed);

        // TODO: when should we wait?
        if DISCOVERY_STATE.load(Relaxed) && (cur < cap) && (cap - cur > 8) {
            sleep(Duration::from_millis(2500));
        }

        if (cur < cap) && (cap - cur > 32) {
            trace!("Track List at capacity, waiting");
            while (cur >= cap) || ((cap - cur > 32) && DISCOVERY_STATE.load(Relaxed)) {
                cur = TRACK_CURSOR.load(Relaxed);
                sleep(Duration::from_millis(500));
            }
        }
    }
    trace!("Stopping tracks job");
}

fn discovery_page_urls_job(mut function: Function) {
    let mut page = 0;

    while DISCOVERY_STATE.load(Relaxed) {
        let album_url_task = discovery_load_album_urls_task(&function);
        if album_url_task.is_none() {
            warn!("Album Url Task failed");
        }
        page += 1;
        function.update_get_web_page(page);
    }
    trace!("Stopping urls job");
}

pub fn start(
    genre: Option<Genre>,
    discovery_type: Option<DiscoveryType>,
    format: Option<Format>,
    recommended_type: Option<RecommendedType>,
) {
    if DISCOVERY_STATE.load(SeqCst) {
        error!("Discover already set, ensure no other tasks are running");
    }
    DISCOVERY_STATE.store(true, SeqCst);

    // TODO: option to store cursor position

    let function = Function::get_web(0, genre, discovery_type, format, recommended_type);
    let mut tasks = Vec::new();
    tasks.push(
        thread::Builder::new()
            .name("Discovery load Albums".to_string())
            .spawn(discovery_load_albums_job),
    );
    tasks.push(
        thread::Builder::new()
            .name("Discovery load Tracks".to_string())
            .spawn(discovery_load_tracks_job),
    );
    tasks.push(
        thread::Builder::new()
            .name("Discovery load URLs".to_string())
            .spawn(move || discovery_page_urls_job(function)),
    );

    let mut error = false;
    for task in tasks {
        if let Ok(task) = task {
            error |= THREADS.push(task).is_err();
        } else {
            error = true;
        }
    }

    if error {
        error!("Failed to push threads, stopping");
        stop();
    }
}

#[derive(Default, PartialEq)]
pub struct Entry {
    pub name: String,
    pub artist: String,
    pub album_name: String,
    pub album_art_url: Option<String>,
    pub url: String,
}

impl std::fmt::Display for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{}\n {} by {}",
            self.name, self.artist, self.album_name
        ))
    }
}

fn get_entry(track_i: usize) -> Option<Entry> {
    // TODO: Handle waiting better
    while FILTERED_TRACK_INDEX_CAP.load(Relaxed) <= track_i {
        sleep(Duration::from_millis(10));
    }

    let track = {
        let track_fi = *FILTERED_TRACK_INDEX.get(track_i)?;
        let ids = MASTER_TRACK_LIST.get(track_fi)?;
        let album = ALBUM_MAP.get(&ids.0)?;
        let track = album.tracks.get(&ids.1)?;
        debug!("Cursor now at : {}", TRACK_CURSOR.load(Relaxed));

        Some(Entry {
            name: track.name.clone(),
            artist: album.artist.clone(),
            album_name: album.name.clone(),
            album_art_url: album.album_art_url.clone(),
            url: track.url.clone(),
        })
    };

    if track.is_none() {
        error!("Failed to get entry {}", track_i);
    }
    track
}

pub fn mark_current_track() -> Option<()> {
    let track_i = TRACK_CURSOR.load(Relaxed);
    let track_fi = *FILTERED_TRACK_INDEX.get(track_i)?;
    let ids = MASTER_TRACK_LIST.get(track_fi)?;
    let album = ALBUM_MAP.get(&ids.0)?;
    let track = album.tracks.get(&ids.1)?;

    add_listened_track(track);

    if album_listened(&album) {
        add_listened_album(&album);
    }
    Some(())
}

// TODO: unmark_current_track

pub fn current() -> Option<Entry> {
    let track = TRACK_CURSOR.load(Relaxed);
    get_entry(track)
}

pub fn next() -> Option<Entry> {
    let track = TRACK_CURSOR.fetch_add(1, Relaxed);
    get_entry(track + 1)
}

pub fn previous() -> Option<Entry> {
    let track = TRACK_CURSOR.load(Relaxed);
    if track > 0 {
        return get_entry(TRACK_CURSOR.fetch_sub(1, Relaxed) - 1);
    }
    get_entry(0)
}

pub fn stop() {
    DISCOVERY_STATE.store(false, SeqCst);
    while !THREADS.is_empty() {
        match THREADS.pop() {
            Some(t) => {
                if t.join().is_err() {
                    warn!("Internal thread error on join");
                }
            }
            None => return,
        }
    }
}
