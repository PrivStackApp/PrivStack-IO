//! .ppk package packing, unpacking, and content hashing.

use std::collections::BTreeMap;
use std::io::{Read, Seek, Write};

use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::manifest::PpkManifest;
use crate::signing::{Signature, SigningKey, VerifyingKey};
use crate::PpkError;

const MANIFEST_ENTRY: &str = "manifest.toml";
const WASM_ENTRY: &str = "plugin.wasm";
const ICON_ENTRY: &str = "icon.png";
const README_ENTRY: &str = "README.md";
const SIGNATURE_ENTRY: &str = "signature.bin";
const VIEWS_PREFIX: &str = "views/";

/// A single file entry inside a .ppk package.
#[derive(Debug, Clone)]
pub struct PackageEntry {
    pub name: String,
    pub data: Vec<u8>,
}

/// An opened .ppk package with parsed contents.
pub struct PpkPackage {
    pub manifest: PpkManifest,
    pub wasm: Option<Vec<u8>>,
    pub icon: Option<Vec<u8>>,
    pub readme: Option<Vec<u8>>,
    pub views: BTreeMap<String, Vec<u8>>,
    pub signature: Option<Vec<u8>>,
    /// Raw entries used for deterministic content hashing.
    entries: Vec<PackageEntry>,
}

impl PpkPackage {
    /// Opens and parses a .ppk package from a reader.
    pub fn open<R: Read + Seek>(reader: R) -> Result<Self, PpkError> {
        let mut archive = ZipArchive::new(reader)?;
        let mut manifest_bytes = None;
        let mut wasm = None;
        let mut icon = None;
        let mut readme = None;
        let mut signature = None;
        let mut views = BTreeMap::new();
        let mut entries = Vec::new();

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();
            let mut data = Vec::new();
            file.read_to_end(&mut data)?;

            entries.push(PackageEntry {
                name: name.clone(),
                data: data.clone(),
            });

            match name.as_str() {
                MANIFEST_ENTRY => manifest_bytes = Some(data),
                WASM_ENTRY => wasm = Some(data),
                ICON_ENTRY => icon = Some(data),
                README_ENTRY => readme = Some(data),
                SIGNATURE_ENTRY => signature = Some(data),
                n if n.starts_with(VIEWS_PREFIX) => {
                    let view_name = n.strip_prefix(VIEWS_PREFIX).unwrap().to_string();
                    if !view_name.is_empty() {
                        views.insert(view_name, data);
                    }
                }
                _ => {}
            }
        }

        let manifest_bytes =
            manifest_bytes.ok_or_else(|| PpkError::MissingEntry(MANIFEST_ENTRY.into()))?;
        let manifest: PpkManifest = toml::from_str(
            std::str::from_utf8(&manifest_bytes)
                .map_err(|e| PpkError::ManifestInvalid(e.to_string()))?,
        )?;

        Ok(Self {
            manifest,
            wasm,
            icon,
            readme,
            views,
            signature,
            entries,
        })
    }

    /// Computes a deterministic SHA-256 content hash over all entries except signature.bin.
    /// Entries are sorted by name to ensure determinism.
    pub fn content_hash(&self) -> String {
        let mut hasher = Sha256::new();
        let mut sorted: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.name != SIGNATURE_ENTRY)
            .collect();
        sorted.sort_by(|a, b| a.name.cmp(&b.name));

        for entry in sorted {
            hasher.update(entry.name.as_bytes());
            hasher.update((entry.data.len() as u64).to_le_bytes());
            hasher.update(&entry.data);
        }

        hex::encode(hasher.finalize())
    }

    /// Verifies the package signature against the content hash.
    pub fn verify(&self, key: &VerifyingKey) -> Result<(), PpkError> {
        let sig_bytes = self.signature.as_ref().ok_or(PpkError::NotSigned)?;
        let sig_array: [u8; 64] = sig_bytes
            .as_slice()
            .try_into()
            .map_err(|_| PpkError::SignatureInvalid)?;
        let signature = Signature::from_bytes(&sig_array);
        let hash = self.content_hash();
        key.verify(hash.as_bytes(), &signature)
    }
}

/// Fluent builder for creating .ppk packages.
pub struct PackageBuilder {
    manifest: PpkManifest,
    wasm: Option<Vec<u8>>,
    icon: Option<Vec<u8>>,
    readme: Option<Vec<u8>>,
    views: BTreeMap<String, Vec<u8>>,
    signing_key: Option<SigningKey>,
}

impl PackageBuilder {
    pub fn new(manifest: PpkManifest) -> Self {
        Self {
            manifest,
            wasm: None,
            icon: None,
            readme: None,
            views: BTreeMap::new(),
            signing_key: None,
        }
    }

    pub fn wasm(mut self, data: Vec<u8>) -> Self {
        self.wasm = Some(data);
        self
    }

    pub fn icon(mut self, data: Vec<u8>) -> Self {
        self.icon = Some(data);
        self
    }

    pub fn readme(mut self, data: Vec<u8>) -> Self {
        self.readme = Some(data);
        self
    }

    pub fn add_view(mut self, name: &str, data: Vec<u8>) -> Self {
        self.views.insert(name.to_string(), data);
        self
    }

    pub fn sign(mut self, key: &SigningKey) -> Self {
        // Re-create key from bytes since SigningKey doesn't implement Clone
        self.signing_key = Some(SigningKey::from_bytes(&key.to_bytes()));
        self
    }

    /// Builds the .ppk zip archive and returns the raw bytes.
    pub fn build(self) -> Result<Vec<u8>, PpkError> {
        let buf = std::io::Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(buf);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // Manifest
        let manifest_toml = toml::to_string_pretty(&self.manifest)?;
        zip.start_file(MANIFEST_ENTRY, options)?;
        zip.write_all(manifest_toml.as_bytes())?;

        // Wasm
        if let Some(ref wasm) = self.wasm {
            zip.start_file(WASM_ENTRY, options)?;
            zip.write_all(wasm)?;
        }

        // Icon
        if let Some(ref icon) = self.icon {
            zip.start_file(ICON_ENTRY, options)?;
            zip.write_all(icon)?;
        }

        // Readme
        if let Some(ref readme) = self.readme {
            zip.start_file(README_ENTRY, options)?;
            zip.write_all(readme)?;
        }

        // Views
        for (name, data) in &self.views {
            let path = format!("{VIEWS_PREFIX}{name}");
            zip.start_file(&path, options)?;
            zip.write_all(data)?;
        }

        // Compute signature if signing key provided
        if let Some(ref signing_key) = self.signing_key {
            // Finish zip without signature to compute content hash
            let finished = zip.finish()?;
            let intermediate_bytes = finished.into_inner();

            // Open intermediate to compute content hash
            let intermediate =
                PpkPackage::open(std::io::Cursor::new(&intermediate_bytes))?;
            let hash = intermediate.content_hash();
            let sig = signing_key.sign(hash.as_bytes());

            // Re-create zip with signature included
            let buf2 = std::io::Cursor::new(Vec::new());
            let mut zip2 = ZipWriter::new(buf2);

            zip2.start_file(MANIFEST_ENTRY, options)?;
            zip2.write_all(manifest_toml.as_bytes())?;

            if let Some(ref wasm) = self.wasm {
                zip2.start_file(WASM_ENTRY, options)?;
                zip2.write_all(wasm)?;
            }
            if let Some(ref icon) = self.icon {
                zip2.start_file(ICON_ENTRY, options)?;
                zip2.write_all(icon)?;
            }
            if let Some(ref readme) = self.readme {
                zip2.start_file(README_ENTRY, options)?;
                zip2.write_all(readme)?;
            }
            for (name, data) in &self.views {
                let path = format!("{VIEWS_PREFIX}{name}");
                zip2.start_file(&path, options)?;
                zip2.write_all(data)?;
            }

            zip2.start_file(SIGNATURE_ENTRY, options)?;
            zip2.write_all(&sig.to_bytes())?;

            let finished2 = zip2.finish()?;
            Ok(finished2.into_inner())
        } else {
            let finished = zip.finish()?;
            Ok(finished.into_inner())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manifest() -> PpkManifest {
        PpkManifest {
            id: "privstack.test".into(),
            name: "Test".into(),
            description: "".into(),
            version: "1.0.0".into(),
            author: "PrivStack".into(),
            icon: None,
            navigation_order: 100,
            category: "utility".into(),
            can_disable: true,
            is_experimental: false,
            min_app_version: None,
            permissions: vec![],
            schemas: vec![],
        }
    }

    #[test]
    fn pack_unpack_minimal() {
        let bytes = PackageBuilder::new(test_manifest())
            .build()
            .unwrap();
        let pkg = PpkPackage::open(std::io::Cursor::new(&bytes)).unwrap();
        assert_eq!(pkg.manifest.id, "privstack.test");
        assert!(pkg.wasm.is_none());
    }

    #[test]
    fn content_hash_excludes_signature() {
        let manifest = test_manifest();
        let unsigned = PackageBuilder::new(manifest.clone())
            .wasm(b"wasm".to_vec())
            .build()
            .unwrap();

        let kp = crate::KeyPair::generate();
        let signed = PackageBuilder::new(manifest)
            .wasm(b"wasm".to_vec())
            .sign(&kp.signing_key)
            .build()
            .unwrap();

        let p1 = PpkPackage::open(std::io::Cursor::new(&unsigned)).unwrap();
        let p2 = PpkPackage::open(std::io::Cursor::new(&signed)).unwrap();
        assert_eq!(p1.content_hash(), p2.content_hash());
        assert!(p2.signature.is_some());
    }
}
