mod bundle;
mod commands;
mod config;
mod ipfs;
mod manifest;
mod wallet;

use anyhow::Result;
use clap::{Parser, Subcommand};
use config::Config;
use wallet::WalletClient;

#[derive(Parser)]
#[command(name = "adytum", about = "Deploy and manage dapp bundles on Tari Ootle", version)]
struct Cli {
    /// Wallet daemon JSON-RPC URL.
    #[arg(long, env = "ADYTUM_DAEMON_URL", global = true)]
    daemon_url: Option<String>,

    /// DappBundle template address (hex, without 'template_' prefix).
    #[arg(long, env = "ADYTUM_BUNDLE_TEMPLATE", global = true)]
    bundle_template: Option<String>,

    /// DappRegistry component address.
    #[arg(long, env = "ADYTUM_REGISTRY", global = true)]
    registry: Option<String>,

    /// Local IPFS/Kubo API URL (e.g. http://127.0.0.1:5001). Overrides config.
    #[arg(long, env = "ADYTUM_IPFS_API", global = true)]
    ipfs_api: Option<String>,

    /// Pinata JWT for IPFS pinning. Overrides config.
    #[arg(long, env = "ADYTUM_PINATA_JWT", global = true)]
    pinata_jwt: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Deploy a frontend bundle from a dist directory.
    Deploy {
        /// Path to the built frontend directory (e.g. ./dist).
        dist: std::path::PathBuf,

        /// Human-readable dapp name (also used for registry registration if --registry is set).
        #[arg(long)]
        name: String,

        /// Semantic version string.
        #[arg(long, default_value = "1.0.0")]
        version: String,

        /// MIME type of the entry point.
        #[arg(long, default_value = "text/html")]
        content_type: String,

        /// Encrypt the bundle (badge-gated, ChaCha20-Poly1305 + ECIES).
        #[arg(long)]
        private: bool,

        /// Permanently lock the bundle after publishing (cannot be undone).
        #[arg(long)]
        immutable: bool,

        /// Maximum transaction fee in microtari.
        #[arg(long, default_value_t = 10_000)]
        max_fee: u64,

        /// Also publish the bundle ZIP to IPFS (requires --ipfs-api or --pinata-jwt).
        #[arg(long)]
        also_ipfs: bool,

        /// Publish to IPFS only — skip on-chain deployment entirely.
        #[arg(long, conflicts_with = "also_ipfs")]
        ipfs_only: bool,
    },

    /// Register an existing DappBundle component under a name in the registry.
    Register {
        /// Component address of the DappBundle.
        bundle: String,

        /// Name to register under.
        #[arg(long)]
        name: String,

        /// Maximum transaction fee in microtari.
        #[arg(long, default_value_t = 10_000)]
        max_fee: u64,
    },

    /// Resolve a name to a DappBundle component address.
    Resolve {
        /// Name to resolve (e.g. "my-dapp").
        name: String,
    },

    /// Show public metadata for a bundle (by name or component address).
    Info {
        /// Bundle name or component address.
        bundle: String,
    },

    /// Save configuration values to ~/.adytum/config.toml.
    Config {
        #[arg(long)]
        daemon_url: Option<String>,
        #[arg(long)]
        bundle_template: Option<String>,
        #[arg(long)]
        registry: Option<String>,
        /// Local IPFS/Kubo API URL.
        #[arg(long)]
        ipfs_api: Option<String>,
        /// Pinata JWT for IPFS pinning.
        #[arg(long)]
        pinata_jwt: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = Config::load()?;

    let daemon_url = cfg.daemon_url(cli.daemon_url.as_deref());
    let bundle_template = cli
        .bundle_template
        .or_else(|| cfg.bundle_template.clone());
    let registry = cli.registry.or_else(|| cfg.registry_address.clone());

    // Resolve IPFS config: CLI flags > saved config
    let ipfs_api  = cli.ipfs_api.or_else(|| cfg.ipfs_api.clone());
    let pinata_jwt = cli.pinata_jwt.or_else(|| cfg.pinata_jwt.clone());

    let mut wallet = WalletClient::new(&daemon_url);

    match cli.command {
        Command::Deploy {
            dist,
            name,
            version,
            content_type,
            private,
            immutable,
            max_fee,
            also_ipfs,
            ipfs_only,
        } => {
            let ipfs_mode = resolve_ipfs_mode(ipfs_api, pinata_jwt, also_ipfs || ipfs_only)?;

            if !ipfs_only {
                let template = bundle_template.ok_or_else(|| {
                    anyhow::anyhow!(
                        "DappBundle template address is required. Pass --bundle-template or set \
                         ADYTUM_BUNDLE_TEMPLATE, or save it with `adytum config --bundle-template <addr>`."
                    )
                })?;
                commands::deploy::run(
                    commands::deploy::DeployArgs {
                        dist_dir: dist.clone(),
                        name: name.clone(),
                        version: version.clone(),
                        content_type: content_type.clone(),
                        encrypted: private,
                        immutable,
                        bundle_template: template,
                        registry_address: registry,
                        max_fee,
                        ipfs_mode: if also_ipfs { ipfs_mode } else { None },
                    },
                    &mut wallet,
                )
                .await?;
            } else {
                // IPFS-only: just bundle and pin
                let b = bundle::Bundle::from_dir(&dist)?;
                let mode = ipfs_mode.ok_or_else(|| {
                    anyhow::anyhow!(
                        "--ipfs-only requires an IPFS backend. Pass --ipfs-api or --pinata-jwt."
                    )
                })?;
                let client = ipfs::IpfsClient::new(mode);
                let filename = format!("{name}-{version}.zip");
                eprintln!("Pinning bundle to IPFS…");
                let cid = client.pin(b.bytes.clone(), &filename).await?;
                println!("ipfs://{cid}");
                println!("https://ipfs.io/ipfs/{cid}");
            }
        },

        Command::Register { bundle, name, max_fee } => {
            let registry_addr = registry.ok_or_else(|| {
                anyhow::anyhow!("--registry is required for the register command")
            })?;
            commands::register::run(&bundle, &name, &registry_addr, max_fee, &mut wallet).await?;
        },

        Command::Resolve { name } => {
            let registry_addr = registry.ok_or_else(|| {
                anyhow::anyhow!("--registry is required for the resolve command")
            })?;
            commands::resolve::run(&name, &registry_addr, &mut wallet).await?;
        },

        Command::Info { bundle } => {
            commands::info::run(&bundle, registry.as_deref(), &mut wallet).await?;
        },

        Command::Config {
            daemon_url,
            bundle_template,
            registry,
            ipfs_api,
            pinata_jwt,
        } => {
            let mut cfg = Config::load()?;
            if let Some(u) = daemon_url      { cfg.daemon_url        = Some(u); }
            if let Some(t) = bundle_template { cfg.bundle_template   = Some(t); }
            if let Some(r) = registry        { cfg.registry_address  = Some(r); }
            if let Some(a) = ipfs_api        { cfg.ipfs_api          = Some(a); }
            if let Some(j) = pinata_jwt      { cfg.pinata_jwt        = Some(j); }
            cfg.save()?;
            println!("Config saved to {}", Config::path().display());
        },
    }

    Ok(())
}

/// Returns `Some(IpfsMode)` when IPFS is requested, `None` when it isn't,
/// and an error when IPFS is requested but no backend is configured.
fn resolve_ipfs_mode(
    ipfs_api: Option<String>,
    pinata_jwt: Option<String>,
    requested: bool,
) -> Result<Option<ipfs::IpfsMode>> {
    if !requested {
        return Ok(None);
    }
    if let Some(jwt) = pinata_jwt {
        return Ok(Some(ipfs::IpfsMode::Pinata { jwt }));
    }
    if let Some(api_url) = ipfs_api {
        return Ok(Some(ipfs::IpfsMode::Local { api_url }));
    }
    anyhow::bail!(
        "IPFS publishing requires a backend. Pass --ipfs-api <url> (local Kubo node) \
         or --pinata-jwt <token> (Pinata cloud), or save them with `adytum config`."
    );
}
