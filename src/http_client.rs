use reqwest::Client;
use serde::de::DeserializeOwned;
use std::time::Duration;

#[derive(Clone)]
pub struct HttpClient {
    client: Client,
}

impl HttpClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, reqwest::Error> {
        self.client.get(url).send().await?.json().await
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}
