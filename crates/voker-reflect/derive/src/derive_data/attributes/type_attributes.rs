use proc_macro2::Span;
use syn::{Attribute, Expr, ExprLit, Lit, MacroDelimiter, Type};
use syn::{Meta, MetaNameValue, Path, Token};
use syn::{parse::ParseStream, spanned::Spanned};

use super::{CustomAttributes, ReflectDocs, TraitAvailableFlags, TraitImplSwitches};

use crate::{REFLECT_ATTRIBUTE, TYPE_DATA_ATTRIBUTE, TYPE_PATH_ATTRIBUTE};

mod kw {
    syn::custom_keyword!(TypePath);
    syn::custom_keyword!(Typed);
    syn::custom_keyword!(Reflect);
    syn::custom_keyword!(GetTypeMeta);
    syn::custom_keyword!(FromReflect);
    syn::custom_keyword!(Struct);
    syn::custom_keyword!(TupleStruct);
    syn::custom_keyword!(Tuple);
    syn::custom_keyword!(Enum);
    syn::custom_keyword!(Opaque);
    syn::custom_keyword!(Default);
    syn::custom_keyword!(Clone);
    syn::custom_keyword!(NotCloneable);
    syn::custom_keyword!(Debug);
    syn::custom_keyword!(Hash);
    syn::custom_keyword!(PartialEq);
    syn::custom_keyword!(PartialOrd);
    syn::custom_keyword!(Serialize);
    syn::custom_keyword!(Deserialize);
    syn::custom_keyword!(Into);
    syn::custom_keyword!(From);
    syn::custom_keyword!(doc);
}

#[derive(Default)]
pub(crate) struct TypeAttributes {
    /// See: [`CustomAttributes`]
    pub custom_attributes: CustomAttributes,
    /// See: [`TraitImplSwitches`]
    pub impl_switchs: TraitImplSwitches,
    /// See: [`TraitAvailableFlags`]
    pub avail_traits: TraitAvailableFlags,
    /// `#[reflect(Opaque)]`
    pub is_opaque: Option<Span>,
    /// `#[type_path = "..."]`
    pub type_path: Option<Path>,
    /// `#[reflect(doc = "...")]` or `#[doc = "..."]`
    pub docs: ReflectDocs,
    /// `#[type_data(...)]`
    pub extra_type_data: Vec<Path>,
    /// `#[reflect(Into<..>)]`
    pub into_types: Vec<Type>,
    /// `#[reflect(From<..>)]`
    pub from_types: Vec<Type>,
}

impl TypeAttributes {
    pub fn validity(&self) -> syn::Result<()> {
        if let (Some(clone_span), Some(not_cloneable_span)) =
            (self.avail_traits.clone, self.avail_traits.not_cloneable)
        {
            let span = clone_span.join(not_cloneable_span).unwrap_or(not_cloneable_span);
            return Err(syn::Error::new(
                span,
                "#[reflect(Clone)] conflicts with #[reflect(NotCloneable)].",
            ));
        }

        if let Some(span) = self.is_opaque
            && self.avail_traits.clone.is_none()
            && self.avail_traits.not_cloneable.is_none()
            && { self.impl_switchs.impl_reflect || self.impl_switchs.impl_from_reflect }
        {
            return Err(syn::Error::new(
                span,
                "#[reflect(Clone)] or #[reflect(NotCloneable)] must be specified \
                when auto impl `Reflect` or `FromReflect` for Opaque Type.",
            ));
        }
        Ok(())
    }

    pub fn parse_type_path(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut type_attributes = TypeAttributes {
            impl_switchs: TraitImplSwitches::empty(),
            ..Default::default()
        };

        type_attributes.impl_switchs.impl_type_path = true;

        for attribute in attrs {
            match &attribute.meta {
                Meta::NameValue(pair) if pair.path.is_ident(TYPE_PATH_ATTRIBUTE) => {
                    type_attributes.parse_type_path_impl(pair)?;
                }
                _ => continue,
            }
        }
        Ok(type_attributes)
    }

    pub fn parse_attrs(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut type_attributes = TypeAttributes::default();

        for attribute in attrs {
            match &attribute.meta {
                Meta::List(meta_list) if meta_list.path.is_ident(REFLECT_ATTRIBUTE) => {
                    if !matches!(&meta_list.delimiter, MacroDelimiter::Paren(_)) {
                        return Err(syn::Error::new(
                            meta_list.delimiter.span().join(),
                            format_args!(
                                "`#[{REFLECT_ATTRIBUTE}(\"...\")]` must use parentheses `(` and `)`"
                            ),
                        ));
                    }

                    meta_list.parse_args_with(|stream: ParseStream| {
                        type_attributes.parse_stream(stream)
                    })?;
                }
                Meta::List(meta_list) if meta_list.path.is_ident(TYPE_DATA_ATTRIBUTE) => {
                    if !matches!(&meta_list.delimiter, MacroDelimiter::Paren(_)) {
                        return Err(syn::Error::new(
                            meta_list.delimiter.span().join(),
                            format_args!(
                                "`#[{TYPE_DATA_ATTRIBUTE}(\"...\")]` must use parentheses `(` and `)`"
                            ),
                        ));
                    }

                    meta_list.parse_args_with(|stream: ParseStream| {
                        type_attributes.parse_type_data_args(stream)
                    })?;
                }
                Meta::NameValue(pair) if pair.path.is_ident(TYPE_PATH_ATTRIBUTE) => {
                    type_attributes.parse_type_path_impl(pair)?;
                }
                Meta::NameValue(pair)
                    if ::core::cfg!(feature = "reflect_docs") && pair.path.is_ident("doc") =>
                {
                    type_attributes.docs.parse_default_docs(pair)?;
                }
                _ => continue,
            }
        }
        Ok(type_attributes)
    }

    pub fn parse_stream(&mut self, stream: ParseStream) -> syn::Result<()> {
        loop {
            if stream.is_empty() {
                break;
            }
            self.parse_stream_internal(stream)?;
            if stream.is_empty() {
                break;
            }
            stream.parse::<Token![,]>()?;
        }
        Ok(())
    }

    fn parse_stream_internal(&mut self, input: ParseStream) -> syn::Result<()> {
        let lookahead = input.lookahead1();
        if lookahead.peek(Token![@]) {
            self.parse_custom_attribute_impl(input)
        } else if lookahead.peek(kw::Clone) {
            self.parse_clone(input)
        } else if lookahead.peek(kw::NotCloneable) {
            self.parse_not_cloneable(input)
        } else if lookahead.peek(kw::Default) {
            self.parse_default_impl(input)
        } else if lookahead.peek(kw::Hash) {
            self.parse_hash_impl(input)
        } else if lookahead.peek(kw::PartialEq) {
            self.parse_partial_eq_impl(input)
        } else if lookahead.peek(kw::PartialOrd) {
            self.parse_partial_ord_impl(input)
        } else if lookahead.peek(kw::Debug) {
            self.parse_debug_impl(input)
        } else if lookahead.peek(kw::Serialize) {
            self.parse_serialize_impl(input)
        } else if lookahead.peek(kw::Deserialize) {
            self.parse_deserialize_impl(input)
        } else if lookahead.peek(kw::Into) {
            self.parse_into_impl(input)
        } else if lookahead.peek(kw::From) {
            self.parse_from_impl(input)
        } else if lookahead.peek(kw::Opaque) {
            self.parse_opaque_impl(input)
        } else if lookahead.peek(kw::TypePath) {
            self.parse_trait_type_path(input)
        } else if lookahead.peek(kw::Typed) {
            self.parse_trait_typed(input)
        } else if lookahead.peek(kw::Reflect) {
            self.parse_trait_reflect(input)
        } else if lookahead.peek(kw::GetTypeMeta) {
            self.parse_trait_get_type_meta(input)
        } else if lookahead.peek(kw::FromReflect) {
            self.parse_trait_from_reflect(input)
        } else if lookahead.peek(kw::Struct) {
            self.parse_trait_struct(input)
        } else if lookahead.peek(kw::TupleStruct) {
            self.parse_trait_tuple_struct(input)
        } else if lookahead.peek(kw::Tuple) {
            self.parse_trait_tuple(input)
        } else if lookahead.peek(kw::Enum) {
            self.parse_trait_enum(input)
        } else if lookahead.peek(kw::doc) {
            self.parse_docs_impl(input)
        } else {
            Err(lookahead.error())
        }
    }

    // #[reflect(@expr)]
    fn parse_custom_attribute_impl(&mut self, input: ParseStream) -> syn::Result<()> {
        self.custom_attributes.parse_stream(input)
    }

    // #[reflect(docs = "...")]
    fn parse_docs_impl(&mut self, input: ParseStream) -> syn::Result<()> {
        let pair = input.parse::<MetaNameValue>()?;
        if ::core::cfg!(feature = "reflect_docs") {
            self.docs.parse_custom_docs(&pair)
        } else {
            Ok(())
        }
    }

    // #[reflect(Default)]
    fn parse_default_impl(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::Default>()?.span;
        self.avail_traits.default = Some(s);
        Ok(())
    }

    // #[reflect(Clone)]
    fn parse_clone(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::Clone>()?.span;
        self.avail_traits.clone = Some(s);
        Ok(())
    }

    // #[reflect(NotCloneable)]
    fn parse_not_cloneable(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::NotCloneable>()?.span;
        self.avail_traits.not_cloneable = Some(s);
        Ok(())
    }

    // #[reflect(Hash)]
    fn parse_hash_impl(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::Hash>()?.span;
        self.avail_traits.hash = Some(s);
        Ok(())
    }

    // #[reflect(PartialEq)]
    fn parse_partial_eq_impl(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::PartialEq>()?.span;
        self.avail_traits.partial_eq = Some(s);
        Ok(())
    }

    // #[reflect(PartialOrd)]
    fn parse_partial_ord_impl(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::PartialOrd>()?.span;
        self.avail_traits.partial_ord = Some(s);
        Ok(())
    }

    // #[reflect(Debug)]
    fn parse_debug_impl(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::Debug>()?.span;
        self.avail_traits.debug = Some(s);
        Ok(())
    }

    // #[reflect(Serialize)]
    fn parse_serialize_impl(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::Serialize>()?.span;
        self.avail_traits.serialize = Some(s);
        Ok(())
    }

    // #[reflect(Deserialize)]
    fn parse_deserialize_impl(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::Deserialize>()?.span;
        self.avail_traits.deserialize = Some(s);
        Ok(())
    }

    // #[reflect(Opaque)]
    fn parse_opaque_impl(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::Opaque>()?.span;
        self.is_opaque = Some(s);
        Ok(())
    }

    // #[type_path = "..."]
    fn parse_type_path_impl(&mut self, pair: &MetaNameValue) -> syn::Result<()> {
        let Expr::Lit(ExprLit {
            lit: Lit::Str(lit), ..
        }) = &pair.value
        else {
            return Err(syn::Error::new(
                pair.value.span(),
                "Expected a string literal value.",
            ));
        };

        let path: Path = syn::parse_str(&lit.value())?;
        if path.segments.is_empty() {
            return Err(syn::Error::new(
                lit.span(),
                "`type_path` should not be empty.",
            ));
        }
        if path.leading_colon.is_some() {
            return Err(syn::Error::new(
                lit.span(),
                "`type_path` should not have leading-colon.",
            ));
        }

        self.type_path = Some(path);
        Ok(())
    }

    // #[reflect(TypePath = false)]
    fn parse_trait_type_path(&mut self, input: ParseStream) -> syn::Result<()> {
        // #[reflect(TypePath = false)]
        let pair = input.parse::<MetaNameValue>()?;

        if let Expr::Lit(ExprLit {
            lit: Lit::Bool(lit),
            ..
        }) = &pair.value
        {
            if lit.value() {
                return Err(syn::Error::new(
                    lit.span(),
                    "Should not be `true`, it's default value.",
                ));
            }
            self.impl_switchs.impl_type_path = lit.value();
        } else {
            return Err(syn::Error::new(pair.value.span(), "Expected a bool value."));
        }

        Ok(())
    }

    fn parse_trait_typed(&mut self, input: ParseStream) -> syn::Result<()> {
        // #[reflect(Typed = false)]
        let pair = input.parse::<MetaNameValue>()?;

        if let Expr::Lit(ExprLit {
            lit: Lit::Bool(lit),
            ..
        }) = &pair.value
        {
            if lit.value() {
                return Err(syn::Error::new(
                    lit.span(),
                    "Should not be `true`, it's default value.",
                ));
            }
            self.impl_switchs.impl_typed = lit.value();
        } else {
            return Err(syn::Error::new(pair.value.span(), "Expected a bool value."));
        }

        Ok(())
    }

    fn parse_trait_reflect(&mut self, input: ParseStream) -> syn::Result<()> {
        // #[reflect(Reflecct = false)]
        let pair = input.parse::<MetaNameValue>()?;

        if let Expr::Lit(ExprLit {
            lit: Lit::Bool(lit),
            ..
        }) = &pair.value
        {
            if lit.value() {
                return Err(syn::Error::new(
                    lit.span(),
                    "Should not be `true`, it's default value.",
                ));
            }
            self.impl_switchs.impl_reflect = lit.value();
        } else {
            return Err(syn::Error::new(pair.value.span(), "Expected a bool value."));
        }

        Ok(())
    }

    fn parse_trait_get_type_meta(&mut self, input: ParseStream) -> syn::Result<()> {
        // #[reflect(GetTypeMeta = false)]
        let pair = input.parse::<MetaNameValue>()?;

        if let Expr::Lit(ExprLit {
            lit: Lit::Bool(lit),
            ..
        }) = &pair.value
        {
            if lit.value() {
                return Err(syn::Error::new(
                    lit.span(),
                    "Should not be `true`, it's default value.",
                ));
            }
            self.impl_switchs.impl_get_type_meta = lit.value();
        } else {
            return Err(syn::Error::new(pair.value.span(), "Expected a bool value."));
        }

        Ok(())
    }

    fn parse_trait_from_reflect(&mut self, input: ParseStream) -> syn::Result<()> {
        // #[reflect(FromReflect = false)]
        let pair = input.parse::<MetaNameValue>()?;

        if let Expr::Lit(ExprLit {
            lit: Lit::Bool(lit),
            ..
        }) = &pair.value
        {
            if lit.value() {
                return Err(syn::Error::new(
                    lit.span(),
                    "Should not be `true`, it's default value.",
                ));
            }
            self.impl_switchs.impl_from_reflect = lit.value();
        } else {
            return Err(syn::Error::new(pair.value.span(), "Expected a bool value."));
        }

        Ok(())
    }

    fn parse_trait_struct(&mut self, input: ParseStream) -> syn::Result<()> {
        // #[reflect(Struct = false)]
        let pair = input.parse::<MetaNameValue>()?;

        if let Expr::Lit(ExprLit {
            lit: Lit::Bool(lit),
            ..
        }) = &pair.value
        {
            if lit.value() {
                return Err(syn::Error::new(
                    lit.span(),
                    "Should not be `true`, it's default value.",
                ));
            }
            self.impl_switchs.impl_struct = lit.value();
        } else {
            return Err(syn::Error::new(pair.value.span(), "Expected a bool value."));
        }

        Ok(())
    }

    fn parse_trait_tuple_struct(&mut self, input: ParseStream) -> syn::Result<()> {
        // #[reflect(TupleStruct = false)]
        let pair = input.parse::<MetaNameValue>()?;

        if let Expr::Lit(ExprLit {
            lit: Lit::Bool(lit),
            ..
        }) = &pair.value
        {
            if lit.value() {
                return Err(syn::Error::new(
                    lit.span(),
                    "Should not be `true`, it's default value.",
                ));
            }
            self.impl_switchs.impl_tuple_struct = lit.value();
        } else {
            return Err(syn::Error::new(pair.value.span(), "Expected a bool value."));
        }

        Ok(())
    }

    fn parse_trait_tuple(&mut self, input: ParseStream) -> syn::Result<()> {
        // #[reflect(Tuple = false)]
        let pair = input.parse::<MetaNameValue>()?;

        if let Expr::Lit(ExprLit {
            lit: Lit::Bool(lit),
            ..
        }) = &pair.value
        {
            if lit.value() {
                return Err(syn::Error::new(
                    lit.span(),
                    "Should not be `true`, it's default value.",
                ));
            }
            self.impl_switchs.impl_tuple = lit.value();
        } else {
            return Err(syn::Error::new(pair.value.span(), "Expected a bool value."));
        }

        Ok(())
    }

    fn parse_trait_enum(&mut self, input: ParseStream) -> syn::Result<()> {
        // #[reflect(Enum = false)]
        let pair = input.parse::<MetaNameValue>()?;

        if let Expr::Lit(ExprLit {
            lit: Lit::Bool(lit),
            ..
        }) = &pair.value
        {
            if lit.value() {
                return Err(syn::Error::new(
                    lit.span(),
                    "Should not be `true`, it's default value.",
                ));
            }
            self.impl_switchs.impl_enum = lit.value();
        } else {
            return Err(syn::Error::new(pair.value.span(), "Expected a bool value."));
        }

        Ok(())
    }

    fn parse_type_data_args(&mut self, stream: ParseStream) -> syn::Result<()> {
        if stream.is_empty() {
            return Err(syn::Error::new(
                stream.span(),
                "Expected at least one type data path.",
            ));
        }

        loop {
            self.extra_type_data.push(stream.parse::<Path>()?);
            if stream.is_empty() {
                break;
            }
            stream.parse::<Token![,]>()?;
        }

        Ok(())
    }

    // #[reflect(Into<Type>)]
    fn parse_into_impl(&mut self, input: ParseStream) -> syn::Result<()> {
        input.parse::<kw::Into>()?;

        input.parse::<Token![<]>()?;
        let ty: Type = input.parse()?;
        input.parse::<Token![>]>()?;

        self.into_types.push(ty);

        Ok(())
    }

    // #[reflect(From<Type>)]
    fn parse_from_impl(&mut self, input: ParseStream) -> syn::Result<()> {
        input.parse::<kw::From>()?;

        input.parse::<Token![<]>()?;
        let ty: Type = input.parse()?;
        input.parse::<Token![>]>()?;

        self.from_types.push(ty);

        Ok(())
    }
}
