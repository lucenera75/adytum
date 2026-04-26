// Copyright 2026 Roberto Di Fiore
// SPDX-License-Identifier: MIT

use tari_template_abi::rust::collections::BTreeMap;
use tari_template_lib::prelude::*;

const CHUNK_SIZE_BYTES: usize = 65_536; // 64 KiB

/// Encryption scheme identifier for ChaCha20-Poly1305 bundle key + ECIES over Ristretto255 per-holder wrapping.
pub const ENCRYPTION_SCHEME: &str = "chacha20poly1305+ecies-ristretto255";

/// Always-public metadata returned by `get_manifest`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BundleManifest {
    pub name: String,
    pub version: String,
    pub chunk_count: u64,
    /// SHA-256 of the plaintext ZIP bundle (verified by Arachne after decryption).
    pub content_hash: [u8; 32],
    pub content_type: String,
    pub published: bool,
    pub immutable: bool,
    /// True when chunks are ChaCha20-Poly1305 ciphertext; false for public (unencrypted) bundles.
    pub encrypted: bool,
}

#[template]
mod dapp_bundle {
    use super::*;

    pub struct DappBundle {
        /// NFT badge that authorises all owner-only operations.
        owner_badge: NonFungibleAddress,
        name: String,
        version: String,
        /// Raw bytes per chunk (ciphertext for encrypted bundles).
        chunks: Vec<Vec<u8>>,
        /// SHA-256 of the fully reassembled plaintext ZIP.
        content_hash: [u8; 32],
        content_type: String,
        /// Access rule enforced on `get_chunk`. Stored here so `get_manifest` can summarise it.
        access_rule: AccessRule,
        /// Per-holder ECIES-encrypted symmetric bundle key. Empty for public bundles.
        /// Key: badge NonFungibleAddress. Value: ECIES ciphertext of the ChaCha20-Poly1305 key.
        access_keys: BTreeMap<NonFungibleAddress, Vec<u8>>,
        /// Some("chacha20poly1305+ecies-ristretto255") for encrypted bundles, None for public.
        encryption: Option<String>,
        /// False until `publish()` is called; upload_chunk is disabled after publish.
        published: bool,
        /// One-way flag — once true all mutating methods permanently revert.
        immutable: bool,
    }

    impl DappBundle {
        /// Deploy an empty bundle. Call `upload_chunk` repeatedly, then `publish`.
        ///
        /// - `access_rule`: `rule!(allow_all)` for a public dapp;
        ///   `rule!(non_fungible(badge))` for a badge-gated dapp.
        /// - `encryption`: `None` for a public bundle;
        ///   `Some(ENCRYPTION_SCHEME)` when chunks will be ciphertext.
        pub fn new(
            owner_badge: NonFungibleAddress,
            name: String,
            version: String,
            content_type: String,
            access_rule: AccessRule,
            encryption: Option<String>,
        ) -> Component<Self> {
            let state = Self {
                owner_badge: owner_badge.clone(),
                name,
                version,
                chunks: Vec::new(),
                content_hash: [0u8; 32],
                content_type,
                access_rule: access_rule.clone(),
                access_keys: BTreeMap::new(),
                encryption,
                published: false,
                immutable: false,
            };
            Component::new(state)
                .with_access_rules(Self::build_access_rules(&owner_badge, &access_rule))
                .create()
        }

        /// Upload (or replace) a single chunk. Chunks may be uploaded in any order before `publish`.
        /// `data` must not exceed CHUNK_SIZE_BYTES.
        pub fn upload_chunk(&mut self, index: u64, data: Vec<u8>) {
            assert!(!self.immutable, "Bundle is immutable");
            assert!(!self.published, "Bundle is already published; upload is disabled");
            assert!(
                data.len() <= CHUNK_SIZE_BYTES,
                "Chunk {} exceeds maximum size of {} bytes",
                index,
                CHUNK_SIZE_BYTES
            );
            let idx = index as usize;
            if idx >= self.chunks.len() {
                self.chunks.resize(idx + 1, Vec::new());
            }
            self.chunks[idx] = data;
        }

        /// Store an ECIES-encrypted bundle key for a badge holder.
        /// `encrypted_key` is `ECIES-Encrypt(holder_ristretto_pubkey, symmetric_key)`.
        pub fn grant_access(&mut self, badge: NonFungibleAddress, encrypted_key: Vec<u8>) {
            assert!(!self.immutable, "Bundle is immutable");
            self.access_keys.insert(badge, encrypted_key);
        }

        /// Remove a holder's encrypted key entry.
        pub fn revoke_access(&mut self, badge: NonFungibleAddress) {
            assert!(!self.immutable, "Bundle is immutable");
            self.access_keys.remove(&badge);
        }

        /// Return the ECIES-encrypted key for a specific badge holder (called by the wallet daemon).
        pub fn get_encrypted_key(&self, badge: NonFungibleAddress) -> Vec<u8> {
            self.access_keys
                .get(&badge)
                .cloned()
                .unwrap_or_else(|| panic!("No key found for badge {}", badge))
        }

        /// Seal the bundle. After this call, `upload_chunk` is permanently disabled.
        /// `content_hash` must be the SHA-256 of the fully reassembled plaintext ZIP.
        /// Set `make_immutable` to true to also lock access rules and key entries permanently.
        pub fn publish(&mut self, content_hash: [u8; 32], make_immutable: bool) {
            assert!(!self.immutable, "Bundle is immutable");
            assert!(!self.published, "Bundle is already published");
            assert!(!self.chunks.is_empty(), "No chunks have been uploaded");
            self.content_hash = content_hash;
            self.published = true;
            if make_immutable {
                self.immutable = true;
            }
        }

        /// Permanently lock the bundle. Once called, all mutating methods are disabled forever.
        /// A user who verifies `manifest.immutable == true` knows the bundle cannot be censored
        /// by the owner regardless of future intentions.
        pub fn make_immutable(&mut self) {
            assert!(!self.immutable, "Bundle is already immutable");
            self.immutable = true;
        }

        /// Return publicly readable bundle metadata. Never gated by access rules.
        pub fn get_manifest(&self) -> BundleManifest {
            BundleManifest {
                name: self.name.clone(),
                version: self.version.clone(),
                chunk_count: self.chunks.len() as u64,
                content_hash: self.content_hash,
                content_type: self.content_type.clone(),
                published: self.published,
                immutable: self.immutable,
                encrypted: self.encryption.is_some(),
            }
        }

        /// Return a single chunk. Gated by `access_rule` (set at deploy time or via `set_access_rule`).
        /// For encrypted bundles the returned bytes are ciphertext — decryption is the caller's responsibility.
        pub fn get_chunk(&self, index: u64) -> Vec<u8> {
            assert!(self.published, "Bundle is not yet published");
            let idx = index as usize;
            assert!(idx < self.chunks.len(), "Chunk index {} out of range (count: {})", idx, self.chunks.len());
            self.chunks[idx].clone()
        }

        /// Update the access rule that gates `get_chunk`. Also updates the enforced component access rules.
        pub fn set_access_rule(&mut self, rule: AccessRule) {
            assert!(!self.immutable, "Bundle is immutable");
            self.access_rule = rule.clone();
            let addr = CallerContext::current_component_address();
            ComponentManager::get(addr).set_access_rules(Self::build_access_rules(&self.owner_badge, &rule));
        }

        // ── helpers ──────────────────────────────────────────────────────────

        fn build_access_rules(owner_badge: &NonFungibleAddress, chunk_rule: &AccessRule) -> AccessRules {
            AccessRules::new()
                .add_method_rule("upload_chunk",   rule!(non_fungible(owner_badge.clone())))
                .add_method_rule("grant_access",   rule!(non_fungible(owner_badge.clone())))
                .add_method_rule("revoke_access",  rule!(non_fungible(owner_badge.clone())))
                .add_method_rule("publish",        rule!(non_fungible(owner_badge.clone())))
                .add_method_rule("make_immutable", rule!(non_fungible(owner_badge.clone())))
                .add_method_rule("set_access_rule",rule!(non_fungible(owner_badge.clone())))
                .add_method_rule("get_chunk",      chunk_rule.clone())
                .add_method_rule("get_encrypted_key", chunk_rule.clone())
                .add_method_rule("get_manifest",   rule!(allow_all))
                .default(rule!(deny_all))
        }
    }
}
