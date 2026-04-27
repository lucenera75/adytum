use crate::{manifest, wallet::WalletClient};
use anyhow::{Context, Result};
use crate::commands::deploy::pubkey_to_nft_address;

pub async fn run(
    bundle_address: &str,
    name: &str,
    registry_address: &str,
    max_fee: u64,
    client: &mut WalletClient,
) -> Result<()> {
    client.authenticate().await.context("Authentication failed")?;

    let (_account_addr, owner_pubkey_hex) = client.default_account().await?;
    let registrant_badge = pubkey_to_nft_address(&owner_pubkey_hex)?;

    let m = manifest::register(registry_address, name, bundle_address);
    let vars = manifest::vars(&[("registrant_badge", &registrant_badge)]);

    client
        .submit_and_wait(&m, vars, max_fee)
        .await
        .context("Registry register transaction failed")?;

    println!("Registered: ootle://{name} → {bundle_address}");
    Ok(())
}
