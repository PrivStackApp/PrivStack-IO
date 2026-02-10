//! Hybrid Logical Clock implementation for causal ordering.
//!
//! Combines physical time with a logical counter to ensure:
//! - Monotonicity (always increasing)
//! - Causality (if A happens-before B, then ts(A) < ts(B))
//! - Bounded drift from physical time

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

/// A Hybrid Logical Clock timestamp.
///
/// Consists of:
/// - `wall_time`: Milliseconds since Unix epoch (physical component)
/// - `logical`: Logical counter for events at the same wall time
///
/// Based on the HLC algorithm from "Logical Physical Clocks" (Kulkarni et al.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HybridTimestamp {
    /// Physical time component (milliseconds since Unix epoch).
    wall_time: u64,
    /// Logical counter for ordering events at the same wall time.
    logical: u32,
}

impl HybridTimestamp {
    /// Creates a new timestamp at the current time.
    #[must_use]
    pub fn now() -> Self {
        let wall_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before Unix epoch")
            .as_millis() as u64;

        Self {
            wall_time,
            logical: 0,
        }
    }

    /// Creates a timestamp from components.
    #[must_use]
    pub const fn new(wall_time: u64, logical: u32) -> Self {
        Self { wall_time, logical }
    }

    /// Returns the wall time component.
    #[must_use]
    pub const fn wall_time(&self) -> u64 {
        self.wall_time
    }

    /// Returns the logical counter.
    #[must_use]
    pub const fn logical(&self) -> u32 {
        self.logical
    }

    /// Generates the next timestamp, ensuring monotonicity.
    ///
    /// This should be called when creating a new local event.
    #[must_use]
    pub fn tick(&self) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before Unix epoch")
            .as_millis() as u64;

        if now > self.wall_time {
            Self {
                wall_time: now,
                logical: 0,
            }
        } else {
            Self {
                wall_time: self.wall_time,
                logical: self.logical.saturating_add(1),
            }
        }
    }

    /// Updates this clock based on a received timestamp.
    ///
    /// Ensures the resulting timestamp is greater than both the current
    /// clock and the received timestamp.
    #[must_use]
    pub fn receive(&self, other: &Self) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before Unix epoch")
            .as_millis() as u64;

        let max_wall = now.max(self.wall_time).max(other.wall_time);

        let logical = if max_wall == self.wall_time && max_wall == other.wall_time {
            self.logical.max(other.logical).saturating_add(1)
        } else if max_wall == self.wall_time {
            self.logical.saturating_add(1)
        } else if max_wall == other.wall_time {
            other.logical.saturating_add(1)
        } else {
            0
        };

        Self {
            wall_time: max_wall,
            logical,
        }
    }

    /// Returns true if this timestamp is causally before the other.
    #[must_use]
    pub fn is_before(&self, other: &Self) -> bool {
        self < other
    }

    /// Returns true if this timestamp is causally after the other.
    #[must_use]
    pub fn is_after(&self, other: &Self) -> bool {
        self > other
    }
}

impl Default for HybridTimestamp {
    fn default() -> Self {
        Self::now()
    }
}

impl PartialOrd for HybridTimestamp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HybridTimestamp {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.wall_time.cmp(&other.wall_time) {
            Ordering::Equal => self.logical.cmp(&other.logical),
            other => other,
        }
    }
}
