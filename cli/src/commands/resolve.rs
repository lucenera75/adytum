use crate::{manifest, wallet::WalletClient};
use anyhow::{anyhow, Context, Result};

pub async fn run(name: &str, registry_address: &str, client: &mut WalletClient) -> Result<()> {
    client.authenticate().await.context("Authentication failed")?;

    let m = manifest::resolve(registry_address, name);
    let result = client
        .submit_and_wait(&m, Default::default(), 1000)
        .await
        .with_context(|| format!("Failed to resolve '{name}'"))?;

    let addr = WalletClient::extract_component_address(&result)
        .ok_or_else(|| anyhow!("Name '{name}' not found in registry"))?;

    println!("{addr}");
    Ok(())
}
