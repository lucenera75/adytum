use crate::{bundle::Bundle, manifest, wallet::WalletClient};
use anyhow::{anyhow, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::{collections::HashMap, path::PathBuf};

pub struct DeployArgs {
    pub dist_dir: PathBuf,
    pub name: String,
    pub version: String,
    pub content_type: String,
    pub encrypted: bool,
    pub immutable: bool,
    pub bundle_template: String,
    pub registry_address: Option<String>,
    pub max_fee: u64,
}

pub async fn run(args: DeployArgs, client: &mut WalletClient) -> Result<String> {
    // ── 1. Authenticate ───────────────────────────────────────────────────────
    client.authenticate().await.context("Authentication with wallet daemon failed")?;

    // ── 2. Get owner public key from default account ──────────────────────────
    let (_account_addr, owner_pubkey_hex) = client
        .default_account()
        .await
        .context("Failed to fetch default account")?;
    println!("Owner pubkey : {}", &owner_pubkey_hex[..16]);

    // ── 3. Build ZIP bundle ───────────────────────────────────────────────────
    println!("Zipping      : {}", args.dist_dir.display());
    let bundle = Bundle::from_dir(&args.dist_dir)
        .context("Failed to create ZIP bundle from dist directory")?;
    println!(
        "Bundle       : {} bytes, {} chunks, hash {}",
        bundle.bytes.len(),
        bundle.chunk_count(),
        &bundle.content_hash_hex()[..16],
    );

    // ── 4. Deploy DappBundle component ────────────────────────────────────────
    println!("Deploying    : DappBundle component…");
    let deploy_manifest = if args.encrypted {
        manifest::deploy_encrypted(&args.bundle_template, &args.name, &args.version, &args.content_type)
    } else {
        manifest::deploy_public(&args.bundle_template, &args.name, &args.version, &args.content_type)
    };
    let deploy_vars = manifest::vars(&[("owner_pubkey", &owner_pubkey_hex)]);
    let deploy_result = client
        .submit_and_wait(&deploy_manifest, deploy_vars, args.max_fee)
        .await
        .context("DappBundle deploy transaction failed")?;

    let bundle_address = WalletClient::extract_component_address(&deploy_result)
        .ok_or_else(|| anyhow!(
            "Could not find new component address in deploy result.\nFull result:\n{}",
            serde_json::to_string_pretty(&deploy_result).unwrap_or_default()
        ))?;
    println!("Bundle addr  : {}", bundle_address);

    // ── 5. Upload chunks ──────────────────────────────────────────────────────
    let pb = ProgressBar::new(bundle.chunk_count() as u64);
    pb.set_style(
        ProgressStyle::with_template("Uploading    : [{bar:40}] {pos}/{len} chunks")
            .unwrap()
            .progress_chars("=> "),
    );

    for (i, _chunk) in bundle.chunks.iter().enumerate() {
        let (chunk_manifest, var_name) = manifest::upload_chunk(&bundle_address, i as u64);
        let mut vars = HashMap::new();
        vars.insert(var_name, bundle.chunk_hex(i));
        client
            .submit_and_wait(&chunk_manifest, vars, args.max_fee)
            .await
            .with_context(|| format!("Failed to upload chunk {i}"))?;
        pb.inc(1);
    }
    pb.finish_and_clear();

    // ── 6. Publish ────────────────────────────────────────────────────────────
    println!("Publishing   : sealing bundle (immutable={})…", args.immutable);
    let publish_manifest = manifest::publish(&bundle_address, args.immutable);
    let publish_vars = manifest::vars(&[("content_hash", &bundle.content_hash_hex())]);
    client
        .submit_and_wait(&publish_manifest, publish_vars, args.max_fee)
        .await
        .context("Publish transaction failed")?;

    // ── 7. Register in DappRegistry (optional) ────────────────────────────────
    if let Some(ref registry_addr) = args.registry_address {
        println!("Registering  : '{}' → {}…", args.name, bundle_address);
        // registrant_badge = the public-key NFT address of the owner
        let registrant_badge = pubkey_to_nft_address(&owner_pubkey_hex)?;
        let reg_manifest = manifest::register(registry_addr, &args.name, &bundle_address);
        let reg_vars = manifest::vars(&[("registrant_badge", &registrant_badge)]);
        client
            .submit_and_wait(&reg_manifest, reg_vars, args.max_fee)
            .await
            .context("Registry register transaction failed")?;
        println!("Registered   : ootle://{}", args.name);
    }

    println!("Done         : ootle://component_{}", &bundle_address[10..]);
    Ok(bundle_address)
}

/// Convert a 32-byte hex public key into the canonical NFT address string
/// used by PUBLIC_IDENTITY_RESOURCE.
/// Format: `nft_<PUBLIC_IDENTITY_RESOURCE_HEX>_u256_<pubkey_hex>`
pub fn pubkey_to_nft_address(pubkey_hex: &str) -> Result<String> {
    // PUBLIC_IDENTITY_RESOURCE_ADDRESS is a well-known constant on all Ootle networks.
    // Its hex value matches the one in tari_template_lib_types::constants.
    const PUBLIC_IDENTITY_RESOURCE: &str =
        "resource_0000000000000000000000000000000000000000000000000000000000000001";
    // Pad pubkey hex to 64 chars (32 bytes = U256)
    let padded = format!("{:0>64}", pubkey_hex);
    Ok(format!("nft_{PUBLIC_IDENTITY_RESOURCE}_u256_{padded}"))
}
