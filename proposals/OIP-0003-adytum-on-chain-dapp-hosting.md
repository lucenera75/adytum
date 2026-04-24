# OIP-0003: Adytum — On-Chain Private Dapp Hosting

```
OIP Number: 0003
Title:      Adytum — On-Chain Private Dapp Hosting
Status:     Draft
Author(s):  Roberto Di Fiore <robertodifiore75@gmail.com>
Created:    2026-04-24
Updated:    2026-04-25
```

---

## Abstract

**Adytum** (from the Greek ἄδυτον — the innermost sanctuary of a temple, entered only by the initiated) is a
template-level protocol for hosting decentralised application frontends directly on the Tari Ootle network.
Frontend code — HTML, JavaScript, CSS, and static assets — is encrypted, chunked, and stored as blobs inside a
`DappBundle` component. A companion `DappRegistry` component provides human-readable name resolution. An
**Arachne** browser (an Electron app or browser extension) resolves `ootle://` URLs, verifies access via the
wallet daemon, downloads and integrity-checks the bundle, decrypts it, and executes it in a sandboxed JS
environment.

The critical differentiators from existing decentralised web stacks (IPFS+ENS, Arweave permaweb) are:

- **The frontend code itself is private** — chunks are encrypted at rest; raw substate reads yield only
  ciphertext. No badge, no key, no code.
- **The author can be pseudonymous** — bundles are deployed from one-time unlinked keys with no on-chain
  identity trail.
- **Published bundles are uncensorable** — an immutable flag permanently locks the bundle against retroactive
  takedown by the owner or registry operator.

---

## Motivation

Even when on-chain state is access-controlled, application frontends are typically hosted on centralised
infrastructure (Vercel, AWS, CDN). This breaks the trust model in three ways:

- Developers want proof that the frontend the user is running is exactly the code the developer published —
  not a phishing clone served from a look-alike domain.
- Badge-gated dapps should be fully private: a user who loses their badge should lose access to both the
  on-chain logic and the UI that drives it — not just one or the other.
- Dapp developers want atomic versioning: the frontend version and the contract version are linked in the same
  immutable substate, not managed via two separate systems.
- Developers publishing sensitive tooling or politically contentious dapps need the author identity to be
  pseudonymous and the published code to be resistant to retroactive removal.

Existing solutions fall short:

| Solution | Code is private | Integrity provable | Access gated on-chain | Author pseudonymous | Uncensorable |
|---|:---:|:---:|:---:|:---:|:---:|
| IPFS + ENS | ✗ | ✓ (CID) | ✗ | ✗ | ~ |
| Arweave permaweb | ✗ | ✓ (txid) | ✗ | ~ | ✓ |
| Centralised hosting | ✗ | ✗ | ✗ | ✗ | ✗ |
| **Adytum (public mode)** | ✗ | ✓ | ✓ | ✓ | ✓ |
| **Adytum (private mode)** | ✓ | ✓ | ✓ | ✓ | ✓ |

> **Note on public dapps:** For dapps that do not require code privacy, the hybrid approach of storing blobs
> on IPFS or Arweave and publishing only `(name, version, content_hash, storage_ref)` on-chain is cheaper and
> equally correct. Adytum's on-chain encrypted storage is justified specifically when the frontend code itself
> must be confidential.

---

## Specification

### 3.1 Templates

#### `DappBundle`

Stores a versioned, chunked frontend bundle with optional encryption and access control.

```rust
pub struct DappBundle {
    /// Owner's badge — controls all write operations while not immutable.
    owner_badge: NonFungibleAddress,

    /// Human-readable dapp name (informational; canonical resolution is via DappRegistry).
    name: String,

    /// Semantic version string, e.g. "1.3.0".
    version: String,

    /// Chunked bundle bytes.  For encrypted bundles these are ciphertext chunks.
    /// The full bundle is the concatenation of chunks in index order.
    chunks: Vec<Vec<u8>>,

    /// SHA-256 hash of the full reassembled plaintext bundle.
    /// Arachne verifies this after decryption and before executing any code.
    content_hash: [u8; 32],

    /// MIME type of the entry point, e.g. "text/html".
    content_type: String,

    /// Access rule governing who may call `get_chunk`.
    /// Use `rule!(allow_all)` for a public dapp.
    /// Use `rule!(non_fungible(badge))` for a private dapp.
    /// Ignored for privacy purposes on its own — encryption is what enforces code privacy.
    access_rule: AccessRule,

    /// Per-holder encrypted symmetric key (ECIES over Ristretto255).
    /// Maps badge NonFungibleAddress → ECIES ciphertext of the ChaCha20-Poly1305 bundle key.
    /// Empty for unencrypted (public) bundles.
    access_keys: BTreeMap<NonFungibleAddress, Vec<u8>>,

    /// Encryption algorithm identifier. None for public bundles.
    /// Current value: "chacha20poly1305+ecies-ristretto255"
    encryption: Option<String>,

    /// Whether new chunks may still be uploaded (false once `publish()` is called).
    published: bool,

    /// When true, all mutating methods are permanently disabled.
    /// Set via `make_immutable()` or atomically with `publish(..., immutable: true)`.
    immutable: bool,
}
```

**Constants**

```rust
/// Maximum bytes per chunk.  Sized to stay within substate write limits.
const CHUNK_SIZE_BYTES: usize = 65_536; // 64 KiB

/// Encryption algorithm identifier for the current scheme.
const ENCRYPTION_SCHEME: &str = "chacha20poly1305+ecies-ristretto255";
```

**Methods**

| Method | Access | Description |
|---|---|---|
| `new(owner_badge, name, version, content_type, access_rule, encryption)` | — | Deploy an empty bundle |
| `upload_chunk(index: u64, data: Vec<u8>)` | owner, not immutable | Upload or replace chunk at `index` |
| `grant_access(badge: NonFungibleAddress, encrypted_key: Vec<u8>)` | owner, not immutable | Add an ECIES-encrypted key for a badge holder |
| `revoke_access(badge: NonFungibleAddress)` | owner, not immutable | Remove a holder's encrypted key |
| `publish(content_hash: [u8;32], immutable: bool)` | owner, not immutable | Seal the bundle; optionally lock it permanently |
| `make_immutable()` | owner, not immutable | Permanently disable all mutating methods |
| `get_manifest() -> BundleManifest` | anyone | Returns metadata |
| `get_chunk(index: u64) -> Vec<u8>` | `access_rule` | Returns chunk bytes (ciphertext if encrypted) |
| `set_access_rule(rule: AccessRule)` | owner, not immutable | Update access rule |
| `withdraw_fee(amount: Amount)` | owner | Withdraw accumulated access fees (fee-gated variant) |

> `make_immutable()` and `publish(..., immutable: true)` are one-way operations. Once `immutable` is true,
> `upload_chunk`, `grant_access`, `revoke_access`, `publish`, `make_immutable`, and `set_access_rule` all
> revert. The bundle and its access policy are frozen forever.

**`BundleManifest`** (returned by `get_manifest`, always public):

```rust
pub struct BundleManifest {
    pub name: String,
    pub version: String,
    pub chunk_count: u64,
    pub content_hash: [u8; 32],
    pub content_type: String,
    pub published: bool,
    pub immutable: bool,
    pub encrypted: bool,
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

> The registry is a convenience layer only. Bundles are always accessible via their direct component address
> (`ootle://component_3af8...`), bypassing the registry entirely. A registry operator cannot prevent access
> to a bundle by deregistering its name.

---

### 3.2 Encryption Scheme

For private bundles (`encryption: Some("chacha20poly1305+ecies-ristretto255")`):

**Publishing flow (CLI / Arachne upload tool):**

1. Publisher generates a random 256-bit symmetric key **K**.
2. Publisher encrypts the full ZIP bundle with ChaCha20-Poly1305 using **K** → ciphertext.
3. Ciphertext is split into 64 KiB chunks and uploaded via `upload_chunk`.
4. For each authorised badge holder, publisher derives the holder's Ristretto255 public key and computes
   `ECIES-Encrypt(holder_pubkey, K)` → `encrypted_key_bytes`.
5. Publisher calls `grant_access(badge_nft_addr, encrypted_key_bytes)` for each holder.
6. Publisher calls `publish(sha256(plaintext_zip), immutable)`.

**Loading flow (Arachne):**

1. Arachne downloads all ciphertext chunks and concatenates them.
2. Arachne calls `wallet_daemon.decrypt_bundle_key(bundle_component, user_badge_nft_addr)`.
3. Wallet daemon reads `access_keys[user_badge_nft_addr]` from the substate, performs ECIES decryption
   using the user's private key, and returns **K**. The plaintext key never leaves the daemon process.
4. Arachne decrypts the ciphertext with **K** to recover the plaintext ZIP.
5. Arachne verifies `SHA-256(plaintext_zip) == manifest.content_hash`. Mismatch → hard abort.
6. Arachne extracts the ZIP in memory and executes the entry point.

> **Why encryption and not just access rules?** Ootle substates are replicated across validators and
> accessible via the indexer API. An access rule on `get_chunk()` gates method invocations in transactions
> but does not prevent a direct substate read. Encryption ensures that raw chunk bytes are useless without
> the key, regardless of how they were obtained.

---

### 3.3 Pseudonymous Deployment

Deploying a `DappBundle` requires a transaction signed by an account key. To avoid linking the bundle to an
existing on-chain identity, the CLI supports anonymous deployment mode:

```
adytum deploy ./dist --name my-dapp --anonymous
```

**Flow:**

1. CLI generates a fresh Ristretto keypair (never stored in the user's main wallet).
2. CLI instructs the user's wallet to send a confidential transfer of the exact deployment fee to the fresh
   address, with no memo or on-chain linkage.
3. CLI deploys `DappBundle` and registers in `DappRegistry` using the fresh key.
4. CLI exports the fresh private key to an encrypted local keyfile (`~/.adytum/keys/<bundle_addr>.key`).
   This key is required for future updates. The user's main wallet identity is never linked on-chain.

> For maximum anonymity, the funding transfer should originate from a wallet that is itself not linked to
> the user's real identity (e.g. funded via a mixing service or purchased anonymously).

---

### 3.4 URL Scheme

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

### 3.5 Arachne Browser

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
5. If encrypted:
   └─ wallet daemon decrypts bundle key → K
   └─ Arachne decrypts ciphertext with K
        │
        ▼
6. Verify SHA-256(plaintext) == manifest.content_hash
   └─ mismatch → refuse to execute, show error
        │
        ▼
7. Execute in sandboxed iframe / V8 context
```

**Security model:**

- All downloaded code is executed in a sandboxed iframe with `sandbox="allow-scripts"`.
- The sandbox has no access to the host filesystem or native APIs.
- Network access from the sandboxed code is restricted to the local wallet daemon (`localhost:5100`) and
  explicitly whitelisted Ootle RPC endpoints.
- The content hash is always verified against plaintext after decryption. A mismatch results in a hard abort
  with a visible warning — not a silent failure.
- The wallet daemon handles all signing, key operations, and bundle key decryption. The dapp JS never
  touches private keys or the plaintext bundle key.

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

### 3.6 Bundle Format

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

The ZIP (or its ciphertext for private bundles) is split into 64 KiB chunks for on-chain storage. On
download, chunks are concatenated, optionally decrypted, and the ZIP is extracted in memory (never written
to disk).

---

## Rationale

**Why ZIP?** It is the universal container for web assets, supported by every build tool. The dapp developer's
existing build pipeline (`npm run build`) already produces a dist folder that zips trivially.

**Why 64 KiB chunks?** Sized conservatively below the substate write limit while keeping the number of
transactions needed to upload a typical dapp (< 500 KiB compressed) in single digits.

**Why SHA-256 on-chain?** The content hash serves as an immutable commitment. If the substate is somehow
corrupted or tampered with in transit, the browser detects it before execution. This is stronger than CID-based
integrity (IPFS) because the hash is part of the authoritative on-chain state, not derived from the content
itself. For encrypted bundles the hash is computed over the plaintext, so it also serves as a decryption
integrity check.

**Why encryption and not just access rules?** Ootle access rules gate method calls at the engine level but
do not prevent direct reads of substate data via the indexer. Encryption ensures that chunk bytes are
opaque to anyone without the bundle key, regardless of how they obtained the raw bytes. Access rules remain
useful as a bandwidth gate and for fee-gated variants.

**Why ECIES over Ristretto255?** Ootle already uses Ristretto255 for account keys, so no new key
infrastructure is needed. ECIES over Ristretto255 allows the CLI to derive a recipient's encryption public
key from their existing badge identity without any additional key registration step.

**Why an `immutable` flag?** Without it, a bundle owner can silence a dapp retroactively by calling
`set_access_rule(deny_all)` or changing the access keys. The `immutable` flag is a one-way commitment that
permanently removes this power from the owner. A user who verifies `manifest.immutable == true` knows the
bundle will be accessible as long as the network is live, regardless of the owner's future intentions.

**Why a registry separate from the bundle?** Decouples naming from versioning. The registry entry points at
the latest bundle; old bundle components remain on-chain and accessible for auditing or rollback. Critically,
the registry is not a censorship point — bundles are always reachable via direct component address.

**Why Electron for Arachne?** Node.js gives native access to the OS keychain (for wallet daemon JWT and
bundle key storage), system-level URL scheme registration, and a full Chromium sandbox — without requiring
users to install a browser extension and manage permissions manually. A browser extension variant can follow.

**Public dapps and the hybrid approach:** For dapps that do not require code privacy, publishing only
`(name, version, content_hash, arweave_txid)` on-chain and serving blobs from IPFS or Arweave is cheaper
with equivalent integrity guarantees. Adytum's on-chain encrypted storage is the right choice when the
frontend code itself must be confidential.

---

## Implementation Plan

| Phase | Deliverable | Notes |
|---|---|---|
| 1 | `DappBundle` template + tests | Template crate, unit + integration tests |
| 2 | `DappRegistry` template + tests | Depends on Phase 1 |
| 3 | Encryption scheme + wallet daemon extension | `decrypt_bundle_key` RPC method; ECIES over Ristretto255 |
| 4 | Arachne MVP | Electron, `ootle://` scheme, wallet bridge, sandboxed iframe, decrypt flow |
| 5 | CLI upload tool | `adytum deploy ./dist --name my-dapp [--private] [--anonymous] [--immutable]` |
| 6 | Fee-gated variant | `get_chunk` requires depositing a fee; funds go to bundle owner |
| 7 | Browser extension | Chrome/Firefox extension wrapping the same resolution logic |

---

## Open Questions

1. **Substate size limits** — the exact maximum substate write size needs to be confirmed for the current
   validator implementation. Chunk size may need adjustment.
2. **Storage fees** — should the protocol impose a per-byte storage rent, or is the transaction fee sufficient
   incentive for validators to store blobs?
3. **Cross-network resolution** — how should Arachne handle `ootle://my-dapp` when no `DappRegistry` is
   deployed on the connected network? Should there be a well-known deployment address per network genesis?
4. **Content Security Policy** — what CSP headers should Arachne enforce on the sandboxed iframe to prevent
   dapp code from exfiltrating data?
5. **Key rotation** — if a badge holder's key is compromised, re-encrypting the bundle key requires the
   owner to call `grant_access` for each holder. For large holder sets this is expensive. A group key
   scheme (e.g. re-encryption proxies) could address this at the cost of additional complexity.
6. **Forward secrecy** — the current scheme ties the bundle key to long-lived badge keys. A session-key
   layer (holder requests a short-lived decryption token via the wallet daemon) could add forward secrecy.

---

## References

- [Arweave Permaweb](https://arweave.org)
- [ENS + IPFS dapp hosting](https://docs.ens.domains)
- [Tari Ootle Template Library](../../developer-docs/src/content/docs/guides/)
- [Wallet Daemon JSON-RPC API](../../developer-docs/src/content/docs/guides/)
- [ChaCha20-Poly1305 (RFC 8439)](https://www.rfc-editor.org/rfc/rfc8439)
- [ECIES over Ristretto255](https://docs.rs/ecies-ed25519/latest/ecies_ed25519/)
