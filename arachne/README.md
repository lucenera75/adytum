# Arachne

> *The weaver who can read every thread.*

Arachne is the reference browser for the [Adytum](https://github.com/lucenera75/adytum) protocol. It resolves `ootle://` URLs, downloads and integrity-checks dapp bundles from the [Tari Ootle](https://github.com/tari-project/tari-ootle) network, and executes them in a sandboxed JS environment — without ever exposing private keys to dapp code.

## Requirements

- [Node.js](https://nodejs.org) 18+
- A running [Tari Ootle wallet daemon](https://github.com/tari-project/tari-ootle) (default port 5100)

## Install & run

```bash
cd arachne
npm install
npm start
```

## Loading a dapp

Type an `ootle://` address in the address bar and press Enter:

```
my-dapp                      ← resolved via DappRegistry
component_3af8d8c2...        ← direct component address, no registry lookup
my-dapp/dashboard?tab=stats  ← with path and query string
```

The first time you launch Arachne, open **Settings** (⚙ top-right) to configure the wallet daemon URL and DappRegistry address.

## Settings

| Setting | Default | Description |
|---|---|---|
| Wallet daemon URL | `http://localhost:5100/json_rpc` | JSON-RPC endpoint of your wallet daemon |
| DappRegistry address | — | Component address of the deployed DappRegistry |
| Max fee | `10000` | Maximum transaction fee per operation, in microtari |

Settings can also be set via environment variables before launch:

```bash
ADYTUM_DAEMON_URL="http://localhost:5100/json_rpc" \
ADYTUM_REGISTRY="component_abc..." \
npm start
```

## How it works

```
ootle://my-dapp
      │
      ▼
1. Resolve name → ComponentAddress   (DappRegistry.resolve — dry-run tx)
      │
      ▼
2. Fetch manifest                    (DappBundle.get_manifest — always public)
      │
      ▼
3. Access check
   ├─ public  → proceed
   └─ gated   → wallet daemon presents badge proof
      │
      ▼
4. Download all chunks in parallel   (DappBundle.get_chunk)
      │
      ▼
5. Verify SHA-256 == manifest.content_hash
   └─ mismatch → hard abort, error shown
      │
      ▼
6. Unzip in memory, inject ootle SDK shim, rewrite asset paths to data URIs
      │
      ▼
7. Load in sandboxed iframe          (sandbox="allow-scripts")
```

## Wallet bridge

Dapp JS communicates with the wallet daemon exclusively through Arachne's `postMessage` bridge. The dapp never holds a JWT or private key.

```js
// Inside a dapp running in Arachne
const result = await window.ootle.call({
  method: 'transactions.submit_manifest',
  params: { manifest: '...', variables: {} }
});
```

Arachne forwards the call to the wallet daemon, injects authentication, and returns the result. The dapp cannot make any direct network requests — all outbound requests from the iframe are blocked at the session level.

## Security model

| Property | Implementation |
|---|---|
| Code isolation | `sandbox="allow-scripts"` iframe — no `allow-same-origin`, no `allow-forms` |
| No network from dapp | All iframe network requests blocked at Electron session level |
| Integrity | SHA-256 verified against on-chain `content_hash` before any JS executes |
| No key exposure | Wallet daemon handles all signing; dapp JS only sees JSON results |
| Asset containment | All relative `src`/`href` rewritten to inline data URIs before load |

## Build for distribution

```bash
npm run build
```

Produces platform-specific installers in `dist/` via [electron-builder](https://www.electron.build). The `ootle://` URL scheme is registered automatically at install time on macOS, Windows, and Linux.

## Project structure

```
arachne/
  main/
    index.js      Electron main process — window, IPC, protocol handler
    wallet.js     JSON-RPC client for the wallet daemon
    resolver.js   ootle:// URL parsing and name resolution
    bundle.js     chunk download, SHA-256 verify, unzip, HTML patching
  preload/
    index.js      contextBridge — exposes window.arachne.* to renderer
  renderer/
    index.html    browser shell
    renderer.js   address bar, history, postMessage bridge, settings panel
    style.css     dark theme UI
```
