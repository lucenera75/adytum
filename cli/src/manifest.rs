use std::collections::HashMap;

/// Manifest to deploy a new public DappBundle component.
/// Variables required: `owner_pubkey` (32-byte hex).
pub fn deploy_public(template_address: &str, name: &str, version: &str, content_type: &str) -> String {
    format!(
        r#"use template_{template} as DappBundle;
let bundle = DappBundle::new_public(
    var!("owner_pubkey"),
    "{name}",
    "{version}",
    "{content_type}"
);"#,
        template = template_address,
        name = escape(name),
        version = escape(version),
        content_type = escape(content_type),
    )
}

/// Manifest to deploy a new encrypted DappBundle component.
/// Variables required: `owner_pubkey` (32-byte hex).
pub fn deploy_encrypted(template_address: &str, name: &str, version: &str, content_type: &str) -> String {
    format!(
        r#"use template_{template} as DappBundle;
let bundle = DappBundle::new_encrypted(
    var!("owner_pubkey"),
    "{name}",
    "{version}",
    "{content_type}"
);"#,
        template = template_address,
        name = escape(name),
        version = escape(version),
        content_type = escape(content_type),
    )
}

/// Manifest to upload one chunk.
/// Variables required: `chunk_N` (hex bytes).
pub fn upload_chunk(bundle_address: &str, index: u64) -> (String, String) {
    let var_name = format!("chunk_{index}");
    let manifest = format!(
        r#"let bundle = global!("{bundle_address}");
bundle.upload_chunk({index}u64, var!("{var_name}"));"#
    );
    (manifest, var_name)
}

/// Manifest to publish (seal) the bundle.
/// Variables required: `content_hash` (32-byte hex).
pub fn publish(bundle_address: &str, immutable: bool) -> String {
    format!(
        r#"let bundle = global!("{bundle_address}");
bundle.publish(var!("content_hash"), {immutable});"#
    )
}

/// Manifest to register the bundle in a DappRegistry.
/// Variables required: `registrant_badge` (nft address string).
pub fn register(registry_address: &str, name: &str, bundle_address: &str) -> String {
    format!(
        r#"let registry = global!("{registry_address}");
registry.register("{name}", global!("{bundle_address}"), var!("registrant_badge"));"#,
        name = escape(name),
    )
}

/// Manifest to call get_manifest on a bundle (dry-run read).
pub fn get_manifest_call(bundle_address: &str) -> String {
    format!(
        r#"let bundle = global!("{bundle_address}");
let _info = bundle.get_manifest();"#
    )
}

/// Manifest to resolve a name in the registry.
pub fn resolve(registry_address: &str, name: &str) -> String {
    format!(
        r#"let registry = global!("{registry_address}");
let _addr = registry.resolve("{name}");"#,
        name = escape(name),
    )
}

/// Build a variables map with a single entry.
pub fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
