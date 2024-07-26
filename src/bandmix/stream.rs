// use std::time::Duration;

use rodio::{OutputStream, OutputStreamHandle, Sink};
// use stream_download::http::HttpStream;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tracing::debug;

pub struct Player {
    // storage: TempStorageProvider,
    // settings: Settings<HttpStream<::reqwest::Client>>,
    output_stream: OutputStream,
    output_stream_handle: OutputStreamHandle,
    pub sink: Sink,
}

impl Player {
    pub fn new() -> Option<Player> {
        let (_stream, handle) = rodio::OutputStream::try_default().ok()?;
        let sink = rodio::Sink::try_new(&handle).ok()?;

        Some(Player {
            // storage: TempStorageProvider::new(),
            // settings: Settings::default(),
            output_stream: _stream,
            output_stream_handle: handle,
            // decode_buffer: ArrayQueue::new(16),
            sink: sink,
        })
    }

    // TODO: decouple start and decoding of stream
    // async fn get_decoded_url(
    //     url: String,
    // ) -> Option<rodio::Decoder<StreamDownload<TempStorageProvider>>> {
    //     let reader = StreamDownload::new_http(
    //         url.parse().ok()?,
    //         TempStorageProvider::new(),
    //         Settings::default(),
    //     )
    //     .await
    //     .ok()?;
    //     let decode = rodio::Decoder::new(reader).ok()?;
    //     Some(decode)
    // }

    pub async fn start(&self, url: &str) -> Option<()> {
        // let runtime = tokio::runtime::Runtime::new().unwrap();
        // let decode = runtime
        //     .block_on(Self::get_decoded_url(url.to_owned()))
        //     .unwrap();
        // self.sink.append(result);
        let url_string = url.to_string();
        // println!("Reading: {}", url);
        let reader = StreamDownload::new_http(
            url.parse().ok()?,
            TempStorageProvider::new(),
            Settings::default().on_progress(move |_client, stream_state| {
                match stream_state.phase {
                    // stream_download::StreamPhase::Downloading { chunk_size, .. } => {
                    //     std::thread::sleep(Duration::from_millis(20));
                    // }
                    stream_download::StreamPhase::Complete => {
                        debug!("Downloading Complete: {}", url_string);
                    }
                    _ => {}
                };
            }),
        )
        .await
        .ok()?;
        // println!("Decoding: {}", url);
        let decode = rodio::Decoder::new(reader).ok()?;
        // self.sink.pause();
        let _playing = !self.sink.is_paused();
        let empty = self.sink.empty();
        self.sink.append(decode);
        if !empty {
            self.sink.skip_one();
        }
        debug!("New Source Playing: {}", url);
        // self.play();
        Some(())
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
    // pub fn block(&self) {
    //     self.sink.sleep_until_end();
    // }
    pub fn empty(&self) -> bool {
        self.sink.empty()
    }
}
