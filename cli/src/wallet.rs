use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct WalletClient {
    client: Client,
    url: String,
    token: Option<String>,
    req_id: u64,
}

impl WalletClient {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            url: url.into(),
            token: None,
            req_id: 1,
        }
    }

    /// Obtain a JWT from the wallet daemon. Must be called before any authenticated method.
    pub async fn authenticate(&mut self) -> Result<()> {
        let resp = self
            .call_raw("auth.request", json!({ "credentials": "None" }))
            .await?;
        let token = resp["result"]["token"]
            .as_str()
            .ok_or_else(|| anyhow!("auth.request returned no token"))?
            .to_string();
        self.token = Some(token);
        Ok(())
    }

    /// Return the default account's component address and owner public key (hex).
    pub async fn default_account(&mut self) -> Result<(String, String)> {
        let result = self.call("accounts.get_default", json!({})).await?;
        let addr = result["account"]["component_address"]
            .as_str()
            .ok_or_else(|| anyhow!("No component_address in account response"))?
            .to_string();
        let pubkey = result["account"]["owner_public_key"]
            .as_str()
            .ok_or_else(|| anyhow!("No owner_public_key in account response"))?
            .to_string();
        Ok((addr, pubkey))
    }

    /// Submit a manifest and return the transaction ID.
    pub async fn submit_manifest(
        &mut self,
        manifest: &str,
        variables: HashMap<String, String>,
        max_fee: u64,
    ) -> Result<String> {
        let result = self
            .call("transactions.submit_manifest", json!({
                "manifest":  manifest,
                "variables": variables,
                "max_fee":   max_fee,
                "dry_run":   false,
            }))
            .await?;
        let tx_id = result["transaction_id"]
            .as_str()
            .ok_or_else(|| anyhow!("No transaction_id in submit_manifest response"))?
            .to_string();
        Ok(tx_id)
    }

    /// Wait for a transaction to finalise and return the full result JSON.
    pub async fn wait_result(&mut self, tx_id: &str) -> Result<Value> {
        self.call("transactions.wait_result", json!({
            "transaction_id": tx_id,
            "timeout_secs":   120,
        }))
        .await
    }

    /// Submit a manifest and wait for it to finalise, returning the result.
    pub async fn submit_and_wait(
        &mut self,
        manifest: &str,
        variables: HashMap<String, String>,
        max_fee: u64,
    ) -> Result<Value> {
        let tx_id = self.submit_manifest(manifest, variables, max_fee).await?;
        self.wait_result(&tx_id).await
    }

    /// Extract the first newly-created component address from a wait_result response.
    /// The response includes a `json_result` array; we search for any value that looks
    /// like a component address string.
    pub fn extract_component_address(result: &Value) -> Option<String> {
        Self::search_for_component(result)
    }

    fn search_for_component(v: &Value) -> Option<String> {
        match v {
            Value::String(s) if s.starts_with("component_") => Some(s.clone()),
            Value::Array(arr) => arr.iter().find_map(Self::search_for_component),
            Value::Object(map) => map.values().find_map(Self::search_for_component),
            _ => None,
        }
    }

    // ── internals ─────────────────────────────────────────────────────────────

    async fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let raw = self.call_raw(method, params).await?;
        if let Some(err) = raw.get("error") {
            return Err(anyhow!("RPC error on {}: {}", method, err));
        }
        Ok(raw["result"].clone())
    }

    async fn call_raw(&mut self, method: &str, params: Value) -> Result<Value> {
        self.req_id += 1;
        let mut req = self.client.post(&self.url).json(&json!({
            "jsonrpc": "2.0",
            "id":      self.req_id,
            "method":  method,
            "params":  params,
        }));
        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }
        req.send()
            .await
            .context("Failed to reach wallet daemon")?
            .json::<Value>()
            .await
            .context("Failed to parse wallet daemon response")
    }
}
