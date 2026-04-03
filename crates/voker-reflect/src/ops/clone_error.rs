use core::fmt;

/// A enumeration of all error outcomes that might happen when
/// running [`Reflect::reflect_clone`](crate::Reflect::reflect_clone).
#[derive(Debug)]
pub enum ReflectCloneError {
    /// The type does not support clone.
    NotSupport { type_path: &'static str },
    /// The field cannot be cloned.
    FieldNotCloneable {
        type_path: &'static str,
        field: &'static str,
        variant: Option<&'static str>,
    },
}

impl fmt::Display for ReflectCloneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSupport { type_path } => {
                write!(f, "`reflect_clone` not support for `{type_path}`")
            }
            Self::FieldNotCloneable {
                type_path,
                field,
                variant,
            } => {
                if let Some(variant) = variant {
                    write!(
                        f,
                        "field `{}::{}::{}` cannot be cloned by `reflect_clone`",
                        type_path, variant, field,
                    )
                } else {
                    write!(
                        f,
                        "field `{}::{}` cannot be cloned by `reflect_clone`",
                        type_path, field,
                    )
                }
            }
        }
    }
}

impl core::error::Error for ReflectCloneError {}
