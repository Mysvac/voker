use alloc::vec::Vec;
use core::fmt::{Debug, Formatter};
use core::slice::Iter;

use crate::info::TypeInfo;

/// Helper struct for managing a stack of [`TypeInfo`] instances.
///
/// This is useful for tracking the type hierarchy when serializing and deserializing types.
#[derive(Default, Clone)]
pub(super) struct TypeInfoStack {
    stack: Vec<&'static TypeInfo>,
}

impl TypeInfoStack {
    /// Create a new empty [`TypeInfoStack`].
    pub const fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Push a new [`TypeInfo`] onto the stack.
    pub fn push(&mut self, type_info: &'static TypeInfo) {
        self.stack.push(type_info);
    }

    /// Pop the last [`TypeInfo`] off the stack.
    pub fn pop(&mut self) {
        self.stack.pop();
    }

    /// clear the stack
    pub fn clear(&mut self) {
        self.stack.clear();
    }

    /// Get an iterator over the stack in the order they were pushed.
    pub fn iter(&self) -> Iter<'_, &'static TypeInfo> {
        self.stack.iter()
    }
}

impl Debug for TypeInfoStack {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let mut iter = self.iter();

        if let Some(first) = iter.next() {
            writeln!(f, "`{}`", first.type_path())?;
        }

        for info in iter {
            writeln!(f, " -> `{}`", info.type_path())?;
        }

        Ok(())
    }
}
