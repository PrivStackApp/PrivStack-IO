//! Observed-Remove Set (OR-Set / Add-Wins Set).
//!
//! A CRDT set that supports both add and remove operations. Unlike naive sets,
//! concurrent add and remove of the same element results in the element being present
//! (add-wins semantics).
//!
//! Each add operation creates a unique tag. Remove operations remove specific tags.
//! An element is in the set if it has at least one tag that hasn't been removed.
//!
//! Use cases:
//! - Document collections (list of documents)
//! - Tags on a document
//! - Block children lists

use privstack_types::PeerId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// A unique tag identifying a specific add operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tag(Uuid);

impl Tag {
    /// Creates a new unique tag.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for Tag {
    fn default() -> Self {
        Self::new()
    }
}

/// An Observed-Remove Set (OR-Set).
///
/// Provides set semantics with add and remove operations that commute properly.
/// Add-wins: if an element is concurrently added and removed, it remains in the set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ORSet<T>
where
    T: Eq + std::hash::Hash + Clone,
{
    /// Map from element to its active tags.
    elements: HashMap<T, HashSet<Tag>>,
    /// Set of all removed tags (tombstones).
    tombstones: HashSet<Tag>,
}

impl<T> Default for ORSet<T>
where
    T: Eq + std::hash::Hash + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> ORSet<T>
where
    T: Eq + std::hash::Hash + Clone,
{
    /// Creates a new empty set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            elements: HashMap::new(),
            tombstones: HashSet::new(),
        }
    }

    /// Returns true if the set contains the element.
    #[must_use]
    pub fn contains(&self, element: &T) -> bool {
        self.elements
            .get(element)
            .map(|tags| !tags.is_empty())
            .unwrap_or(false)
    }

    /// Returns the number of elements in the set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.elements
            .values()
            .filter(|tags| !tags.is_empty())
            .count()
    }

    /// Returns true if the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns an iterator over the elements in the set.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.elements
            .iter()
            .filter(|(_, tags)| !tags.is_empty())
            .map(|(elem, _)| elem)
    }

    /// Adds an element to the set.
    ///
    /// Returns the tag created for this add operation.
    /// The same element can be added multiple times, creating multiple tags.
    pub fn add(&mut self, element: T, _peer_id: PeerId) -> Tag {
        let tag = Tag::new();
        self.add_with_tag(element, tag);
        tag
    }

    /// Adds an element with a specific tag (for replication).
    pub fn add_with_tag(&mut self, element: T, tag: Tag) {
        // Only add if the tag hasn't been tombstoned
        if !self.tombstones.contains(&tag) {
            self.elements.entry(element).or_default().insert(tag);
        }
    }

    /// Removes an element from the set.
    ///
    /// This removes all current tags for the element. Concurrent adds with new tags
    /// will still succeed (add-wins semantics).
    ///
    /// Returns the tags that were removed.
    pub fn remove(&mut self, element: &T) -> Vec<Tag> {
        let removed_tags = self
            .elements
            .get_mut(element)
            .map(|tags| {
                let removed: Vec<Tag> = tags.drain().collect();
                removed
            })
            .unwrap_or_default();

        // Add removed tags to tombstones
        for tag in &removed_tags {
            self.tombstones.insert(*tag);
        }

        removed_tags
    }

    /// Removes specific tags (for replication).
    pub fn remove_tags(&mut self, tags: &[Tag]) {
        for tag in tags {
            self.tombstones.insert(*tag);
        }

        // Remove these tags from all elements
        for tags_set in self.elements.values_mut() {
            for tag in tags {
                tags_set.remove(tag);
            }
        }
    }

    /// Merges another OR-Set into this one.
    ///
    /// The resulting set contains all elements that have at least one tag
    /// that isn't tombstoned in either set.
    pub fn merge(&mut self, other: &Self) {
        // Merge tombstones first
        self.tombstones.extend(&other.tombstones);

        // Merge elements
        for (element, other_tags) in &other.elements {
            let entry = self.elements.entry(element.clone()).or_default();
            for tag in other_tags {
                if !self.tombstones.contains(tag) {
                    entry.insert(*tag);
                }
            }
        }

        // Clean up: remove tombstoned tags from our elements
        for tags in self.elements.values_mut() {
            tags.retain(|tag| !self.tombstones.contains(tag));
        }
    }

    /// Creates a new set that is the merge of this and another.
    #[must_use]
    pub fn merged(&self, other: &Self) -> Self {
        let mut result = self.clone();
        result.merge(other);
        result
    }

    /// Returns all tags for an element (for replication).
    #[must_use]
    pub fn tags_for(&self, element: &T) -> Option<&HashSet<Tag>> {
        self.elements.get(element)
    }

    /// Returns all tombstones (for replication/debugging).
    #[must_use]
    pub fn tombstones(&self) -> &HashSet<Tag> {
        &self.tombstones
    }

    /// Cleans up tombstones that are older than a certain threshold.
    ///
    /// This is a garbage collection operation. Only safe to call when you're
    /// certain no peer will send adds with these old tags.
    pub fn gc_tombstones(&mut self, keep_tags: impl Fn(&Tag) -> bool) {
        self.tombstones.retain(|tag| keep_tags(tag));
    }
}

impl<T> FromIterator<T> for ORSet<T>
where
    T: Eq + std::hash::Hash + Clone,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let peer = PeerId::new();
        let mut set = Self::new();
        for item in iter {
            set.add(item, peer);
        }
        set
    }
}
