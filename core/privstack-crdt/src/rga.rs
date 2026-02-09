//! Replicated Growable Array (RGA) for text sequences.
//!
//! A CRDT for ordered sequences that supports insert and delete operations.
//! Uses unique IDs for each element to enable conflict-free merging.
//!
//! Based on "A comprehensive study of Convergent and Commutative Replicated Data Types"
//! (Shapiro et al.) and Yjs/Automerge implementations.
//!
//! Use cases:
//! - Text content in blocks (the characters in a paragraph)
//! - Ordered lists where position matters

use privstack_types::{HybridTimestamp, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Unique identifier for an element in the sequence.
///
/// Combines a timestamp and peer ID for global uniqueness and ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ElementId {
    /// When this element was created.
    pub timestamp: HybridTimestamp,
    /// Which peer created this element.
    pub peer_id: PeerId,
    /// Sequence number for elements created at the same timestamp by the same peer.
    pub seq: u32,
}

impl ElementId {
    /// Creates a new element ID.
    #[must_use]
    pub fn new(timestamp: HybridTimestamp, peer_id: PeerId, seq: u32) -> Self {
        Self {
            timestamp,
            peer_id,
            seq,
        }
    }

    /// The root element ID (used as the "before first" anchor).
    #[must_use]
    pub fn root() -> Self {
        Self {
            timestamp: HybridTimestamp::new(0, 0),
            peer_id: PeerId::from_uuid(uuid::Uuid::nil()),
            seq: 0,
        }
    }

    /// Returns true if this is the root element.
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.timestamp.wall_time() == 0 && self.seq == 0
    }
}

impl fmt::Display for ElementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.timestamp.wall_time(),
            self.peer_id,
            self.seq
        )
    }
}

impl std::str::FromStr for ElementId {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 {
            return Err("invalid ElementId format");
        }

        let wall_time: u64 = parts[0].parse().map_err(|_| "invalid wall_time")?;
        let peer_id = PeerId::parse(parts[1]).map_err(|_| "invalid peer_id")?;
        let seq: u32 = parts[2].parse().map_err(|_| "invalid seq")?;

        Ok(Self {
            timestamp: HybridTimestamp::new(wall_time, 0), // counter lost, but OK for deserialization
            peer_id,
            seq,
        })
    }
}

impl PartialOrd for ElementId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ElementId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // First compare by timestamp
        match self.timestamp.cmp(&other.timestamp) {
            std::cmp::Ordering::Equal => {
                // Then by peer ID
                match self.peer_id.as_uuid().cmp(&other.peer_id.as_uuid()) {
                    std::cmp::Ordering::Equal => {
                        // Finally by sequence number
                        self.seq.cmp(&other.seq)
                    }
                    ord => ord,
                }
            }
            ord => ord,
        }
    }
}

/// An element in the RGA sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Element<T> {
    /// The element's unique ID.
    id: ElementId,
    /// The ID of the element this was inserted after.
    origin: ElementId,
    /// The value (None if deleted/tombstoned).
    value: Option<T>,
}

/// A Replicated Growable Array for sequences.
///
/// Supports insert and delete operations that commute across replicas.
/// Maintains total order of elements even with concurrent operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(
    serialize = "T: Serialize + Clone",
    deserialize = "T: Deserialize<'de> + Clone"
))]
pub struct RGA<T> {
    /// All elements, indexed by ID.
    #[serde(with = "elements_serde")]
    elements: HashMap<ElementId, Element<T>>,
    /// Counter for generating unique IDs.
    seq_counter: u32,
    /// The peer ID for this replica.
    peer_id: PeerId,
    /// Current timestamp.
    timestamp: HybridTimestamp,
}

/// Custom serialization for RGA elements HashMap to use string keys for JSON compatibility.
mod elements_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S, T: Serialize>(
        elements: &HashMap<ElementId, Element<T>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(elements.len()))?;
        for (k, v) in elements {
            map.serialize_entry(&k.to_string(), v)?;
        }
        map.end()
    }

    pub fn deserialize<'de, D, T: Deserialize<'de> + Clone>(
        deserializer: D,
    ) -> Result<HashMap<ElementId, Element<T>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{MapAccess, Visitor};

        struct ElementsVisitor<T>(std::marker::PhantomData<T>);

        impl<'de, T: Deserialize<'de> + Clone> Visitor<'de> for ElementsVisitor<T> {
            type Value = HashMap<ElementId, Element<T>>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a map with string keys")
            }

            fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut map = HashMap::with_capacity(access.size_hint().unwrap_or(0));
                while let Some((key, value)) = access.next_entry::<String, Element<T>>()? {
                    let element_id: ElementId = key.parse().map_err(serde::de::Error::custom)?;
                    map.insert(element_id, value);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_map(ElementsVisitor(std::marker::PhantomData))
    }
}

impl<T: Clone> RGA<T> {
    /// Sets the peer ID for this replica.
    pub fn set_peer_id(&mut self, peer_id: PeerId) {
        self.peer_id = peer_id;
    }

    /// Returns the peer ID for this replica.
    #[must_use]
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// Creates a new empty RGA.
    #[must_use]
    pub fn new(peer_id: PeerId) -> Self {
        let mut rga = Self {
            elements: HashMap::new(),
            seq_counter: 0,
            peer_id,
            timestamp: HybridTimestamp::now(),
        };

        // Insert the root element
        let root = Element {
            id: ElementId::root(),
            origin: ElementId::root(),
            value: None,
        };
        rga.elements.insert(ElementId::root(), root);

        rga
    }

    /// Builds the ordered list of element IDs by traversing the origin graph.
    ///
    /// This is the key to commutativity: the order is computed deterministically
    /// from the element data, not from the order of operations.
    fn build_order(&self) -> Vec<ElementId> {
        // Group elements by their origin
        let mut children: HashMap<ElementId, Vec<ElementId>> = HashMap::new();
        for elem in self.elements.values() {
            if !elem.id.is_root() {
                children.entry(elem.origin).or_default().push(elem.id);
            }
        }

        // Sort children by ID descending (higher ID = inserted later = appears earlier among siblings)
        // This is the standard RGA ordering rule for concurrent inserts
        for siblings in children.values_mut() {
            siblings.sort_by(|a, b| b.cmp(a));
        }

        // DFS traversal from root
        let mut order = Vec::new();
        let mut stack = vec![ElementId::root()];

        while let Some(current) = stack.pop() {
            if !current.is_root() {
                order.push(current);
            }

            // Add children in reverse order (so they come out in correct order from stack)
            if let Some(kids) = children.get(&current) {
                for &child in kids.iter().rev() {
                    stack.push(child);
                }
            }
        }

        order
    }

    /// Returns the number of visible (non-deleted) elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.elements.values().filter(|e| e.value.is_some()).count()
    }

    /// Returns true if the sequence is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the visible elements as a vector.
    #[must_use]
    pub fn to_vec(&self) -> Vec<T> {
        let order = self.build_order();
        order
            .iter()
            .filter_map(|id| {
                self.elements
                    .get(id)
                    .and_then(|e| e.value.as_ref().cloned())
            })
            .collect()
    }

    /// Returns the element at the given visible index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        let order = self.build_order();
        let mut visible_count = 0;
        for id in &order {
            if let Some(elem) = self.elements.get(id) {
                if elem.value.is_some() {
                    if visible_count == index {
                        return elem.value.as_ref();
                    }
                    visible_count += 1;
                }
            }
        }
        None
    }

    /// Generates a new unique element ID.
    fn next_id(&mut self) -> ElementId {
        self.timestamp = self.timestamp.tick();
        self.seq_counter += 1;
        ElementId::new(self.timestamp, self.peer_id, self.seq_counter)
    }

    /// Finds the origin element ID for inserting at a visible index.
    fn find_origin_for_index(&self, index: usize) -> ElementId {
        if index == 0 {
            return ElementId::root();
        }

        let order = self.build_order();
        let mut visible_count = 0;
        for id in &order {
            if let Some(elem) = self.elements.get(id) {
                if elem.value.is_some() {
                    visible_count += 1;
                    if visible_count == index {
                        return *id;
                    }
                }
            }
        }

        // Insert at end - find the last element
        order.last().copied().unwrap_or(ElementId::root())
    }

    /// Inserts a value at the given visible index.
    ///
    /// Returns the ID of the inserted element.
    pub fn insert(&mut self, index: usize, value: T) -> ElementId {
        let origin = self.find_origin_for_index(index);
        let id = self.next_id();

        self.insert_with_id(id, origin, value);
        id
    }

    /// Inserts a value with a specific ID (for replication).
    pub fn insert_with_id(&mut self, id: ElementId, origin: ElementId, value: T) {
        // Update our timestamp if the incoming ID is newer
        if id.timestamp > self.timestamp {
            self.timestamp = id.timestamp;
        }

        let elem = Element {
            id,
            origin,
            value: Some(value),
        };

        self.elements.insert(id, elem);
    }

    /// Finds the element ID at a visible index.
    fn find_id_for_index(&self, index: usize) -> Option<ElementId> {
        let order = self.build_order();
        let mut visible_count = 0;
        for id in &order {
            if let Some(elem) = self.elements.get(id) {
                if elem.value.is_some() {
                    if visible_count == index {
                        return Some(*id);
                    }
                    visible_count += 1;
                }
            }
        }
        None
    }

    /// Deletes the element at the given visible index.
    ///
    /// Returns the ID of the deleted element, if any.
    pub fn delete(&mut self, index: usize) -> Option<ElementId> {
        let id = self.find_id_for_index(index)?;
        self.delete_by_id(id);
        Some(id)
    }

    /// Deletes an element by ID (for replication).
    pub fn delete_by_id(&mut self, id: ElementId) {
        if let Some(elem) = self.elements.get_mut(&id) {
            elem.value = None;
        }
    }

    /// Merges another RGA into this one.
    ///
    /// This operation is commutative, associative, and idempotent.
    pub fn merge(&mut self, other: &Self) {
        // Update timestamp
        self.timestamp = self.timestamp.receive(&other.timestamp);

        // Merge elements
        for (id, other_elem) in &other.elements {
            if id.is_root() {
                continue;
            }

            if let Some(existing) = self.elements.get_mut(id) {
                // Element exists - merge tombstone status (delete wins)
                if other_elem.value.is_none() {
                    existing.value = None;
                }
            } else {
                // New element - add it
                self.elements.insert(*id, other_elem.clone());
            }
        }
    }

    /// Creates a new RGA that is the merge of this and another.
    #[must_use]
    pub fn merged(&self, other: &Self) -> Self {
        let mut result = self.clone();
        result.merge(other);
        result
    }
}

impl<T: Clone> RGA<T> {
    /// Returns the ElementId at a visible index.
    ///
    /// This is useful for anchoring formatting marks to specific characters
    /// in a way that survives CRDT merges.
    #[must_use]
    pub fn element_id_at(&self, index: usize) -> Option<ElementId> {
        let order = self.build_order();
        let mut visible_count = 0;
        for id in &order {
            if let Some(elem) = self.elements.get(id) {
                if elem.value.is_some() {
                    if visible_count == index {
                        return Some(*id);
                    }
                    visible_count += 1;
                }
            }
        }
        None
    }

    /// Returns the ElementId after the last visible element.
    ///
    /// Useful for anchoring to the end of text.
    #[must_use]
    pub fn last_element_id(&self) -> Option<ElementId> {
        let order = self.build_order();
        let mut last_visible = None;
        for id in &order {
            if let Some(elem) = self.elements.get(id) {
                if elem.value.is_some() {
                    last_visible = Some(*id);
                }
            }
        }
        last_visible
    }

    /// Returns all ElementIds in order (including tombstones).
    ///
    /// Useful for advanced operations like formatting resolution.
    #[must_use]
    pub fn element_ids_in_order(&self) -> Vec<ElementId> {
        self.build_order()
    }

    /// Returns the visible index of an ElementId, if it exists and is not deleted.
    #[must_use]
    pub fn index_of(&self, target: &ElementId) -> Option<usize> {
        let order = self.build_order();
        let mut visible_count = 0;
        for id in &order {
            if let Some(elem) = self.elements.get(id) {
                if elem.value.is_some() {
                    if id == target {
                        return Some(visible_count);
                    }
                    visible_count += 1;
                }
            }
        }
        None
    }

    /// Returns whether an ElementId exists (even if tombstoned).
    #[must_use]
    pub fn contains_element(&self, id: &ElementId) -> bool {
        self.elements.contains_key(id)
    }

    /// Returns whether an ElementId is tombstoned (deleted).
    #[must_use]
    pub fn is_tombstoned(&self, id: &ElementId) -> bool {
        self.elements
            .get(id)
            .map(|e| e.value.is_none())
            .unwrap_or(false)
    }
}

impl RGA<char> {
    /// Creates an RGA from a string.
    #[must_use]
    pub fn from_str(s: &str, peer_id: PeerId) -> Self {
        let mut rga = Self::new(peer_id);
        for (i, c) in s.chars().enumerate() {
            rga.insert(i, c);
        }
        rga
    }

    /// Converts the RGA to a String.
    #[must_use]
    pub fn as_string(&self) -> String {
        self.to_vec().into_iter().collect()
    }

    /// Inserts a string at the given index.
    pub fn insert_str(&mut self, index: usize, s: &str) {
        for (i, c) in s.chars().enumerate() {
            self.insert(index + i, c);
        }
    }

    /// Deletes a range of characters.
    pub fn delete_range(&mut self, start: usize, count: usize) {
        // Delete from end to start to keep indices valid
        for _ in 0..count {
            self.delete(start);
        }
    }
}
