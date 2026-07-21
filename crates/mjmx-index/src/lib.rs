//! Hand-rolled typed indices for `mjmx`.
//!
//! A tiny, dependency-free analogue of `oxc-index` / `index_vec`. The AST stores
//! nodes in flat arenas addressed by typed newtype indices (like [`NodeId`]) rather
//! than raw `usize`, so an index into one collection can't accidentally be used
//! against another. [`IndexVec`] is a `Vec` wrapper that only accepts its associated
//! index type.

use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

/// A type that can be used as a typed index into an [`IndexVec`].
pub trait Idx: Copy {
    /// Construct the index from a `usize` position.
    fn from_usize(i: usize) -> Self;
    /// The `usize` position this index refers to.
    fn into_usize(self) -> usize;
}

/// A typed index for AST nodes, backed by a `u32`.
///
/// Supports at most `u32::MAX` (≈4.29 billion) nodes; constructing a `NodeId`
/// beyond that range panics via the guarded cast in [`Idx::from_usize`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u32);

impl NodeId {
    /// The `usize` position this id refers to.
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl Idx for NodeId {
    fn from_usize(i: usize) -> Self {
        assert!(
            i <= u32::MAX as usize,
            "NodeId overflow: {i} exceeds u32::MAX"
        );
        NodeId(i as u32)
    }

    fn into_usize(self) -> usize {
        self.0 as usize
    }
}

/// A `Vec<T>` addressed by a typed index `I` instead of a raw `usize`.
pub struct IndexVec<I: Idx, T> {
    raw: Vec<T>,
    _marker: PhantomData<fn(I) -> I>,
}

impl<I: Idx, T> IndexVec<I, T> {
    /// Create an empty `IndexVec`.
    pub fn new() -> Self {
        IndexVec {
            raw: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Create an empty `IndexVec` with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        IndexVec {
            raw: Vec::with_capacity(capacity),
            _marker: PhantomData,
        }
    }

    /// Push a value and return the typed index it was stored at.
    pub fn push(&mut self, value: T) -> I {
        let idx = I::from_usize(self.raw.len());
        self.raw.push(value);
        idx
    }

    /// The number of elements.
    pub fn len(&self) -> usize {
        self.raw.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    /// Get a reference to the element at `index`, or `None` if out of range.
    pub fn get(&self, index: I) -> Option<&T> {
        self.raw.get(index.into_usize())
    }

    /// Iterate over the elements.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.raw.iter()
    }

    /// Iterate mutably over the elements.
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.raw.iter_mut()
    }
}

impl<I: Idx, T> Default for IndexVec<I, T> {
    fn default() -> Self {
        IndexVec::new()
    }
}

impl<I: Idx, T: Clone> Clone for IndexVec<I, T> {
    fn clone(&self) -> Self {
        IndexVec {
            raw: self.raw.clone(),
            _marker: PhantomData,
        }
    }
}

impl<I: Idx, T: std::fmt::Debug> std::fmt::Debug for IndexVec<I, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.raw.iter()).finish()
    }
}

impl<I: Idx, T> Index<I> for IndexVec<I, T> {
    type Output = T;

    fn index(&self, index: I) -> &T {
        &self.raw[index.into_usize()]
    }
}

impl<I: Idx, T> IndexMut<I> for IndexVec<I, T> {
    fn index_mut(&mut self, index: I) -> &mut T {
        &mut self.raw[index.into_usize()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_returns_incrementing_ids() {
        let mut v: IndexVec<NodeId, &str> = IndexVec::new();
        let a = v.push("a");
        let b = v.push("b");
        let c = v.push("c");
        assert_eq!(a, NodeId::from_usize(0));
        assert_eq!(b, NodeId::from_usize(1));
        assert_eq!(c, NodeId::from_usize(2));
        assert_eq!(a.index(), 0);
        assert_eq!(v.len(), 3);
        assert!(!v.is_empty());
    }

    #[test]
    fn index_round_trip() {
        let mut v: IndexVec<NodeId, u32> = IndexVec::with_capacity(2);
        let a = v.push(10);
        let b = v.push(20);
        assert_eq!(v[a], 10);
        assert_eq!(v[b], 20);
        v[a] = 11;
        assert_eq!(v[a], 11);
    }

    #[test]
    fn get_out_of_range_is_none() {
        let mut v: IndexVec<NodeId, u32> = IndexVec::new();
        let a = v.push(1);
        assert_eq!(v.get(a), Some(&1));
        assert_eq!(v.get(NodeId::from_usize(5)), None);
    }

    #[test]
    fn iter_and_default() {
        let v: IndexVec<NodeId, u8> = IndexVec::default();
        assert!(v.is_empty());
        let mut v2: IndexVec<NodeId, u8> = IndexVec::new();
        v2.push(1);
        v2.push(2);
        for x in v2.iter_mut() {
            *x += 1;
        }
        let collected: Vec<u8> = v2.iter().copied().collect();
        assert_eq!(collected, vec![2, 3]);
    }
}
