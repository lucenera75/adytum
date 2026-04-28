use anyhow::{bail, Context, Result};
use reqwest::multipart;

const PINATA_API: &str = "https://api.pinata.cloud";

pub enum IpfsMode {
    Local { api_url: String },
    Pinata { jwt: String },
}

pub struct IpfsClient {
    mode: IpfsMode,
    http: reqwest::Client,
}

impl IpfsClient {
    pub fn new(mode: IpfsMode) -> Self {
        Self { mode, http: reqwest::Client::new() }
    }

    /// Upload `data` and return the resulting CID.
    pub async fn pin(&self, data: Vec<u8>, filename: &str) -> Result<String> {
        match &self.mode {
            IpfsMode::Local { api_url } => self.pin_kubo(api_url, data, filename).await,
            IpfsMode::Pinata { jwt }    => self.pin_pinata(jwt, data, filename).await,
        }
    }

    async fn pin_kubo(&self, api_url: &str, data: Vec<u8>, filename: &str) -> Result<String> {
        let url = format!("{}/api/v0/add?pin=true&cid-version=1", api_url.trim_end_matches('/'));
        let part = multipart::Part::bytes(data)
            .file_name(filename.to_string())
            .mime_str("application/octet-stream")?;
        let form = multipart::Form::new().part("file", part);

        let resp = self.http.post(&url).multipart(form).send().await
            .with_context(|| format!("POST {url}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("Kubo API error {status}: {body}");
        }

        // Kubo streams one JSON object per added file; take the last line.
        let text = resp.text().await?;
        let last = text.lines().filter(|l| !l.trim().is_empty()).last()
            .context("Empty response from Kubo")?;
        let json: serde_json::Value = serde_json::from_str(last)
            .context("Invalid JSON from Kubo")?;
        json["Hash"].as_str()
            .map(str::to_string)
            .context("Missing 'Hash' field in Kubo response")
    }

    async fn pin_pinata(&self, jwt: &str, data: Vec<u8>, filename: &str) -> Result<String> {
        let url = format!("{PINATA_API}/pinning/pinFileToIPFS");
        let part = multipart::Part::bytes(data)
            .file_name(filename.to_string())
            .mime_str("application/octet-stream")?;
        let form = multipart::Form::new().part("file", part);

        let resp = self.http.post(&url)
            .bearer_auth(jwt)
            .multipart(form)
            .send()
            .await
            .with_context(|| format!("POST {url}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("Pinata API error {status}: {body}");
        }

        let json: serde_json::Value = resp.json().await.context("Invalid JSON from Pinata")?;
        json["IpfsHash"].as_str()
            .map(str::to_string)
            .context("Missing 'IpfsHash' field in Pinata response")
    }
}
