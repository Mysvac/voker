#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, expect(internal_features, reason = "needed for fake_variadic"))]
#![cfg_attr(docsrs, feature(doc_cfg, rustdoc_internals))]
#![no_std]

// -----------------------------------------------------------------------------
// Compilation config

/// Some macros used for compilation control.
pub mod cfg {
    voker_cfg::define_alias! {
        #[cfg(feature = "std")] => std,
        #[cfg(feature = "backtrace")] => backtrace,
        #[cfg(feature = "reflect_docs")] => reflect_docs,
    }
}

// -----------------------------------------------------------------------------
// Extern Self

// Usually, we need to use `crate` in the crate itself and use `voker_reflect` in doc testing.
// But `macro_utils::Manifest` can only choose one, so we must have an
// `extern self` to ensure `voker_reflect` can be used as an alias for `crate`.
extern crate self as voker_reflect;

// -----------------------------------------------------------------------------
// no_std support

crate::cfg::std! {
    extern crate std;
}

extern crate alloc;

// -----------------------------------------------------------------------------
// Modules

mod reflection;

pub mod access;
pub mod impls;
pub mod info;
pub mod ops;
pub mod registry;
pub mod serde;

// -----------------------------------------------------------------------------
// Top-Level exports

pub mod __macro_exports;

pub use info::TypePath;
pub use reflection::{FromReflect, Reflect, reflect_hasher};
pub use voker_reflect_derive as derive;
pub use voker_reflect_derive::Reflect;
pub use voker_reflect_derive::{auto_register, impl_auto_register};

pub mod prelude {
    pub use crate::access::{PathAccessor, ReflectPathAccess};
    pub use crate::info::{TypeInfo, TypePath, Typed};
    pub use crate::registry::{ReflectConvert, ReflectDefault, ReflectFromReflect};
    pub use crate::registry::{TypeMeta, TypeRegistry};
    pub use crate::serde::{DeserializeDriver, SerializeDriver};
    pub use crate::serde::{ReflectDeserializeDriver, ReflectSerializeDriver};
    pub use crate::{FromReflect, Reflect};
}
