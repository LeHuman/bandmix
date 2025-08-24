use std::{env, thread, time};

use anyhow::bail;
use http_cache_reqwest::{CACacheManager, Cache, CacheMode, HttpCache, HttpCacheOptions};
use once_cell::sync::Lazy;
use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use tokio::runtime::Runtime;
use tracing::{error, warn};
use url::Url;

static TOKIO_RUNTIME: Lazy<Runtime> =
    Lazy::new(|| Runtime::new().expect("Failed to create Tokio runtime"));

pub struct HTTPClient {
    host: String,
    client: ClientWithMiddleware,
}

impl HTTPClient {
    pub fn new(host_base_url: &str, app_name: &str, module_name: &str, max_retries: u32) -> Self {
        let temp_path = env::temp_dir().join(app_name).join(module_name);

        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(max_retries);

        let client = ClientBuilder::new(Client::new())
            .with(Cache(HttpCache {
                mode: CacheMode::Default,
                manager: CACacheManager::new(temp_path, true),
                options: HttpCacheOptions::default(),
            }))
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        HTTPClient {
            host: host_base_url.into(),
            client,
        }
    }

    // pub fn has_internet() -> bool {
    //     std::net::TcpStream::connect("1.1.1.1:53").is_ok() // Cloudflare DNS
    // }

    // pub fn host_reachable(host: &str, port: u16) -> bool {
    //     std::net::TcpStream::connect((host, port)).is_ok()
    // }

    // fn host_reachable_http(host: &str) -> bool {
    //     std::net::TcpStream::connect((host, 80)).is_ok()
    // }

    // fn host_reachable_https(host: &str) -> bool {
    //     std::net::TcpStream::connect((host, 443)).is_ok()
    // }

    // pub fn has_connection(&self) -> bool {
    //     let internet = Self::has_internet();
    //     let host = Self::host_reachable_http(&self.host);

    //     if internet && !host {
    //         warn!("Host probably down? Unable to connect to {}", self.host);
    //     }

    //     if !internet && host {
    //         warn!("Can connect to host but not Cloudflare DNS, continuing anyways");
    //     }

    //     if !internet && !host {
    //         warn!(
    //             "No internet? Cannot connect to Cloudflare DNS or {}",
    //             self.host
    //         );
    //     }

    //     host
    // }

    pub fn fetch(&self, url: Url) -> anyhow::Result<Vec<u8>> {
        let url_str = url.to_string();

        let result = TOKIO_RUNTIME.block_on(async move {
            let response = self.client.get(url).send().await;
            if let Err(err) = response.as_ref() {
                error!("{}", err);
            }

            let r = response.ok()?;

            if r.status().is_success() {
                Some((r.status(), Some(r.bytes().await.ok()?.to_vec())))
            } else {
                Some((r.status(), None))
            }
        });

        let Some((status, data_op)) = result else {
            bail!("Failed to get response on request for {url_str}");
        };

        if status.is_success() {
            let Some(data) = data_op else {
                bail!("Failed to get data after successful request for {url_str}");
            };

            Ok(data)
        } else {
            bail!("Failed to fetch html for {url_str}")
        }
    }

    // pub fn fetch_guarded(&self, url: Url, mut retries: u64) -> anyhow::Result<Vec<u8>> {
    //     const MAX_RETRY_TIME: time::Duration = time::Duration::from_secs(10);

    //     let url_str = url.to_string();
    //     let mut retry_time = time::Duration::from_millis(250);

    //     if retries != u64::MAX {
    //         retries += 1;
    //     }

    //     while retries != 0 {
    //         retries -= 1;
    //         let result = self.fetch(url.clone());

    //         match result {
    //             Ok(_) => return result,
    //             Err(_) => {
    //                 if self.has_connection() {
    //                     return result;
    //                 } else {
    //                     retry_time *= 2;
    //                     if retry_time > MAX_RETRY_TIME {
    //                         retry_time = MAX_RETRY_TIME;
    //                     }
    //                     warn!(
    //                         "No internet detected, waiting {}s and retrying",
    //                         retry_time.as_secs_f64()
    //                     );
    //                     thread::sleep(retry_time);
    //                 }
    //             }
    //         }
    //     }

    //     bail!("Failed to get a result for {url_str}");
    // }
}
