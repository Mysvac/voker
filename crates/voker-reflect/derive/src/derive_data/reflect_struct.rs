use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{Field, Ident};

use super::{FieldAttributes, ReflectMeta};

// -----------------------------------------------------------------------------
// Define

pub(crate) struct ReflectStruct<'a> {
    meta: ReflectMeta<'a>,
    fields: Vec<StructField<'a>>,
}

pub(crate) struct StructField<'a> {
    pub data: &'a Field,
    pub attrs: FieldAttributes,
    pub field_index: usize,
    pub reflect_index: usize,
}

// -----------------------------------------------------------------------------
// Field Implementation

impl<'a> StructField<'a> {
    /// Generates a `TokenStream` for `NamedField` or `UnnamedField` construction.
    ///
    /// This function is only allowed to be called for active fields(self.reflection_index is some).
    pub fn to_info_tokens(&self, voker_reflect_path: &syn::Path) -> TokenStream {
        let field_info = if self.data.ident.is_some() {
            crate::path::named_field_(voker_reflect_path) // String Literal
        } else {
            crate::path::unnamed_field_(voker_reflect_path) // Num Literal
        };

        let name = match &self.data.ident {
            Some(ident) => ident.to_string().to_token_stream(), // String Literal
            None => self.field_index.to_token_stream(),
        };

        let ty = &self.data.ty;

        // See [`CustomAttributes::get_with_expression`]
        let with_custom_attributes =
            self.attrs.custom_attributes.get_with_expression(voker_reflect_path);
        // See [`ReflectDocs::get_with_expression`]
        // If feature is diabled, this function will return a empty TokenStream, so it's safe.
        let with_docs = self.attrs.docs.get_with_expression();

        let with_skip_serde = if self.attrs.skip_serde.is_some() {
            quote! { .with_skip_serde(true) }
        } else {
            TokenStream::new()
        };

        quote! {
            #field_info::new::<#ty>(#name)
                #with_skip_serde
                #with_custom_attributes
                #with_docs
        }
    }

    /// Generates a [`syn::Member`] based on this field.
    ///
    /// If the field is unnamed, the declaration index is used.
    /// This allows this member to be used for both active and ignored fields.
    pub fn to_member(&self) -> syn::Member {
        match &self.data.ident {
            Some(ident) => syn::Member::Named(ident.clone()),
            None => syn::Member::Unnamed(self.field_index.into()),
        }
    }

    /// Get the field name for the `field` function of `Struct/TupleStruct`.
    ///
    /// - Named fields return values similar to `"name"`.
    /// - Unnamedfields return values similar to `2`.
    pub fn reflect_accessor(&self) -> TokenStream {
        match &self.data.ident {
            Some(ident) => ident.to_string().to_token_stream(),
            None => self.reflect_index.to_token_stream(),
        }
    }

    pub fn is_ignore(&self) -> bool {
        self.attrs.ignore.is_some()
    }

    pub fn cloneable(&self) -> bool {
        self.attrs.clone.is_some()
    }

    pub fn defaultable(&self) -> bool {
        self.attrs.default.is_some()
    }
}

// -----------------------------------------------------------------------------
// Struct Implementation

impl<'a> ReflectStruct<'a> {
    pub fn new(meta: ReflectMeta<'a>, fields: Vec<StructField<'a>>) -> Self {
        let mut val = Self { meta, fields };
        val.meta
            .set_active_types(val.active_fields().map(|f| &f.data.ty).collect());
        val
    }

    /// Access the metadata associated with this struct definition.
    #[inline]
    pub fn meta(&self) -> &ReflectMeta<'a> {
        &self.meta
    }

    /// The complete set of fields in this struct.
    #[inline]
    pub fn fields(&self) -> &[StructField<'a>] {
        &self.fields
    }

    /// Get an iterator of fields which are exposed to the reflection API.
    pub fn active_fields(&self) -> impl Iterator<Item = &StructField<'a>> {
        // Active fields are those not marked with `#[reflect(ignore)]`.
        self.fields().iter().filter(|f| !f.is_ignore())
    }

    pub fn type_info_tokens(&self, is_tuple: bool) -> TokenStream {
        let voker_reflect_path = self.meta.voker_reflect_path();

        let type_info_path = crate::path::type_info_(voker_reflect_path);

        let type_info_kind = if is_tuple {
            Ident::new("TupleStruct", Span::call_site())
        } else {
            Ident::new("Struct", Span::call_site())
        };

        let info_struct_path = if is_tuple {
            crate::path::tuple_struct_info_(voker_reflect_path)
        } else {
            crate::path::struct_info_(voker_reflect_path)
        };

        let field_infos = self
            .active_fields()
            .map(|field| field.to_info_tokens(voker_reflect_path));

        // See [`CustomAttributes::get_with_expression`]
        let with_custom_attributes = self.meta.with_custom_attributes_expression();
        // See [`ReflectDocs::get_with_expression`]
        // If feature is diabled, this function will return a empty TokenStream, so it's safe.
        let with_docs = self.meta.with_docs_expression();
        // See [`ReflectMeta::with_generics_expression`]
        let with_generics = self.meta.with_generics_expression();

        quote! {
            #type_info_path::#type_info_kind(
                #info_struct_path::new::<Self>(&[ #(#field_infos),* ])
                    #with_generics
                    #with_custom_attributes
                    #with_docs
            )
        }
    }
}

// -----------------------------------------------------------------------------
// Field Accessor

/// A helper struct for creating field accessors.
pub(crate) struct FieldAccessors {
    /// The referenced field accessors, such as `&self.foo`.
    pub fields_ref: Vec<TokenStream>,
    /// The mutably referenced field accessors, such as `&mut self.foo`.
    pub fields_mut: Vec<TokenStream>,
    /// The ordered set of field indices (basically just the range of [0, `field_count`).
    pub field_indices: Vec<usize>,
    /// The number of fields in the reflected struct.
    pub field_count: usize,
}

impl FieldAccessors {
    pub fn new(info: &ReflectStruct) -> Self {
        let (fields_ref, fields_mut): (Vec<_>, Vec<_>) = info
            .active_fields()
            .map(|field| {
                let member = field.to_member();
                (quote!(&self.#member), quote!(&mut self.#member))
            })
            .unzip();

        let field_count = fields_ref.len();
        let field_indices = (0..field_count).collect();

        Self {
            fields_ref,
            fields_mut,
            field_indices,
            field_count,
        }
    }
}
