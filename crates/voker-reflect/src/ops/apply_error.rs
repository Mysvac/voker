use alloc::borrow::Cow;
use core::{error, fmt};

use crate::info::{ReflectKind, ReflectKindError};

/// A enumeration of all error outcomes
/// that might happen when running [`apply`](crate::Reflect::apply).
#[derive(Debug)]
pub enum ApplyError {
    /// Special reflection type, not allowed to apply.
    NotSupport { type_path: &'static str },
    /// Attempted to apply an array or tuple like type to another of different size.
    MismatchedSize { from_size: usize, to_size: usize },
    /// Attempted to apply incompatible types.
    MismatchedType {
        from_type: Cow<'static, str>,
        to_type: Cow<'static, str>,
    },
    /// Attempted to apply the wrong [kind](ReflectKind) to a type, e.g. a struct to an enum.
    MismatchedKind {
        from_kind: ReflectKind,
        to_kind: ReflectKind,
    },
    /// The enum didn't contain a variant with the give name.
    MismatchedVariant {
        from_variant: Cow<'static, str>,
        to_variant: Cow<'static, str>,
    },
}

impl fmt::Display for ApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSupport { type_path } => {
                write!(f, "type `{type_path}` does not support `apply`")
            }
            Self::MismatchedSize { from_size, to_size } => {
                write!(
                    f,
                    "attempted to apply with {from_size} size to {to_size} size"
                )
            }
            Self::MismatchedType { from_type, to_type } => {
                write!(f, "attempted to apply `{from_type}` to `{to_type}`")
            }
            Self::MismatchedKind { from_kind, to_kind } => {
                write!(f, "attempted to apply `{from_kind}` to `{to_kind}`")
            }
            Self::MismatchedVariant {
                from_variant,
                to_variant,
            } => {
                write!(f, "attempted to apply `{from_variant}` to `{to_variant}`")
            }
        }
    }
}

impl error::Error for ApplyError {}

impl From<ReflectKindError> for ApplyError {
    #[inline]
    fn from(value: ReflectKindError) -> Self {
        Self::MismatchedKind {
            from_kind: value.received,
            to_kind: value.expected,
        }
    }
}
