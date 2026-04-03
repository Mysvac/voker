//! Provide path-based access helpers for reflected data.
//!
//! This module provides utilities to access nested values inside `Reflect` types
//! using a compact, human-readable path syntax. There are two complementary
//! APIs exposed here:
//!
//! - [`PathAccessor`]: a parsed, reusable accessor optimized for repeated queries.
//!   Use this when you will run the same path multiple times — the path is
//!   parsed once and then reused without additional parsing or allocation.
//! - [`ReflectPathAccess`]: a convenience trait implemented for `Reflect` that
//!   parses the provided access path on each call. It is suitable for one-off
//!   lookups where reuse is not required.
//!
//! The module also exposes the [`AccessPath`] abstraction which lets you provide
//! custom path representations (for example, `&str`, `String`, or user-defined
//! types) and the `PathAccessError` type for detailed error reporting (parse
//! errors, traversal/access errors, and invalid downcasts).
//!
//! # Syntax
//!
//! We provided 4 single layer access kind:
//!
//! - FieldName: Can be used to access struct or enum's struct variant.
//! - FieldIndex: Can be used to access struct or enum's struct variant.
//! - TupleIndex: Can be used to access tuple, tuple-struct or enum's tuple variant.
//! - ListIndex: Can be used to access list and array.
//!
//! The specific syntax can be defined by [`AccessPath`].
//! Here is the syntax used by the default implementation (`&str`):
//!
//! - FieldName: `.Name`, e.g. `.field_0`
//! - FieldIndex: `#Number`, e.g. `#1`
//! - TupleIndex: `.Number`, e.g. `.1`
//! - ListIndex: `[Number]`, e.g. `[1]`
//!
//! Notes:
//! - `.Number` is always a tuple-style index. It does not access struct fields by index.
//! - Struct field index access must use `#Number`.
//!
//! # Examples
//!
//! `ReflectPathAccess`:
//!
//! ```no_run
//! use voker_reflect::{Reflect, access::ReflectPathAccess};
//!
//! #[derive(Reflect)]
//! struct Foo { id: u32, data: (Vec<u8>, bool) }
//!
//! let foo = Foo { id: 1, data: (vec![1,2,3], true) };
//!
//! // parse-and-access in a single call
//! let v = foo.access_as::<u8>(".data.0[1]").unwrap();
//! assert_eq!(*v, 2);
//! ```
//!
//! `PathAccessor`:
//!
//! ```no_run
//! use voker_reflect::{Reflect, access::PathAccessor};
//!
//! #[derive(Reflect)]
//! struct Foo { id: u32, data: (Vec<u8>, bool) }
//!
//! let mut foo = Foo { id: 1, data: (vec![0,1,2,3], false) };
//! let accessor = PathAccessor::parse_static(".data.0[3]").unwrap();
//!
//! let v = accessor.access_as::<u8>(&foo).unwrap();
//! assert_eq!(*v, 3);
//!
//! foo.data.0 = vec![10, 11, 12, 13];
//!
//! // reuse
//! let val = accessor.access_as::<u8>(&foo).unwrap();
//! assert_eq!(*val, 13);
//! ```

// -----------------------------------------------------------------------------
// Modules

mod accessor;
mod path;
mod path_access;
mod string_parser;

// -----------------------------------------------------------------------------
// Exports

pub use accessor::{AccessError, AccessErrorKind};
pub use accessor::{Accessor, OffsetAccessor};
pub use path::{AccessPath, ParseError};
pub use path_access::{PathAccessError, PathAccessor, ReflectPathAccess};
