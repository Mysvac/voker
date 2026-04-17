use proc_macro2::Span;
use syn::{Attribute, Expr, ExprLit, Lit, MacroDelimiter};
use syn::{Meta, MetaNameValue, Path, Token};
use syn::{parse::ParseStream, spanned::Spanned};

use super::{CustomAttributes, ReflectDocs, TraitAvailableFlags, TraitImplSwitches};

use crate::REFLECT_ATTRIBUTE;

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
    syn::custom_keyword!(Debug);
    syn::custom_keyword!(Hash);
    syn::custom_keyword!(PartialEq);
    syn::custom_keyword!(PartialOrd);
    syn::custom_keyword!(Serialize);
    syn::custom_keyword!(Deserialize);
    syn::custom_keyword!(FromWorld);
    syn::custom_keyword!(Component);
    syn::custom_keyword!(Resource);
    syn::custom_keyword!(type_path);
    syn::custom_keyword!(doc);
    syn::custom_keyword!(type_data);
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
    /// `#[reflect(type_path = "...")]`
    pub type_path: Option<Path>,
    /// `#[reflect(doc = "...")]` or `#[doc = "..."]`
    pub docs: ReflectDocs,
    /// `#[reflect(type_data = (...))]`
    pub extra_type_data: Vec<Path>,
}

impl TypeAttributes {
    pub fn validity(&self) -> syn::Result<()> {
        if let Some(span) = self.is_opaque
            && self.avail_traits.clone.is_none()
            && { self.impl_switchs.impl_reflect || self.impl_switchs.impl_from_reflect }
        {
            return Err(syn::Error::new(
                span,
                "#[reflect(Clone)] must be specified when auto impl `Reflect` or `FromReflect` for Opaque Type.",
            ));
        }
        Ok(())
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
            self.parse_custom_attribute(input)
        } else if lookahead.peek(kw::doc) {
            self.parse_docs(input)
        } else if lookahead.peek(kw::Clone) {
            self.parse_clone(input)
        } else if lookahead.peek(kw::Default) {
            self.parse_default(input)
        } else if lookahead.peek(kw::Hash) {
            self.parse_hash(input)
        } else if lookahead.peek(kw::PartialEq) {
            self.parse_partial_eq(input)
        } else if lookahead.peek(kw::PartialOrd) {
            self.parse_partial_ord(input)
        } else if lookahead.peek(kw::Debug) {
            self.parse_debug(input)
        } else if lookahead.peek(kw::Serialize) {
            self.parse_serialize(input)
        } else if lookahead.peek(kw::Deserialize) {
            self.parse_deserialize(input)
        } else if lookahead.peek(kw::FromWorld) {
            self.parse_from_world(input)
        } else if lookahead.peek(kw::Component) {
            self.parse_component(input)
        } else if lookahead.peek(kw::Resource) {
            self.parse_resource(input)
        } else if lookahead.peek(kw::Opaque) {
            self.parse_opaque(input)
        } else if lookahead.peek(kw::type_path) {
            self.parse_type_path(input)
        } else if lookahead.peek(kw::type_data) {
            self.parses_extra_type_data(input)
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
        } else {
            Err(lookahead.error())
        }
    }

    // #[reflect(@expr)]
    fn parse_custom_attribute(&mut self, input: ParseStream) -> syn::Result<()> {
        self.custom_attributes.parse_stream(input)
    }

    // #[reflect(docs = "...")]
    fn parse_docs(&mut self, input: ParseStream) -> syn::Result<()> {
        let pair = input.parse::<MetaNameValue>()?;
        if ::core::cfg!(feature = "reflect_docs") {
            self.docs.parse_custom_docs(&pair)
        } else {
            Ok(())
        }
    }

    // #[reflect(Default)]
    fn parse_default(&mut self, input: ParseStream) -> syn::Result<()> {
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

    // #[reflect(Hash)]
    fn parse_hash(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::Hash>()?.span;
        self.avail_traits.hash = Some(s);
        Ok(())
    }

    // #[reflect(PartialEq)]
    fn parse_partial_eq(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::PartialEq>()?.span;
        self.avail_traits.partial_eq = Some(s);
        Ok(())
    }

    // #[reflect(PartialOrd)]
    fn parse_partial_ord(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::PartialOrd>()?.span;
        self.avail_traits.partial_ord = Some(s);
        Ok(())
    }

    // #[reflect(Debug)]
    fn parse_debug(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::Debug>()?.span;
        self.avail_traits.debug = Some(s);
        Ok(())
    }

    // #[reflect(Serialize)]
    fn parse_serialize(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::Serialize>()?.span;
        self.avail_traits.serialize = Some(s);
        Ok(())
    }

    // #[reflect(Deserialize)]
    fn parse_deserialize(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::Deserialize>()?.span;
        self.avail_traits.deserialize = Some(s);
        Ok(())
    }

    // #[reflect(FromWorld)]
    fn parse_from_world(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::FromWorld>()?.span;
        self.avail_traits.from_world = Some(s);
        Ok(())
    }

    // #[reflect(Component)]
    fn parse_component(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::Component>()?.span;
        self.avail_traits.component = Some(s);
        Ok(())
    }

    // #[reflect(Resource)]
    fn parse_resource(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::Resource>()?.span;
        self.avail_traits.resource = Some(s);
        Ok(())
    }

    // #[reflect(Opaque)]
    fn parse_opaque(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::Opaque>()?.span;
        self.is_opaque = Some(s);
        Ok(())
    }

    // #[reflect(type_path = "...")]
    fn parse_type_path(&mut self, input: ParseStream) -> syn::Result<()> {
        let pair = input.parse::<MetaNameValue>()?;

        if let Expr::Lit(ExprLit {
            lit: Lit::Str(lit), ..
        }) = &pair.value
        {
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
        } else {
            return Err(syn::Error::new(
                pair.value.span(),
                "Expected a string liternal value.",
            ));
        }

        Ok(())
    }

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

    fn parses_extra_type_data(&mut self, input: ParseStream) -> syn::Result<()> {
        let pair = input.parse::<MetaNameValue>()?;

        if let Expr::Tuple(tuple) = &pair.value {
            for elem in &tuple.elems {
                if let Expr::Path(expr_path) = elem {
                    self.extra_type_data.push(expr_path.path.clone());
                } else {
                    return Err(syn::Error::new(elem.span(), "Expected a path in tuple."));
                }
            }
        } else if let Expr::Path(expr_path) = &pair.value {
            self.extra_type_data.push(expr_path.path.clone());
        } else {
            return Err(syn::Error::new(
                pair.value.span(),
                "Expected a path or tuple of paths.",
            ));
        }
        Ok(())
    }
}
