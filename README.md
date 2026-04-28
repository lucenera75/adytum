# Adytum

> *From the Greek ἄδυτον — the innermost sanctuary of a temple, entered only by the initiated.*

Adytum is a protocol for hosting decentralised application frontends directly on the [Tari Ootle](https://github.com/tari-project/tari-ootle) network, where the **frontend code itself is access-controlled by on-chain badge rules**.

No badge, no code — not just no interaction with the contract.

## How it works

- **`DappBundle`** — a Tari Ootle template that stores chunked, optionally encrypted frontend assets (HTML, JS, CSS). Access rules gate who can download chunks; encryption ensures raw substate reads return only ciphertext.
- **`DappRegistry`** — a name registry mapping human-readable names to `DappBundle` component addresses (DNS for `ootle://` URLs).
- **`adytum` CLI** — zips your dist directory, uploads it in chunks, and manages the full lifecycle from deploy to registry registration.
- **Arachne** — an Electron browser that resolves `ootle://` URLs, verifies the SHA-256 content hash, decrypts if needed, and executes the bundle in a sandboxed JS environment. The dapp JS never touches private keys.

## Components

| Component | Description | Docs |
|---|---|---|
| `templates/dapp_bundle` | On-chain chunked bundle storage with access control | [OIP-0003](proposals/OIP-0003-adytum-on-chain-dapp-hosting.md) |
| `templates/dapp_registry` | Human-readable name → component address registry | [OIP-0003](proposals/OIP-0003-adytum-on-chain-dapp-hosting.md) |
| `cli` | Deploy and manage bundles from the command line | [cli/README.md](cli/README.md) |
| `arachne` | Electron browser for `ootle://` URLs | [arachne/README.md](arachne/README.md) |

## CLI

### Install

```bash
cargo install --path cli
```

### Prerequisites

A running [Tari Ootle wallet daemon](https://github.com/tari-project/tari-ootle) (default port 5100).

### Configure

Save your network addresses once so you don't repeat them on every command:

```bash
adytum config \
  --daemon-url http://localhost:5100/json_rpc \
  --bundle-template <dapp_bundle_template_address> \
  --registry <dapp_registry_component_address>
```

All values can also be set via environment variables:

| Variable | Description |
|---|---|
| `ADYTUM_DAEMON_URL` | Wallet daemon JSON-RPC URL |
| `ADYTUM_BUNDLE_TEMPLATE` | DappBundle template address |
| `ADYTUM_REGISTRY` | DappRegistry component address |

### Deploy a public dapp

```bash
adytum deploy ./dist --name my-dapp --version 1.0.0
```

This will:
1. Zip the `./dist` directory
2. Compute a SHA-256 integrity hash
3. Deploy a `DappBundle` component on-chain
4. Upload all chunks (64 KiB each) with a progress bar
5. Publish and seal the bundle
6. Register `my-dapp` in the `DappRegistry` (if `--registry` is configured)

Your dapp is then reachable at `ootle://my-dapp`.

### Deploy a private (encrypted) dapp

```bash
adytum deploy ./dist --name my-dapp --private
```

Chunks are stored as ChaCha20-Poly1305 ciphertext. The symmetric bundle key is wrapped per badge holder via ECIES over Ristretto255 and stored on-chain. Only badge holders can decrypt.

### Lock a bundle permanently

```bash
adytum deploy ./dist --name my-dapp --immutable
```

Once published with `--immutable`, no one — not even the owner — can change the access rules or take the bundle offline. Users can verify `manifest.immutable == true` for censorship-resistance guarantees.

### Other commands

```bash
# Resolve a name to its component address
adytum resolve my-dapp

# Show public metadata for a bundle (name, version, hash, immutable flag, …)
adytum info my-dapp
adytum info component_3af8d8c2...

# Register an already-deployed bundle under a name
adytum register component_3af8d8c2... --name my-dapp
```

### Anonymous deployment

To deploy without linking the bundle to your main wallet identity, fund a fresh key via a confidential transfer and pass it separately. See [proposals/OIP-0003-adytum-on-chain-dapp-hosting.md](proposals/OIP-0003-adytum-on-chain-dapp-hosting.md) § 3.3 for the full pseudonymous deployment flow.

## Proposal

The full design spec is in [proposals/OIP-0003-adytum-on-chain-dapp-hosting.md](proposals/OIP-0003-adytum-on-chain-dapp-hosting.md).

## Roadmap

| Phase | Status | Deliverable |
|---|---|---|
| 1 | ✓ | `DappBundle` template |
| 2 | ✓ | `DappRegistry` template |
| 3 | ✓ | `adytum` CLI (deploy, register, resolve, info) |
| 4 | — | Arachne browser (Electron, `ootle://` scheme, wallet bridge) |
| 5 | — | Fee-gated bundle variant |
| 6 | — | Browser extension (Chrome/Firefox) |

## License

MIT
