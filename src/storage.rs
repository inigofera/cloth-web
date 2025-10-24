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

    pub async fn upload_file(&self, filename: &str, bytes: Vec<u8>) -> anyhow::Result<String> {
        let url = format!(
            "{}/storage/v1/object/{}/{}",
            self.base_url, self.bucket, filename
        );

        let res = self
            .client
            .post(&url)
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

    pub async fn get_public_url(&self, filename: &str) -> String {
        format!(
            "{}/storage/v1/object/public/{}/{}",
            self.base_url, self.bucket, filename
        )
    }
}
