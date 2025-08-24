use http_cache_reqwest::{CACacheManager, Cache, CacheMode, HttpCache, HttpCacheOptions};
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use rodio::cpal::traits::HostTrait;
use rodio::{cpal, OutputStream, Sink};
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Once};
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tracing::{debug, error, info, warn};

pub struct Player {
    stream_handle: OutputStream,
    pub sink: Sink,
    dead: Arc<AtomicBool>,
    last_url: String,
}

static CACHE_INIT: Once = Once::new();

impl Player {
    pub fn new() -> Option<Player> {
        let dead = Arc::new(AtomicBool::new(false));
        let dead_callback = dead.clone();

        let (stream_handle, sink) = Self::default_handle(dead_callback)?;

        Some(Player {
            stream_handle,
            sink,
            dead,
            last_url: String::default(),
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
        let url_string = url.to_string();

        if self.last_url != url_string {
            self.last_url = url_string.clone();
            debug!("Reading: {}", url);
        }

        let reader = StreamDownload::new_http_with_middleware(
            url.parse().ok()?,
            TempStorageProvider::new(),
            Settings::default().on_progress(move |_client, stream_state, _| {
                if stream_state.phase == stream_download::StreamPhase::Complete {
                    debug!("Downloading Complete: {}", url_string);
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
        Some(())
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
