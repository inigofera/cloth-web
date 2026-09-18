use reqwest::Client;
use std::env;

#[derive(Clone)]
pub struct SupabaseStorage {
    base_url: String,
    api_key: String,
    bucket: String,
    client: reqwest::Client,
}

impl SupabaseStorage {
    pub fn new() -> Self {
        Self {
            base_url: env::var("SUPABASE_URL").unwrap(),
            api_key: env::var("SUPABASE_SERVICE_ROLE_KEY").unwrap(),
            bucket: env::var("SUPABASE_STORAGE_BUCKET").unwrap(),
            client: Client::new(),
        }
    }

    /// Upload `bytes` to the given object key (e.g. `users/{user_id}/{uuid}-{name}`).
    pub async fn upload_file(&self, key: &str, bytes: Vec<u8>) -> anyhow::Result<String> {
        let url = format!(
            "{}/storage/v1/object/{}/{}",
            self.base_url, self.bucket, key
        );

        let res = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("apikey", &self.api_key)
            .body(bytes)
            .send()
            .await?;

        if res.status().is_success() {
            Ok(url)
        } else {
            Err(anyhow::anyhow!("upload failed: {:?}", res.text().await?))
        }
    }

    /// Public URL for the given object key.
    pub async fn get_public_url(&self, key: &str) -> String {
        format!(
            "{}/storage/v1/object/public/{}/{}",
            self.base_url, self.bucket, key
        )
    }

    pub async fn download_file(&self, path: &str) -> anyhow::Result<(Vec<u8>, String)> {
        let url = format!(
            "{}/storage/v1/object/{}/{}",
            self.base_url, self.bucket, path
        );

        let res = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("apikey", &self.api_key)
            .send()
            .await?;

        if res.status().is_success() {
            let content_type = res
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_string();
            let bytes = res.bytes().await?;
            Ok((bytes.to_vec(), content_type))
        } else {
            Err(anyhow::anyhow!("download failed: {:?}", res.text().await?))
        }
    }
}
