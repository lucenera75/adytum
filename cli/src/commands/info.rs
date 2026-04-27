use crate::{manifest, wallet::WalletClient};
use anyhow::{Context, Result};

pub async fn run(bundle_or_name: &str, registry_address: Option<&str>, client: &mut WalletClient) -> Result<()> {
    client.authenticate().await.context("Authentication failed")?;

    // Resolve name → address if needed
    let bundle_address = if bundle_or_name.starts_with("component_") {
        bundle_or_name.to_string()
    } else {
        let registry = registry_address
            .ok_or_else(|| anyhow::anyhow!("--registry is required to resolve a name"))?;
        resolve_name(bundle_or_name, registry, client).await?
    };

    // Dry-run get_manifest to read the public metadata
    let m = manifest::get_manifest_call(&bundle_address);
    let result = client
        .submit_and_wait(&m, Default::default(), 1000)
        .await
        .context("get_manifest call failed")?;

    // The result JSON contains the execution outputs; print them
    if let Some(json_result) = result.get("json_result") {
        println!("{}", serde_json::to_string_pretty(json_result).unwrap_or_default());
    } else {
        println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default());
    }
    Ok(())
}

async fn resolve_name(name: &str, registry: &str, client: &mut WalletClient) -> Result<String> {
    let m = manifest::resolve(registry, name);
    let result = client
        .submit_and_wait(&m, Default::default(), 1000)
        .await
        .with_context(|| format!("Failed to resolve '{name}'"))?;

    WalletClient::extract_component_address(&result)
        .ok_or_else(|| anyhow::anyhow!("Name '{name}' not found in registry"))
}
