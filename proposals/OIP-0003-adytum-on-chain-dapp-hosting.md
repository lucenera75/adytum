# OIP-0003: Adytum — On-Chain Private Dapp Hosting

```
OIP Number: 0003
Title:      Adytum — On-Chain Private Dapp Hosting
Status:     Draft
Author(s):  Roberto Di Fiore <robertodifiore75@gmail.com>
Created:    2026-04-24
```

---

## Abstract

**Adytum** (from the Greek ἄδυτον — the innermost sanctuary of a temple, entered only by the initiated) is a
template-level protocol for hosting decentralised application frontends directly on the Tari Ootle network.
Frontend code — HTML, JavaScript, CSS, and static assets — is stored as chunked blobs inside a `DappBundle`
component. A companion `DappRegistry` component provides human-readable name resolution. An **Arachne** browser
(an Electron app or browser extension) resolves `ootle://` URLs, verifies access via the wallet daemon, downloads
and integrity-checks the bundle, and executes it in a sandboxed JS environment.

The critical differentiator from existing decentralised web stacks (IPFS+ENS, Arweave permaweb) is that **the
frontend code itself is access-controlled by on-chain badge rules**. No badge, no code — not just no interaction
with the contract.

---

## Motivation

Private and permissioned blockchains have a fundamental gap: even when the on-chain state is private, the
application frontends that expose that state are typically hosted on centralised infrastructure (Vercel, AWS,
internal CDN). This breaks the trust model.

Concretely:

- Developers want proof that the frontend the user is running is exactly the code the developer published —
  not a phishing clone served from a look-alike domain.
- Badge-gated dapps should be fully private: a user who loses their badge should lose access to both the
  on-chain logic and the UI that drives it — not just one or the other.
- Dapp developers want atomic versioning: the frontend version and the contract version are linked in the same
  immutable substate, not managed via two separate systems.

Existing solutions fall short:

| Solution | Code is private | Code integrity provable | Access gated by on-chain rule |
|---|:---:|:---:|:---:|
| IPFS + ENS | ✗ | ✓ (CID) | ✗ |
| Arweave permaweb | ✗ | ✓ (txid) | ✗ |
| Centralised hosting | ✗ | ✗ | ✗ |
| **Adytum** | ✓ | ✓ | ✓ |

---

## Specification

### 3.1 Templates

#### `DappBundle`

Stores a versioned, chunked frontend bundle with optional access control.

```rust
pub struct DappBundle {
    /// Owner's badge — controls all write operations.
    owner_badge: NonFungibleAddress,

    /// Human-readable dapp name (informational; canonical resolution is via DappRegistry).
    name: String,

    /// Semantic version string, e.g. "1.3.0".
    version: String,

    /// Chunked bundle bytes.  Each chunk is at most CHUNK_SIZE_BYTES.
    /// The full bundle is the concatenation of chunks in index order.
    chunks: Vec<Vec<u8>>,

    /// SHA-256 hash of the full reassembled bundle.
    /// Arachne verifies this before executing any code.
    content_hash: [u8; 32],

    /// MIME type of the entry point, e.g. "text/html".
    content_type: String,

    /// Access rule governing who may call `get_chunk`.
    /// Use `rule!(allow_all)` for a public dapp.
    /// Use `rule!(non_fungible(badge))` for a private dapp.
    access_rule: AccessRule,

    /// Whether new chunks may still be uploaded (false once `publish()` is called).
    published: bool,
}
```

**Constants**

```rust
/// Maximum bytes per chunk.  Sized to stay within substate write limits.
const CHUNK_SIZE_BYTES: usize = 65_536; // 64 KiB
```

**Methods**

| Method | Access | Description |
|---|---|---|
| `new(owner_badge, name, version, content_type, access_rule)` | — | Deploy an empty bundle |
| `upload_chunk(index: u64, data: Vec<u8>)` | owner | Upload or replace chunk at `index` |
| `publish(content_hash: [u8;32])` | owner | Seal the bundle; locks further uploads |
| `get_manifest() -> BundleManifest` | anyone | Returns metadata (name, version, chunk_count, content_hash, content_type) |
| `get_chunk(index: u64) -> Vec<u8>` | `access_rule` | Returns chunk bytes |
| `set_access_rule(rule: AccessRule)` | owner | Update access rule (e.g. to open a previously private dapp) |
| `withdraw_fee(amount: Amount)` | owner | Withdraw accumulated access fees (if fee-gated variant is used) |

**`BundleManifest`** (returned by `get_manifest`, always public):

```rust
pub struct BundleManifest {
    pub name: String,
    pub version: String,
    pub chunk_count: u64,
    pub content_hash: [u8; 32],
    pub content_type: String,
    pub published: bool,
    pub access_rule_summary: String, // "public" | "badge-gated" | "fee-gated"
}
```

---

#### `DappRegistry`

A global name registry mapping human-readable names to `DappBundle` component addresses.
Functions as DNS for `ootle://` URLs.

```rust
pub struct DappRegistry {
    /// Maps name → (component_address, registrant_badge)
    entries: BTreeMap<String, RegistryEntry>,
}

pub struct RegistryEntry {
    pub bundle: ComponentAddress,
    pub registrant_badge: NonFungibleAddress,
    pub registered_at: u64, // epoch
}
```

**Methods**

| Method | Access | Description |
|---|---|---|
| `new()` | — | Deploy the registry |
| `register(name, bundle, registrant_badge)` | anyone (name must be free) | Register a name |
| `update(name, new_bundle)` | registrant | Point an existing name at a new bundle |
| `deregister(name)` | registrant | Remove an entry |
| `resolve(name) -> Option<ComponentAddress>` | anyone | Look up a bundle address |
| `list() -> Vec<(String, RegistryEntry)>` | anyone | Enumerate all entries |

---

### 3.2 URL Scheme

```
ootle://<name>[/<path>][?<query>]
```

Examples:
```
ootle://payroll-app
ootle://payroll-app/employees?dept=engineering
ootle://component_3af8d8c2.../          ← direct address, no registry lookup
```

Resolution order:
1. If the host segment is a valid `ComponentAddress`, use it directly.
2. Otherwise, query the well-known `DappRegistry` on the network.

---

### 3.3 Arachne Browser

**Arachne** is the reference client — an Electron desktop app (or browser extension) that implements the
`ootle://` URL scheme.

**Resolution and loading flow:**

```
User types ootle://my-dapp
        │
        ▼
1. DappRegistry.resolve("my-dapp")  → ComponentAddress
        │
        ▼
2. DappBundle.get_manifest()        → BundleManifest (always public)
        │
        ▼
3. Access check
   ├─ public  → proceed
   └─ gated   → wallet daemon presents badge proof
        │
        ▼
4. Download all chunks in parallel
        │
        ▼
5. Reassemble → verify SHA-256 against manifest.content_hash
   └─ mismatch → refuse to execute, show error
        │
        ▼
6. Execute in sandboxed iframe / V8 context
```

**Security model:**

- All downloaded code is executed in a sandboxed iframe with `sandbox="allow-scripts"`.
- The sandbox has no access to the host filesystem or native APIs.
- Network access from the sandboxed code is restricted to the local wallet daemon (`localhost:5100`) and
  explicitly whitelisted Ootle RPC endpoints.
- The content hash is always verified before execution. A hash mismatch results in a hard abort with a visible
  warning — not a silent failure.
- The wallet daemon handles all signing and key operations; the dapp JS never touches private keys.

**Wallet integration:**

The sandboxed dapp communicates with the local wallet daemon via a `postMessage` bridge exposed by Arachne:

```js
// Inside the dapp JS
const result = await ootle.call({
    method: 'transactions.submit_manifest',
    params: { manifest: '...', variables: {} }
});
```

Arachne forwards the call to `http://localhost:5100/json_rpc`, injects authentication, and returns the result.
The dapp never holds a JWT or private key.

---

### 3.4 Bundle Format

The bundle is a standard **ZIP archive** containing:

```
adytum.json          ← manifest (name, version, entry_point)
index.html           ← entry point (or whatever entry_point specifies)
assets/
  main.js
  style.css
  ...
```

`adytum.json`:
```json
{
  "adytum_version": "1",
  "name": "my-dapp",
  "version": "1.0.0",
  "entry_point": "index.html",
  "wallet_api_version": "1"
}
```

The ZIP is split into 64 KiB chunks for on-chain storage. On download, chunks are concatenated and the ZIP is
extracted in memory (never written to disk).

---

## Rationale

**Why ZIP?** It is the universal container for web assets, supported by every build tool. The dapp developer's
existing build pipeline (`npm run build`) already produces a dist folder that zips trivially.

**Why 64 KiB chunks?** Sized conservatively below the substate write limit while keeping the number of
transactions needed to upload a typical dapp (< 500 KiB compressed) in single digits.

**Why SHA-256 on-chain?** The content hash serves as an immutable commitment. If the substate is somehow
corrupted or tampered with in transit, the browser detects it before execution. This is stronger than CID-based
integrity (IPFS) because the hash is part of the authoritative on-chain state, not derived from the content
itself.

**Why a registry separate from the bundle?** Decouples naming from versioning. The registry entry points at the
latest bundle; the old bundle component remains on-chain and accessible for auditing or rollback.

**Why Electron for Arachne?** Node.js gives native access to the OS keychain (for wallet daemon JWT storage),
system-level URL scheme registration, and a full Chromium sandbox — without requiring users to install a browser
extension and manage permissions manually. A browser extension variant can follow.

---

## Implementation Plan

| Phase | Deliverable | Notes |
|---|---|---|
| 1 | `DappBundle` template + tests | Template crate, unit + integration tests |
| 2 | `DappRegistry` template + tests | Depends on Phase 1 |
| 3 | Arachne MVP | Electron, `ootle://` scheme, wallet bridge, sandboxed iframe |
| 4 | CLI upload tool | `adytum deploy ./dist --name my-dapp --network mainnet` |
| 5 | Fee-gated variant | `get_chunk` requires depositing a fee; funds go to bundle owner |
| 6 | Browser extension | Chrome/Firefox extension wrapping the same resolution logic |

---

## Open Questions

1. **Substate size limits** — the exact maximum substate write size needs to be confirmed for the current
   validator implementation. Chunk size may need adjustment.
2. **Storage fees** — should the protocol impose a per-byte storage rent, or is the transaction fee sufficient
   incentive for validators to store blobs?
3. **Encrypted bundles** — for maximum privacy, the bundle itself could be encrypted with the badge holder's
   public key. Key management UX in Arachne needs design work.
4. **Cross-network resolution** — how should Arachne handle `ootle://my-dapp` when no `DappRegistry` is
   deployed on the connected network? Should there be a well-known deployment address per network genesis?
5. **Content Security Policy** — what CSP headers should Arachne enforce on the sandboxed iframe to prevent
   dapp code from exfiltrating data?

---

## References

- [Arweave Permaweb](https://arweave.org)
- [ENS + IPFS dapp hosting](https://docs.ens.domains)
- [Tari Ootle Template Library](../../developer-docs/src/content/docs/guides/)
- [OIP-0001: Indexer Transaction Indexes](../indexer/OIP-0001-indexer-transaction-indexes.md)
- [Wallet Daemon JSON-RPC API](../../developer-docs/src/content/docs/guides/)
