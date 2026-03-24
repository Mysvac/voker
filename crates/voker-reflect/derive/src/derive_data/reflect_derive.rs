use proc_macro2::Span;
use syn::{DeriveInput, Fields, Variant};
use syn::{punctuated::Punctuated, spanned::Spanned, token::Comma};

use super::{EnumVariant, EnumVariantFields, StructField};
use super::{FieldAttributes, TypeAttributes, TypeParser};
use super::{ReflectEnum, ReflectMeta, ReflectStruct};
use crate::ImplSourceKind;

pub(crate) enum ReflectDerive<'a> {
    Struct(ReflectStruct<'a>),
    TupleStruct(ReflectStruct<'a>),
    UnitStruct(ReflectMeta<'a>),
    Enum(ReflectEnum<'a>),
    Opaque(ReflectMeta<'a>),
}

impl<'a> ReflectDerive<'a> {
    pub fn from_input(input: &'a DeriveInput, source: ImplSourceKind) -> syn::Result<Self> {
        let type_attributes = TypeAttributes::parse_attrs(&input.attrs)?;
        type_attributes.validity()?;

        // For local types, can use `module_path!()` to get the module path,
        // but for foreign types, the user needs to explicitly provide it.
        // If automatic implementation is disabled, it can also be ignored.
        if source == ImplSourceKind::ImplForeignType
            && type_attributes.impl_switchs.impl_type_path
            && type_attributes.type_path.is_none()
        {
            return Err(syn::Error::new(
                input.ident.span(),
                "#[reflect(type_path = \"...\")] must be specified when auto impl TypePath for Foreign Type.",
            ));
        }

        // After meeting the above conditions, they can all be considered as local types.
        //
        // There are other algorithms for Foreign TypePath and Primitive TypePath.
        let type_parser = TypeParser::new_local(
            &input.ident,
            type_attributes.type_path.clone(),
            &input.generics,
        );

        let meta = ReflectMeta::new(type_attributes, type_parser);

        if meta.attrs().is_opaque.is_some() {
            return Ok(Self::Opaque(meta));
        }

        match &input.data {
            syn::Data::Struct(data_struct) => {
                let fields = Self::colloct_struct_field(&data_struct.fields)?;
                match data_struct.fields {
                    Fields::Named(..) => Ok(Self::Struct(ReflectStruct::new(meta, fields))),
                    Fields::Unnamed(..) => Ok(Self::TupleStruct(ReflectStruct::new(meta, fields))),
                    Fields::Unit => Ok(Self::UnitStruct(meta)),
                }
            }
            syn::Data::Enum(data_enum) => {
                let variants = Self::collect_enum_variants(&data_enum.variants)?;

                Ok(Self::Enum(ReflectEnum::new(meta, variants)))
            }
            syn::Data::Union(..) => Err(syn::Error::new(
                input.span(),
                "reflection macros do not support unions.",
            )),
        }
    }

    fn colloct_struct_field(fields: &'a Fields) -> syn::Result<Vec<StructField<'a>>> {
        if fields.len() > u16::MAX as usize {
            return Err(syn::Error::new(
                Span::call_site(),
                "the number of fields in reflected struct cannot exceed `65535`.",
            ));
        }

        let mut res: Vec<StructField<'a>> = Vec::with_capacity(fields.len());

        for (field_index, field) in fields.iter().enumerate() {
            let attrs = FieldAttributes::parse_attrs(&field.attrs)?;

            res.push(StructField {
                data: field,
                attrs,
                field_index,
            });
        }

        Ok(res)
    }

    fn collect_enum_variants(
        variants: &'a Punctuated<Variant, Comma>,
    ) -> syn::Result<Vec<EnumVariant<'a>>> {
        if variants.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                "reflection macros do not support empty enum.",
            ));
        }

        if variants.len() > u16::MAX as usize {
            return Err(syn::Error::new(
                Span::call_site(),
                "the number of variants in reflected enum cannot exceed `65535`.",
            ));
        }

        let mut res: Vec<EnumVariant<'a>> = Vec::with_capacity(variants.len());

        for variant in variants.iter() {
            let fields = Self::colloct_struct_field(&variant.fields)?;
            let variant_fields = match variant.fields {
                Fields::Named(..) => EnumVariantFields::Named(fields),
                Fields::Unnamed(..) => EnumVariantFields::Unnamed(fields),
                Fields::Unit => EnumVariantFields::Unit,
            };
            let variant_item = EnumVariant {
                data: variant,
                fields: variant_fields,
                attrs: FieldAttributes::parse_attrs(&variant.attrs)?,
            };

            res.push(variant_item);
        }

        Ok(res)
    }
}
