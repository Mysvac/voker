//! Provide some tools for parsing attributes.
//!
//! This includes all the attributes required to generate code,
//! not just reflect custom attributes.

// -----------------------------------------------------------------------------
// Modules

mod custom_attributes;
mod field_attributes;
mod flags;
mod reflect_docs;
mod type_attributes;

// -----------------------------------------------------------------------------
// Internal API

use custom_attributes::CustomAttributes;
use flags::{TraitAvailableFlags, TraitImplSwitches};
use reflect_docs::ReflectDocs;

pub(crate) use field_attributes::FieldAttributes;
pub(crate) use type_attributes::TypeAttributes;
