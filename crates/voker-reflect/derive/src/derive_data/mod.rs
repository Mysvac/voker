//! Provide some tools for parsing token stream.

// -----------------------------------------------------------------------------
// Modules

mod attributes;
mod define_parser;
mod reflect_derive;
mod reflect_enum;
mod reflect_meta;
mod reflect_struct;
mod type_signature;

// -----------------------------------------------------------------------------
// Internal API

pub(crate) use attributes::{FieldAttributes, TypeAttributes};

pub(crate) use define_parser::{ReflectOpaqueParser, ReflectTypePathParser};
pub(crate) use type_signature::TypeSignature;

pub(crate) use reflect_derive::ReflectDerive;
pub(crate) use reflect_enum::{EnumVariant, EnumVariantFields, ReflectEnum};
pub(crate) use reflect_meta::ReflectMeta;
pub(crate) use reflect_struct::{FieldAccessors, ReflectStruct, StructField};
