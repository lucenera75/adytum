# adytum CLI

Command-line tool for deploying and managing dapp bundles on [Tari Ootle](https://github.com/tari-project/tari-ootle) via the **Adytum** protocol.

## Requirements

- Rust 1.75+
- A running [Tari Ootle wallet daemon](https://github.com/tari-project/tari-ootle) (default port 5100)
- `DappBundle` and `DappRegistry` templates deployed on the target network

## Install

```bash
cargo install --path .
```

Or build locally:

```bash
cargo build --release
./target/release/adytum --help
```

## Quick start

### 1. Save config once

```bash
adytum config \
  --daemon-url http://localhost:5100/json_rpc \
  --bundle-template <dapp_bundle_template_address> \
  --registry <dapp_registry_component_address>
```

Config is saved to `~/.adytum/config.toml`. All values can also be supplied as environment variables:

| Variable | Description |
|---|---|
| `ADYTUM_DAEMON_URL` | Wallet daemon JSON-RPC URL |
| `ADYTUM_BUNDLE_TEMPLATE` | DappBundle template address (hex) |
| `ADYTUM_REGISTRY` | DappRegistry component address |
| `ADYTUM_IPFS_API` | Local Kubo/IPFS API URL (e.g. `http://127.0.0.1:5001`) |
| `ADYTUM_PINATA_JWT` | Pinata JWT for cloud IPFS pinning |

To save IPFS settings permanently:

```bash
# Local Kubo node
adytum config --ipfs-api http://127.0.0.1:5001

# Or Pinata
adytum config --pinata-jwt <your-jwt>
```

### 2. Deploy

```bash
adytum deploy ./dist --name my-dapp
```

The CLI will:
1. Zip the `./dist` directory with deflate compression
2. Compute a SHA-256 integrity hash of the ZIP
3. Deploy a `DappBundle` component on-chain
4. Upload all 64 KiB chunks with a progress bar
5. Publish and seal the bundle on-chain
6. Register `my-dapp → <component_address>` in the DappRegistry (if `--registry` is set)

Your dapp is then reachable at `ootle://my-dapp`.

## Commands

### `deploy`

```
adytum deploy <dist> [OPTIONS]
```

| Flag | Default | Description |
|---|---|---|
| `--name <name>` | required | Human-readable dapp name |
| `--version <ver>` | `1.0.0` | Semantic version |
| `--content-type <mime>` | `text/html` | MIME type of the entry point |
| `--private` | off | Encrypt the bundle (badge-gated, ChaCha20-Poly1305 + ECIES) |
| `--immutable` | off | Permanently lock the bundle after publishing |
| `--max-fee <n>` | `10000` | Maximum transaction fee in microtari |
| `--also-ipfs` | off | Also pin the bundle ZIP to IPFS after on-chain deploy |
| `--ipfs-only` | off | Pin to IPFS only — skip on-chain deployment entirely |

#### Public dapp

```bash
adytum deploy ./dist --name my-dapp --version 2.0.0
```

#### Private (encrypted) dapp

Chunks are stored as ChaCha20-Poly1305 ciphertext. The symmetric bundle key is ECIES-wrapped per badge holder and stored on-chain. Only authorised badge holders can decrypt.

```bash
adytum deploy ./dist --name my-dapp --private
```

#### Immutable dapp

Once published with `--immutable`, no one — not even the deployer — can change the access rules or take the bundle offline. Clients can verify `manifest.immutable == true` for censorship-resistance guarantees.

```bash
adytum deploy ./dist --name my-dapp --immutable
```

#### IPFS publishing

Optionally pin the bundle ZIP to IPFS alongside (or instead of) the on-chain deploy. IPFS records no authorship at the protocol level — the CID is purely content-derived.

**Also publish to IPFS** (on-chain + IPFS):

```bash
# Using a local Kubo node
adytum deploy ./dist --name my-dapp --also-ipfs --ipfs-api http://127.0.0.1:5001

# Using Pinata cloud
adytum deploy ./dist --name my-dapp --also-ipfs --pinata-jwt <your-jwt>
```

**IPFS only** (no on-chain transaction):

```bash
adytum deploy ./dist --name my-dapp --ipfs-only --ipfs-api http://127.0.0.1:5001
```

The CID printed by `--ipfs-only` or `--also-ipfs` is a permanent, immutable content address. Pin it on additional nodes or gateways to improve availability.

---

### `register`

Register an already-deployed bundle under a name in the DappRegistry.

```bash
adytum register component_3af8d8c2... --name my-dapp
```

---

### `resolve`

Resolve a name to its `DappBundle` component address.

```bash
adytum resolve my-dapp
# → component_3af8d8c2...
```

---

### `info`

Print the public metadata (`BundleManifest`) for a bundle, addressed by name or directly by component address.

```bash
adytum info my-dapp
adytum info component_3af8d8c2...
```

---

### `config`

Persist configuration values to `~/.adytum/config.toml`.

```bash
adytum config --daemon-url http://localhost:5100/json_rpc
adytum config --bundle-template <addr>
adytum config --registry <addr>
```

---

## Anonymous deployment

To deploy without linking the bundle to your main wallet identity:

1. Generate a fresh keypair
2. Fund it from your main wallet via a confidential transfer (exact deploy fee only)
3. Deploy using the fresh key — no on-chain link to your main identity

The full pseudonymous deployment flow is described in [../proposals/OIP-0003-adytum-on-chain-dapp-hosting.md](../proposals/OIP-0003-adytum-on-chain-dapp-hosting.md) § 3.3.

---

## Bundle format

The CLI zips your entire `dist/` directory. The ZIP should contain at minimum:

```
adytum.json      ← metadata (name, version, entry_point)
index.html       ← entry point
assets/
  main.js
  style.css
  ...
```

`adytum.json` example:

```json
{
  "adytum_version": "1",
  "name": "my-dapp",
  "version": "1.0.0",
  "entry_point": "index.html",
  "wallet_api_version": "1"
}
```

If `adytum.json` is absent, Arachne defaults to `index.html` as the entry point.
