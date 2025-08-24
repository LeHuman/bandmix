use http_cache_reqwest::{CACacheManager, Cache, CacheMode, HttpCache, HttpCacheOptions};
use localsavefile::{localsavefile, LocalSaveFilePersistent};
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use rodio::cpal::traits::HostTrait;
use rodio::{cpal, OutputStream, Sink};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Once};
use std::{env, time};
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tracing::{debug, error, info, warn};
use url::Url;

#[localsavefile(persist = true, version = 0)]
struct PlayerCache {
    url: String,
    position: time::Duration,
}

pub struct Player {
    stream_handle: OutputStream,
    pub sink: Sink,
    dead: Arc<AtomicBool>,
    last_url: String,
    seek_back: bool,
    cache: PlayerCache,
}

static CACHE_INIT: Once = Once::new();

impl Player {
    pub fn new(seek_back: bool) -> Option<Player> {
        let dead = Arc::new(AtomicBool::new(false));
        let dead_callback = dead.clone();

        let (stream_handle, sink) = Self::default_handle(dead_callback)?;

        Some(Player {
            stream_handle,
            sink,
            dead,
            last_url: String::default(),
            seek_back,
            cache: PlayerCache::load_default(),
        })
    }

    pub fn enable_caching() {
        // IMPROVE: Make the cache be set per reader creation instance
        CACHE_INIT.call_once(|| {
            // IMPROVE: Make the cache be set per reader creation instance
            let temp_path = env::temp_dir().join("bandmix").join("stream");
            debug!("Storing stream cache to {}", temp_path.to_string_lossy());

            // TODO: cancellation token for backend? How to force skip / failure?
            let retry_policy = ExponentialBackoff::builder().build_with_max_retries(32);

            // IMPROVE: Make use of streaming variant of cache
            let middle = Cache(HttpCache {
                mode: CacheMode::Default,
                manager: CACacheManager::new(temp_path, true),
                options: HttpCacheOptions::default(),
            });

            Settings::add_default_middleware(middle);
            Settings::add_default_middleware(RetryTransientMiddleware::new_with_policy(
                retry_policy,
            ));
        });
    }

    fn default_handle(callback: Arc<AtomicBool>) -> Option<(OutputStream, Sink)> {
        let default_device = cpal::default_host()
            .default_output_device()
            .ok_or("No default audio output device is found.")
            .ok()?;

        let stream_handle = rodio::OutputStreamBuilder::from_device(default_device)
            .ok()?
            .with_error_callback(move |err| {
                if let cpal::StreamError::DeviceNotAvailable = err {
                    error!("Device error: {}", err);
                } else {
                    error!("Backend error: {}", err);
                }
                debug!("Triggering restart");
                callback.store(true, Ordering::SeqCst);
            })
            .open_stream_or_fallback()
            .ok()?;

        let mixer = stream_handle.mixer();
        let sink: Sink = rodio::Sink::connect_new(mixer);
        sink.pause();

        Some((stream_handle, sink))
    }

    pub async fn rebuild(&mut self) -> Option<()> {
        self.dead.store(false, Ordering::SeqCst);
        let dead_callback = self.dead.clone();

        let (stream_handle, sink) = Self::default_handle(dead_callback)?;

        let playing = !self.is_paused();
        let position = self.sink.get_pos();

        self.stream_handle = stream_handle;
        self.sink = sink;

        if !self.last_url.is_empty() {
            info!("Restarting audio");
            self.start(&self.last_url.clone()).await?;
        }

        if playing {
            self.play();
        }

        if let Err(e) = self.sink.try_seek(position) {
            warn!("Failed to seek on restart: {}", e);
        } else {
            debug!("Seeked back to {}", position.as_secs_f64());
        }

        Some(())
    }

    // TODO: decouple start and decoding of stream
    pub async fn start(&mut self, url: &str) -> Option<()> {
        let url = Url::parse(url).ok()?;

        let mut url_base = url.clone();
        url_base.set_query(None);

        let url_string = url_base.to_string();

        if self.last_url != url_string {
            self.last_url = url_string.clone();
            debug!("Reading: {}", url);
        }

        let captured_url_string = url.to_string();
        let reader = StreamDownload::new_http_with_middleware(
            url.clone(),
            TempStorageProvider::new(),
            Settings::default().on_progress(move |_client, stream_state, _| {
                if stream_state.phase == stream_download::StreamPhase::Complete {
                    debug!("Downloading Complete: {}", captured_url_string);
                };
            }),
        )
        .await
        .ok()?;

        debug!("Decoding: {}", url);
        let decode = rodio::Decoder::new(reader).ok()?;

        let empty = self.sink.empty();
        self.sink.append(decode);
        if !empty {
            self.sink.skip_one();
        }

        debug!("New Source Playing: {}", url);

        if self.seek_back
            && (self.cache.url == url_string)
            && (self.cache.position != time::Duration::default())
        {
            if self.sink.try_seek(self.cache.position).is_ok() {
                info!(
                    "Seeked back to {}s from cache",
                    self.cache.position.as_secs_f64()
                );
            }
        } else {
            self.cache.url = url_string;
            self.cache.position = time::Duration::default();
            self.save_cache();
        }

        Some(())
    }

    fn save_cache(&mut self) {
        if self.cache.save().is_err() {
            warn!("Failed to save player cache");
        }
    }

    pub fn save_position(&mut self) {
        if self.seek_back && !self.is_paused() {
            self.cache.position = self.sink.get_pos();
            self.save_cache();
        }
    }

    pub fn has_error(&self) -> bool {
        self.dead.load(Ordering::Relaxed)
    }

    pub fn play(&self) {
        self.sink.play()
    }

    pub fn pause(&self) {
        self.sink.pause()
    }

    pub fn stop(&self) {
        self.sink.stop()
    }

    pub fn is_paused(&self) -> bool {
        self.sink.is_paused()
    }

    pub fn empty(&self) -> bool {
        self.sink.empty()
    }
}
