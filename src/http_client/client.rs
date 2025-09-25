use std::env;

use anyhow::bail;
use http_cache_reqwest::{CACacheManager, Cache, CacheMode, HttpCache, HttpCacheOptions};
use once_cell::sync::Lazy;
use reqwest::Client;
use reqwest_leaky_bucket::leaky_bucket::RateLimiter;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use tokio::runtime::Runtime;
use tracing::error;
use url::Url;

static TOKIO_RUNTIME: Lazy<Runtime> =
    Lazy::new(|| Runtime::new().expect("Failed to create Tokio runtime"));

pub struct HTTPClient {
    client: ClientWithMiddleware,
}

impl HTTPClient {
    pub fn new(app_name: &str, module_name: &str, max_retries: u32) -> Self {
        let temp_path = env::temp_dir().join(app_name).join(module_name);

        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(max_retries);

        let limiter = RateLimiter::builder().initial(0).refill(2).build();

        let client = ClientBuilder::new(Client::new())
            .with(Cache(HttpCache {
                mode: CacheMode::Default,
                manager: CACacheManager::new(temp_path, true),
                options: HttpCacheOptions::default(),
            }))
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .with(reqwest_leaky_bucket::rate_limit_all(limiter))
            .build();

        HTTPClient { client }
    }

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
}
