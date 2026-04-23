# Adytum

> *From the Greek ἄδυτον — the innermost sanctuary of a temple, entered only by the initiated.*

Adytum is a protocol for hosting decentralised application frontends directly on the [Tari Ootle](https://github.com/tari-project/tari-ootle) network, where the **frontend code itself is access-controlled by on-chain badge rules**.

No badge, no code — not just no interaction with the contract.

## How it works

- **`DappBundle`** — a Tari Ootle template that stores chunked frontend assets (HTML, JS, CSS) with a configurable `AccessRule`. Only badge holders can download the code.
- **`DappRegistry`** — a name registry mapping human-readable names to `DappBundle` component addresses (like DNS for `ootle://` URLs).
- **Arachne** — an Electron browser that resolves `ootle://` URLs, verifies the SHA-256 content hash, and executes the bundle in a sandboxed JS environment. The dapp JS never touches private keys.

## Proposal

The full design is in [proposals/OIP-0003-adytum-on-chain-dapp-hosting.md](proposals/OIP-0003-adytum-on-chain-dapp-hosting.md).

## Roadmap

| Phase | Deliverable |
|---|---|
| 1 | `DappBundle` template + tests |
| 2 | `DappRegistry` template + tests |
| 3 | Arachne MVP (Electron, `ootle://` scheme, wallet bridge) |
| 4 | CLI upload tool (`adytum deploy ./dist --name my-dapp`) |
| 5 | Fee-gated bundle variant |
| 6 | Browser extension (Chrome/Firefox) |

## License

MIT
