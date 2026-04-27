mod bundle;
mod commands;
mod config;
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
        } => {
            let template = bundle_template.ok_or_else(|| {
                anyhow::anyhow!(
                    "DappBundle template address is required. Pass --bundle-template or set \
                     ADYTUM_BUNDLE_TEMPLATE, or save it with `adytum config --bundle-template <addr>`."
                )
            })?;
            commands::deploy::run(
                commands::deploy::DeployArgs {
                    dist_dir: dist,
                    name,
                    version,
                    content_type,
                    encrypted: private,
                    immutable,
                    bundle_template: template,
                    registry_address: registry,
                    max_fee,
                },
                &mut wallet,
            )
            .await?;
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
        } => {
            let mut cfg = Config::load()?;
            if let Some(u) = daemon_url {
                cfg.daemon_url = Some(u);
            }
            if let Some(t) = bundle_template {
                cfg.bundle_template = Some(t);
            }
            if let Some(r) = registry {
                cfg.registry_address = Some(r);
            }
            cfg.save()?;
            println!("Config saved to {}", Config::path().display());
        },
    }

    Ok(())
}
