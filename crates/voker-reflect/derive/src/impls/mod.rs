// -----------------------------------------------------------------------------
// Modules

mod match_reflect;

mod enum_kind;
mod opaque_kind;
mod struct_kind;
mod tuple_struct_kind;
mod unit_kind;

mod auto_register;
mod common_imps;
mod reflect_trait;
mod struct_clone;
mod struct_from_reflect;
mod trait_get_type_meta;
mod trait_reflect;
mod trait_type_path;
mod trait_typed;

// -----------------------------------------------------------------------------
// Internal API

pub(crate) use match_reflect::match_reflect_impls;

use auto_register::get_auto_register_impl;
use common_imps::get_common_apply_tokens;
use common_imps::get_common_from_reflect_tokens;
use enum_kind::impl_enum;
use struct_clone::get_struct_clone_impl;
use struct_from_reflect::impl_struct_from_reflect;
use struct_kind::impl_struct;
use trait_get_type_meta::impl_trait_get_type_meta;
use trait_reflect::impl_trait_reflect;
use trait_typed::impl_trait_typed;
use tuple_struct_kind::impl_tuple_struct;
use unit_kind::impl_unit;

pub(crate) use opaque_kind::impl_opaque;
pub(crate) use reflect_trait::impl_reflect_trait;
pub(crate) use trait_type_path::impl_trait_type_path;
