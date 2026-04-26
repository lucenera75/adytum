// Copyright 2026 Roberto Di Fiore
// SPDX-License-Identifier: MIT

use tari_template_abi::rust::collections::BTreeMap;
use tari_template_lib::prelude::*;

/// A single registry entry mapping a name to a bundle component address.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RegistryEntry {
    /// The DappBundle component address.
    pub bundle: ComponentAddress,
    /// The badge whose holder may update or deregister this entry.
    pub registrant_badge: NonFungibleAddress,
    /// Ootle epoch at registration time.
    pub registered_at: u64,
}

#[template]
mod dapp_registry {
    use super::*;

    /// Global name registry mapping human-readable names to DappBundle component addresses.
    /// Functions as DNS for `ootle://` URLs.
    ///
    /// **The registry is a convenience layer only.** A DappBundle is always reachable via its
    /// direct component address (`ootle://component_3af8...`), bypassing this registry entirely.
    /// Deregistering a name does not prevent access to the bundle.
    pub struct DappRegistry {
        entries: BTreeMap<String, RegistryEntry>,
    }

    impl DappRegistry {
        /// Deploy a new empty registry.
        pub fn new() -> Component<Self> {
            Component::new(Self {
                entries: BTreeMap::new(),
            })
            .with_access_rules(AccessRules::allow_all())
            .create()
        }

        /// Register a human-readable name pointing at a DappBundle component.
        /// The name must not already be taken.
        /// `registrant_badge` is the NFT address whose holder will be allowed to update or remove
        /// this entry in the future.
        pub fn register(
            &mut self,
            name: String,
            bundle: ComponentAddress,
            registrant_badge: NonFungibleAddress,
        ) {
            assert!(!self.entries.contains_key(&name), "Name '{}' is already registered", name);
            self.entries.insert(name, RegistryEntry {
                bundle,
                registrant_badge,
                registered_at: Consensus::current_epoch(),
            });
        }

        /// Point an existing name at a new bundle. Requires a proof of the registrant badge.
        pub fn update(&mut self, name: String, new_bundle: ComponentAddress, proof: Proof) {
            let entry = self.entries.get_mut(&name)
                .unwrap_or_else(|| panic!("Name '{}' is not registered", name));
            Self::assert_registrant_proof(&proof, &entry.registrant_badge);
            proof.authorize();
            entry.bundle = new_bundle;
        }

        /// Remove a name from the registry. Requires a proof of the registrant badge.
        /// Note: the underlying DappBundle component is unaffected and still accessible
        /// via its direct component address.
        pub fn deregister(&mut self, name: String, proof: Proof) {
            let entry = self.entries.get(&name)
                .unwrap_or_else(|| panic!("Name '{}' is not registered", name));
            Self::assert_registrant_proof(&proof, &entry.registrant_badge);
            proof.authorize();
            self.entries.remove(&name);
        }

        /// Look up the bundle component address for a name.
        pub fn resolve(&self, name: String) -> Option<ComponentAddress> {
            self.entries.get(&name).map(|e| e.bundle)
        }

        /// Return a full registry entry by name.
        pub fn get(&self, name: String) -> Option<RegistryEntry> {
            self.entries.get(&name).cloned()
        }

        /// Enumerate all registered names and their entries.
        pub fn list(&self) -> Vec<(String, RegistryEntry)> {
            self.entries.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        }

        // ── helpers ──────────────────────────────────────────────────────────

        fn assert_registrant_proof(proof: &Proof, registrant_badge: &NonFungibleAddress) {
            assert_eq!(
                proof.resource_address(),
                *registrant_badge.resource_address(),
                "Proof is for the wrong resource"
            );
            let ids = proof.get_non_fungibles();
            assert!(
                ids.contains(registrant_badge.id()),
                "Proof does not contain the registrant badge"
            );
        }
    }
}
