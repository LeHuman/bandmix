use std::{
    collections::HashSet,
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
use tracing::{debug, error, info, warn};

use crate::bandcamp::{
    self,
    api::{DiscoveryType, Format, Function, Genre, RecommendedType},
    models::{Album, Track},
};

static DISCOVERY_STATE: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

static ALBUM_URL_QUEUE: Lazy<ArrayQueue<String>> = Lazy::new(|| ArrayQueue::new(32));
static ALBUM_QUEUE: Lazy<ArrayQueue<Album>> = Lazy::new(|| ArrayQueue::new(4));
static ALBUM_MAP: Lazy<DashMap<u32, Album>> = Lazy::new(|| DashMap::new());

// TODO: make reference to album.track using id's instead of cloning
static MASTER_TRACK_LIST: Lazy<Arc<Slab<Track>>> = Lazy::new(|| Arc::new(Slab::<Track>::new()));
static FILTERED_TRACK_INDEX: Lazy<Arc<Slab<usize>>> = Lazy::new(|| Arc::new(Slab::<usize>::new()));
static FILTERED_TRACK_INDEX_CAP: Lazy<Arc<AtomicUsize>> =
    Lazy::new(|| Arc::new(AtomicUsize::new(0)));

static THREADS: Lazy<ArrayQueue<JoinHandle<()>>> = Lazy::new(|| ArrayQueue::new(3));

// TODO: Expose cache and only add to it when a song has been 'listened' to
static mut TRACK_CACHE: Lazy<Mutex<TrackCache>> =
    Lazy::new(|| Mutex::new(TrackCache::load_default()));

pub static TRACK_CURSOR: AtomicUsize = AtomicUsize::new(0);

#[localsavefile(persist = true)]
#[derive(Default)]
struct TrackCache {
    last_cursor: usize,
    track_ids: HashSet<u32>,
}

fn filtered_track(track: &Track) -> bool {
    // TODO: genre blacklist
    unsafe { TRACK_CACHE.lock().unwrap().track_ids.contains(&track.id) }
}

fn discovery_load_album_urls_task(function: Function) -> Option<()> {
    let query = bandcamp::api::DISCOVER_API.build_query(&function).ok()?;
    info!("Obtaining URLs : {}", query);
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
            debug!("Album URL queue full, waiting");
            while ALBUM_URL_QUEUE.is_full() {
                if !DISCOVERY_STATE.load(Relaxed) {
                    break;
                }
                sleep(Duration::from_millis(100));
            }
        }
        DISCOVERY_STATE.load(Relaxed) // keep iterating
    });
    Some(())
}

fn discovery_load_albums_job() {
    while DISCOVERY_STATE.load(Relaxed) {
        if ALBUM_URL_QUEUE.is_empty() {
            debug!("Album URL Queue empty, waiting");
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
            info!("Processing Album : {}", album.name);
            if let Err(error) = ALBUM_QUEUE.push(album.clone()) {
                warn!("Album error pushing to queue : {}", error); // IMPROVE: Actual logging
            } else {
                ALBUM_MAP.insert(album.id, album);
            }
        } else {
            warn!("Failed to fetch Album : {}", url);
        }
        // TODO: How much should we wait?
        if DISCOVERY_STATE.load(Relaxed) && (ALBUM_QUEUE.len() > 1) {
            sleep(Duration::from_millis(2500));
        }
        if ALBUM_QUEUE.is_full() {
            debug!("Album Queue full, waiting");
            while ALBUM_QUEUE.is_full() && DISCOVERY_STATE.load(Relaxed) {
                sleep(Duration::from_millis(100));
            }
        }
    }
    debug!("Stopping albums job");
}

fn discovery_load_tracks_job() {
    while DISCOVERY_STATE.load(Relaxed) {
        if ALBUM_QUEUE.is_empty() {
            debug!("Album Queue empty, waiting");
        }
        while ALBUM_QUEUE.is_empty() {
            if !DISCOVERY_STATE.load(Relaxed) {
                return;
            }
            sleep(Duration::from_millis(100));
        }
        let album = match ALBUM_QUEUE.pop() {
            Some(a) => a,
            None => {
                warn!("Failed to pop album for tracks");
                continue;
            }
        };

        info!("Processing Tracks : {}", album.name);
        for track in album.tracks {
            if track.valid() {
                let filtered = filtered_track(&track);
                if filtered {
                    info!("    Filtered : {}", track.name);
                }
                match MASTER_TRACK_LIST.insert(track) {
                    Some(index) => {
                        if !filtered {
                            match FILTERED_TRACK_INDEX.insert(index) {
                                Some(i) => {
                                    FILTERED_TRACK_INDEX_CAP.store(i, Relaxed);
                                }
                                None => warn!("Failed to insert into filtered track list"),
                            }
                        }
                    }
                    None => warn!("Failed to insert into master track list"),
                }
            }
            if !DISCOVERY_STATE.load(Relaxed) {
                break;
            }
        }

        let cap = FILTERED_TRACK_INDEX_CAP.load(Relaxed);
        let cur = TRACK_CURSOR.load(Relaxed);

        // TODO: when should we wait?
        if DISCOVERY_STATE.load(Relaxed) && (cur < cap) && (cap - cur > 10) {
            sleep(Duration::from_millis(2500));
        }

        if (cur < cap) && (cap - cur > 32) {
            debug!("Track List at capacity, waiting");
            let cur = TRACK_CURSOR.load(Relaxed);
            while (cur >= cap) || ((cap - cur > 32) && DISCOVERY_STATE.load(Relaxed)) {
                sleep(Duration::from_millis(100));
            }
        }
    }
    debug!("Stopping tracks job");
}

fn discovery_page_urls_job(function: &mut Function) {
    let mut page = 0;

    while DISCOVERY_STATE.load(Relaxed) {
        let album_url_task = discovery_load_album_urls_task(function.to_owned());
        if album_url_task.is_none() {
            warn!("Album Url Task failed");
        }
        page += 1;
        function.update_get_web_page(page);
    }
    debug!("Stopping urls job");
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
    // unsafe {
    //     let tc = TRACK_CACHE.lock().unwrap();
    //     TRACK_CURSOR.store(tc.last_cursor, Relaxed);
    // };

    let mut function = Function::get_web(0, genre, discovery_type, format, recommended_type);

    let album_task = thread::spawn(discovery_load_albums_job);
    let track_task = thread::spawn(discovery_load_tracks_job);
    let url_task = thread::spawn(move || discovery_page_urls_job(&mut function));

    let error = THREADS
        .push(album_task)
        .and(THREADS.push(track_task))
        .and(THREADS.push(url_task));

    if error.is_err() {
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
        let track = *FILTERED_TRACK_INDEX.get(track_i)?;
        let track = MASTER_TRACK_LIST.get(track)?;
        let album = ALBUM_MAP.get(&track.album_id)?;

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

    let track = *FILTERED_TRACK_INDEX.get(track_i)?;
    let track = MASTER_TRACK_LIST.get(track)?;

    unsafe {
        let mut tc = TRACK_CACHE.lock().unwrap();
        tc.track_ids.insert(track.id);
        if tc.save().is_err() {
            warn!("Failed to save track cache");
        }
    };
    Some(())
}

pub fn unmark_current_track() -> Option<()> {
    let track_i = TRACK_CURSOR.load(Relaxed);

    let track = *FILTERED_TRACK_INDEX.get(track_i)?;
    let track = MASTER_TRACK_LIST.get(track)?;

    unsafe {
        let mut tc = TRACK_CACHE.lock().unwrap();
        tc.track_ids.remove(&track.id);
        if tc.save().is_err() {
            warn!("Failed to save track cache");
        }
    };
    Some(())
}

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
                t.join();
            }
            None => return,
        }
    }
}
