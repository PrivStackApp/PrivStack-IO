//! CRDT implementations for PrivStack.
//!
//! This crate provides Conflict-free Replicated Data Types:
//!
//! - [`LWWRegister<T>`] — Last-Writer-Wins Register for single values
//! - [`VectorClock`] — Causality tracking across peers
//! - [`ORSet<T>`] — Observed-Remove Set for collections
//! - [`PNCounter`] — Positive-Negative Counter for distributed inc/dec
//! - [`RGA<T>`] — Replicated Growable Array for sequences/text
//!
//! All CRDTs in this crate satisfy the following properties:
//! - **Commutative**: merge(a, b) == merge(b, a)
//! - **Associative**: merge(merge(a, b), c) == merge(a, merge(b, c))
//! - **Idempotent**: merge(a, a) == a
//!
//! These properties ensure that replicas will converge to the same state
//! regardless of the order in which operations are received.

mod lww_register;
mod orset;
mod pn_counter;
mod rga;
mod vector_clock;

pub use lww_register::LWWRegister;
pub use orset::{ORSet, Tag};
pub use pn_counter::PNCounter;
pub use rga::{ElementId, RGA};
pub use vector_clock::{CausalOrder, VectorClock};
