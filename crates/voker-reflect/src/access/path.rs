//! Provide `path` interface for path accessing.

use alloc::borrow::Cow;
use core::fmt;

use crate::access::OffsetAccessor;

// -----------------------------------------------------------------------------
// ParseError

/// An interface for representing path parsing error information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError<'a> {
    /// Position in `path`.
    pub offset: usize,
    /// The path that the error occurred in.
    pub path: &'a str,
    /// The underlying error.
    pub error: Cow<'a, str>,
}

impl fmt::Display for ParseError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Encountered an error at offset {} while parsing `{}`: {}",
            self.offset, self.path, self.error,
        )
    }
}

impl core::error::Error for ParseError<'_> {}

// -----------------------------------------------------------------------------
// AccessPath

/// An interface where the type implementing
/// this trait can be considered as a "Path" for path access.
///
/// This allows users to customize the syntax of access path.
///
/// This crate defaults to providing implementation for [`&str`]
///
/// # Default Syntax
///
/// - FieldName: `.Name`, e.g. `.field_0`
/// - FieldIndex: `#Number`, e.g. `#1`
/// - TupleIndex: `.Number`, e.g. `.1`
/// - ListIndex: `[Number]`, e.g. `[1]`
///
/// The FieldName cannot begin with number.
///
/// [`&str`]: str
pub trait AccessPath<'a> {
    /// Parses the path and returns an iterator of [`OffsetAccessor`].
    fn parse_to_accessor(&self)
    -> impl Iterator<Item = Result<OffsetAccessor<'a>, ParseError<'a>>>;
}
