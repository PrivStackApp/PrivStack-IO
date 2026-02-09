//! Sync pairing system for secure device discovery and approval.
//!
//! This module implements a secure pairing flow:
//! 1. User generates or enters a sync code (e.g., "PEAR-MANGO-KIWI-GRAPE")
//! 2. Only devices with the same sync code can discover each other
//! 3. Discovered devices must be approved before syncing
//! 4. Approved devices become "trusted peers" that auto-sync

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Word list for generating human-readable sync codes.
/// Using common, easy-to-spell words for verbal sharing.
const WORD_LIST: &[&str] = &[
    "APPLE", "BANANA", "CHERRY", "DELTA", "ECHO", "FOXTROT", "GRAPE", "HOTEL",
    "INDIA", "JULIET", "KILO", "LIMA", "MANGO", "NOVEMBER", "OSCAR", "PAPA",
    "QUEBEC", "ROMEO", "SIERRA", "TANGO", "ULTRA", "VICTOR", "WHISKEY", "XRAY",
    "YANKEE", "ZULU", "AMBER", "BRONZE", "CORAL", "DENIM", "EMBER", "FROST",
    "GOLDEN", "HARBOR", "IVORY", "JADE", "KARMA", "LEMON", "MAPLE", "NAVY",
    "OLIVE", "PEARL", "QUARTZ", "RUBY", "SAGE", "TOPAZ", "UNITY", "VELVET",
    "WILLOW", "XENON", "YELLOW", "ZINC", "ARCTIC", "BLAZE", "CLOUD", "DAWN",
    "EAGLE", "FLAME", "GLACIER", "HORIZON", "ISLAND", "JUNGLE", "KNIGHT", "LUNAR",
];

/// A sync code for pairing devices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncCode {
    /// The human-readable code (e.g., "PEAR-MANGO-KIWI-GRAPE")
    pub code: String,
    /// SHA-256 hash of the code, used for DHT namespace
    pub hash: String,
}

impl SyncCode {
    /// Generates a new random sync code with 4 words.
    /// This provides ~24 bits of entropy (64^4 = ~16 million combinations).
    pub fn generate() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let words: Vec<&str> = (0..4)
            .map(|_| {
                let idx = rng.gen_range(0..WORD_LIST.len());
                WORD_LIST[idx]
            })
            .collect();

        let code = words.join("-");
        let hash = Self::hash_code(&code);

        Self { code, hash }
    }

    /// Creates a SyncCode from user input.
    /// Normalizes the input (uppercase, trims whitespace).
    pub fn from_input(input: &str) -> Result<Self, SyncCodeError> {
        let normalized = input
            .trim()
            .to_uppercase()
            .replace(' ', "-")
            .replace('_', "-");

        // Validate format: should be 4 words separated by dashes
        let words: Vec<&str> = normalized.split('-').collect();
        if words.len() != 4 {
            return Err(SyncCodeError::InvalidFormat(
                "Sync code must have exactly 4 words".into()
            ));
        }

        // Validate each word is in our word list (optional, for strict validation)
        // For flexibility, we allow any words but warn if not in list

        let hash = Self::hash_code(&normalized);

        Ok(Self {
            code: normalized,
            hash,
        })
    }

    /// Computes SHA-256 hash of the code for DHT namespace.
    fn hash_code(code: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(code.as_bytes());
        let result = hasher.finalize();
        hex::encode(result)
    }

    /// Returns the DHT namespace key derived from this sync code.
    /// Used to isolate DHT queries to only devices with the same code.
    pub fn dht_namespace(&self) -> Vec<u8> {
        // Use first 32 bytes of hash as DHT key
        hex::decode(&self.hash).unwrap_or_default()
    }
}

/// Errors related to sync codes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncCodeError {
    InvalidFormat(String),
}

impl std::fmt::Display for SyncCodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFormat(msg) => write!(f, "Invalid sync code format: {}", msg),
        }
    }
}

impl std::error::Error for SyncCodeError {}

/// Status of a pairing request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PairingStatus {
    /// Peer discovered, awaiting local approval
    PendingLocalApproval,
    /// We approved, waiting for remote approval
    PendingRemoteApproval,
    /// Both sides approved, fully trusted
    Trusted,
    /// Pairing was rejected
    Rejected,
}

/// Information about a discovered peer during pairing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredPeerInfo {
    /// The peer's libp2p peer ID
    pub peer_id: String,
    /// The peer's device name
    pub device_name: String,
    /// When we discovered this peer
    pub discovered_at: u64,
    /// Current pairing status
    pub status: PairingStatus,
    /// Addresses we can reach this peer at
    pub addresses: Vec<String>,
}

/// A trusted peer that has completed the pairing process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedPeer {
    /// The peer's libp2p peer ID
    pub peer_id: String,
    /// The peer's device name
    pub device_name: String,
    /// When the peer was approved
    pub approved_at: u64,
    /// When we last successfully synced with this peer
    pub last_synced: Option<u64>,
    /// Known addresses for direct connection
    pub addresses: Vec<String>,
}

impl TrustedPeer {
    /// Creates a new trusted peer from a discovered peer.
    pub fn from_discovered(peer: &DiscoveredPeerInfo) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            peer_id: peer.peer_id.clone(),
            device_name: peer.device_name.clone(),
            approved_at: now,
            last_synced: None,
            addresses: peer.addresses.clone(),
        }
    }

    /// Updates the last synced timestamp (epoch milliseconds to match entity modified_at).
    pub fn mark_synced(&mut self) {
        self.last_synced = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
        );
    }
}

/// Manages the pairing state and trusted peers.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PairingManager {
    /// Current sync code (if set)
    current_code: Option<SyncCode>,
    /// Peers discovered with the current sync code
    discovered_peers: HashMap<String, DiscoveredPeerInfo>,
    /// Fully trusted peers (persisted)
    trusted_peers: HashMap<String, TrustedPeer>,
}

impl PairingManager {
    /// Creates a new pairing manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads pairing state from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serializes pairing state to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Gets the current sync code, if any.
    pub fn current_code(&self) -> Option<&SyncCode> {
        self.current_code.as_ref()
    }

    /// Sets a new sync code (clears discovered peers, keeps trusted peers).
    pub fn set_sync_code(&mut self, code: SyncCode) {
        self.current_code = Some(code);
        self.discovered_peers.clear();
    }

    /// Clears the sync code and discovered peers.
    pub fn clear_sync_code(&mut self) {
        self.current_code = None;
        self.discovered_peers.clear();
    }

    /// Adds a discovered peer.
    pub fn add_discovered_peer(&mut self, peer: DiscoveredPeerInfo) {
        // If already trusted, skip
        if self.trusted_peers.contains_key(&peer.peer_id) {
            return;
        }
        self.discovered_peers.insert(peer.peer_id.clone(), peer);
    }

    /// Gets all discovered peers pending approval.
    pub fn discovered_peers(&self) -> Vec<&DiscoveredPeerInfo> {
        self.discovered_peers.values().collect()
    }

    /// Gets a discovered peer by ID.
    pub fn get_discovered_peer(&self, peer_id: &str) -> Option<&DiscoveredPeerInfo> {
        self.discovered_peers.get(peer_id)
    }

    /// Approves a discovered peer, making them trusted.
    pub fn approve_peer(&mut self, peer_id: &str) -> Option<TrustedPeer> {
        if let Some(peer) = self.discovered_peers.remove(peer_id) {
            let trusted = TrustedPeer::from_discovered(&peer);
            self.trusted_peers.insert(peer_id.to_string(), trusted.clone());
            Some(trusted)
        } else {
            None
        }
    }

    /// Rejects a discovered peer.
    pub fn reject_peer(&mut self, peer_id: &str) {
        if let Some(peer) = self.discovered_peers.get_mut(peer_id) {
            peer.status = PairingStatus::Rejected;
        }
    }

    /// Removes a discovered peer.
    pub fn remove_discovered_peer(&mut self, peer_id: &str) {
        self.discovered_peers.remove(peer_id);
    }

    /// Gets all trusted peers.
    pub fn trusted_peers(&self) -> Vec<&TrustedPeer> {
        self.trusted_peers.values().collect()
    }

    /// Gets a trusted peer by ID.
    pub fn get_trusted_peer(&self, peer_id: &str) -> Option<&TrustedPeer> {
        self.trusted_peers.get(peer_id)
    }

    /// Gets a mutable trusted peer by ID.
    pub fn get_trusted_peer_mut(&mut self, peer_id: &str) -> Option<&mut TrustedPeer> {
        self.trusted_peers.get_mut(peer_id)
    }

    /// Checks if a peer is trusted.
    pub fn is_trusted(&self, peer_id: &str) -> bool {
        self.trusted_peers.contains_key(peer_id)
    }

    /// Removes a trusted peer.
    pub fn remove_trusted_peer(&mut self, peer_id: &str) {
        self.trusted_peers.remove(peer_id);
    }

    /// Updates a trusted peer's addresses.
    pub fn update_peer_addresses(&mut self, peer_id: &str, addresses: Vec<String>) {
        if let Some(peer) = self.trusted_peers.get_mut(peer_id) {
            peer.addresses = addresses;
        }
    }

    /// Marks a trusted peer as synced.
    pub fn mark_peer_synced(&mut self, peer_id: &str) {
        if let Some(peer) = self.trusted_peers.get_mut(peer_id) {
            peer.mark_synced();
        }
    }

    /// Updates the device name for a trusted or discovered peer.
    /// Returns true if the name was updated.
    pub fn update_device_name(&mut self, peer_id: &str, name: &str) -> bool {
        let mut updated = false;
        if let Some(peer) = self.trusted_peers.get_mut(peer_id) {
            if peer.device_name != name {
                peer.device_name = name.to_string();
                updated = true;
            }
        }
        if let Some(peer) = self.discovered_peers.get_mut(peer_id) {
            if peer.device_name != name {
                peer.device_name = name.to_string();
                updated = true;
            }
        }
        updated
    }
}

/// Protocol messages for pairing handshake.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PairingMessage {
    /// Announce presence with device info (sent to DHT namespace)
    Announce {
        peer_id: String,
        device_name: String,
        addresses: Vec<String>,
    },
    /// Request pairing with a discovered peer
    PairRequest {
        peer_id: String,
        device_name: String,
    },
    /// Accept a pairing request
    PairAccept {
        peer_id: String,
        device_name: String,
    },
    /// Reject a pairing request
    PairReject {
        peer_id: String,
        reason: Option<String>,
    },
}
