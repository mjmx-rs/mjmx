//! Arena allocator for `mrml`.
//!
//! A thin wrapper around [`bumpalo`], analogous to `oxc_allocator`. AST nodes are
//! allocated in an [`Allocator`] arena and borrow from it for a shared lifetime,
//! avoiding per-node heap allocations and enabling cheap, copyable references.

pub use bumpalo::Bump;

/// A `Box` whose contents live in an [`Allocator`] arena.
pub type Box<'alloc, T> = bumpalo::boxed::Box<'alloc, T>;

/// A `Vec` whose contents live in an [`Allocator`] arena.
pub type Vec<'alloc, T> = bumpalo::collections::Vec<'alloc, T>;

/// An arena allocator backing `mrml`'s AST and other bump-allocated data.
///
/// Values allocated here share the allocator's lifetime and are freed all at once
/// when the [`Allocator`] is dropped.
#[derive(Debug, Default)]
pub struct Allocator {
    bump: Bump,
}

impl Allocator {
    /// Create a new, empty arena.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate `value` in the arena and return a mutable reference to it.
    pub fn alloc<T>(&self, value: T) -> &mut T {
        self.bump.alloc(value)
    }
}

impl core::ops::Deref for Allocator {
    type Target = Bump;

    fn deref(&self) -> &Self::Target {
        &self.bump
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_a_value() {
        let alloc = Allocator::new();
        let value = alloc.alloc(42_u32);
        assert_eq!(*value, 42);
    }

    #[test]
    fn builds_a_vec_in_the_arena() {
        let alloc = Allocator::new();
        let mut vec = Vec::new_in(&alloc);
        vec.push(1_u32);
        vec.push(2);
        vec.push(3);
        assert_eq!(vec.as_slice(), &[1, 2, 3]);
    }
}
