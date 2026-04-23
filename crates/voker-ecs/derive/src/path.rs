//! This independent module is used to provide the required path.
//! So as to minimize changes when the `voker_ecs` structure is modified.

use proc_macro2::TokenStream;
use quote::quote;

// -----------------------------------------------------------------------------
// Crate Path

/// Get the correct access path to the `voker_ecs` crate.
pub(crate) fn voker_ecs() -> syn::Path {
    voker_macro_utils::crate_path!(voker_ecs)
}

pub(crate) use voker_macro_utils::full_path as fp;

// -----------------------------------------------------------------------------
// Resource

#[inline(always)]
pub(crate) fn component_cloner_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::clone::ComponentCloner
    }
}

#[inline(always)]
pub(crate) fn resource_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::resource::Resource
    }
}

#[inline(always)]
pub(crate) fn component_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::component::Component
    }
}

#[inline(always)]
pub(crate) fn required_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::component::Required
    }
}

#[inline(always)]
pub(crate) fn storage_mode_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::component::StorageMode
    }
}

#[inline(always)]
pub(crate) fn component_collector_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::component::ComponentCollector
    }
}

#[inline(always)]
pub(crate) fn component_writer_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::component::ComponentWriter
    }
}

#[inline(always)]
pub(crate) fn component_hook_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::component::ComponentHook
    }
}

#[inline(always)]
pub(crate) fn bundle_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::bundle::Bundle
    }
}

#[inline(always)]
pub(crate) fn schedule_label_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::schedule::ScheduleLabel
    }
}

#[inline(always)]
pub(crate) fn system_set_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::schedule::SystemSet
    }
}

#[inline(always)]
pub(crate) fn system_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::system::System
    }
}

#[inline(always)]
pub(crate) fn into_system_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::system::IntoSystem
    }
}

#[inline(always)]
pub(crate) fn system_set_begin_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::schedule::SystemSetBegin
    }
}

#[inline(always)]
pub(crate) fn system_set_end_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::schedule::SystemSetEnd
    }
}

#[inline(always)]
pub(crate) fn game_error_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::error::GameError
    }
}

#[inline(always)]
pub(crate) fn severity_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::error::Severity
    }
}

#[inline(always)]
pub(crate) fn event_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::event::Event
    }
}

#[inline(always)]
pub(crate) fn entity_event_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::event::EntityEvent
    }
}

#[inline(always)]
pub(crate) fn entity_event_mut_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::event::EntityEventMut
    }
}

#[inline(always)]
pub(crate) fn child_of_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::hierarchy::ChildOf
    }
}

#[inline(always)]
pub(crate) fn global_trigger_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::event::GlobalTrigger
    }
}

#[inline(always)]
pub(crate) fn entity_trigger_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::event::EntityTrigger
    }
}

#[inline(always)]
pub(crate) fn propagate_entity_trigger_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::event::PropagateEntityTrigger
    }
}

#[inline(always)]
pub(crate) fn message_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::message::Message
    }
}

#[inline(always)]
pub(crate) fn map_entities_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::entity::MapEntities
    }
}

#[inline(always)]
pub(crate) fn entity_mapper_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::entity::EntityMapper
    }
}

#[inline(always)]
pub(crate) fn relationship_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::relationship::Relationship
    }
}

#[inline(always)]
pub(crate) fn relationship_target_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::relationship::RelationshipTarget
    }
}

#[inline(always)]
pub(crate) fn relationship_registrar_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::relationship::RelationshipRegistrar
    }
}

#[inline(always)]
pub(crate) fn entity_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::entity::Entity
    }
}

#[inline(always)]
pub(crate) fn query_data_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::query::QueryData
    }
}

#[inline(always)]
pub(crate) fn readonly_query_data_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::query::ReadOnlyQueryData
    }
}

#[inline(always)]
pub(crate) fn world_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::world::World
    }
}

#[inline(always)]
pub(crate) fn unsafe_world_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::world::UnsafeWorld
    }
}

#[inline(always)]
pub(crate) fn tick_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::tick::Tick
    }
}

#[inline(always)]
pub(crate) fn archetype_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::archetype::Archetype
    }
}

#[inline(always)]
pub(crate) fn table_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::storage::Table
    }
}

#[inline(always)]
pub(crate) fn table_row_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::storage::TableRow
    }
}

#[inline(always)]
pub(crate) fn access_param_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::system::AccessParam
    }
}

#[inline(always)]
pub(crate) fn filter_param_builder_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::system::FilterParamBuilder
    }
}

#[inline(always)]
pub(crate) fn system_param_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::system::SystemParam
    }
}

#[inline(always)]
pub(crate) fn system_param_error_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::system::SystemParamError
    }
}

#[inline(always)]
pub(crate) fn access_table_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::system::AccessTable
    }
}

#[inline(always)]
pub(crate) fn system_meta_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::system::SystemMeta
    }
}

#[inline(always)]
pub(crate) fn deferred_world_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::world::DeferredWorld
    }
}
