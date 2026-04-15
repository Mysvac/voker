use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use quote::format_ident;
use quote::quote;
use syn::DeriveInput;
use syn::Token;
use syn::parse_quote;
use syn::spanned::Spanned;
use syn::{Type, parse::Parse};

// -----------------------------------------------------------------------------
// Relationship & RelationshipTarget

struct Relationship {
    relationship_target: Type,
    allow_self_referential: bool,
}

struct RelationshipTarget {
    relationship: Type,
    linked_lifecycle: bool,
}

impl Parse for Relationship {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        mod kw {
            syn::custom_keyword!(relationship_target);
            syn::custom_keyword!(allow_self_referential);
        }

        let mut relationship_target: Option<Type> = None;
        let mut allow_self_referential: bool = false;

        while !input.is_empty() {
            let lookahead = input.lookahead1();
            if lookahead.peek(kw::relationship_target) {
                input.parse::<kw::relationship_target>()?;
                input.parse::<Token![=]>()?;
                relationship_target = Some(input.parse()?);
            } else if lookahead.peek(kw::allow_self_referential) {
                input.parse::<kw::allow_self_referential>()?;
                if input.peek(Token![=]) {
                    input.parse::<Token![=]>()?;
                    allow_self_referential = input.parse::<syn::LitBool>()?.value();
                } else {
                    allow_self_referential = true;
                }
            } else {
                return Err(lookahead.error());
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Relationship {
            relationship_target: relationship_target.ok_or_else(|| {
                syn::Error::new(input.span(), "Missing `relationship_target = T`.")
            })?,
            allow_self_referential,
        })
    }
}

impl Parse for RelationshipTarget {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        mod kw {
            syn::custom_keyword!(relationship);
            syn::custom_keyword!(linked_lifecycle);
        }

        let mut relationship: Option<Type> = None;
        let mut linked_lifecycle: bool = false;

        while !input.is_empty() {
            let lookahead = input.lookahead1();
            if lookahead.peek(kw::relationship) {
                input.parse::<kw::relationship>()?;
                input.parse::<Token![=]>()?;
                relationship = Some(input.parse()?);
            } else if lookahead.peek(kw::linked_lifecycle) {
                input.parse::<kw::linked_lifecycle>()?;
                if input.peek(Token![=]) {
                    input.parse::<Token![=]>()?;
                    linked_lifecycle = input.parse::<syn::LitBool>()?.value();
                } else {
                    linked_lifecycle = true;
                }
            } else {
                return Err(lookahead.error());
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(RelationshipTarget {
            relationship: relationship
                .ok_or_else(|| syn::Error::new(input.span(), "Missing `relationship = T`"))?,
            linked_lifecycle,
        })
    }
}

fn parse_related_field(fields: &syn::Fields, span: Span) -> syn::Result<&syn::Field> {
    match fields {
        syn::Fields::Named(fields) => {
            let mut ret: Option<&syn::Field> = None;

            for field in fields.named.iter() {
                if field.attrs.iter().any(|attr| attr.path().is_ident("related"))
                    && ret.replace(field).is_some()
                {
                    return Err(syn::Error::new(
                        span,
                        "#[related] can only annotate in one field.",
                    ));
                }
            }

            let Some(ret) = ret else {
                return Err(syn::Error::new(
                    span,
                    "`Relationship` derive expected with a field annotated with #[related].",
                ));
            };

            Ok(ret)
        }
        syn::Fields::Unnamed(fields) => {
            if fields.unnamed.len() != 1 {
                return Err(syn::Error::new(
                    span,
                    "`Relationship` derive expected named struct or **newtype** unnamed struct.",
                ));
            }

            let field = fields.unnamed.get(0).unwrap();

            if !field.attrs.iter().any(|attr| attr.path().is_ident("related")) {
                return Err(syn::Error::new(
                    span,
                    "`Relationship` derive expected with a field annotated with #[related].",
                ));
            }

            Ok(fields.unnamed.get(0).unwrap())
        }
        syn::Fields::Unit => Err(syn::Error::new(
            span,
            "`Relationship` derive expected named or unnamed struct, found unit struct.",
        )),
    }
}

// -----------------------------------------------------------------------------
// Attributes

enum Cloner {
    Default,
    Copy,
    Clone,
    Relationship,
    RelationshipTarget,
    Custom(syn::ExprPath),
}

enum Storage {
    Dense,
    Sparse,
}

struct Attributes {
    mutable: bool,
    no_entity: bool,
    cloner: Cloner,
    storage: Storage,
    required: Option<Type>,
    on_add: Option<syn::ExprPath>,
    on_clone: Option<syn::ExprPath>,
    on_insert: Option<syn::ExprPath>,
    on_remove: Option<syn::ExprPath>,
    on_discard: Option<syn::ExprPath>,
    on_despawn: Option<syn::ExprPath>,
    relationship: Option<Relationship>,
    relationship_target: Option<RelationshipTarget>,
}

fn parse_hook_path(
    meta: &syn::meta::ParseNestedMeta,
    default: &'static str,
) -> syn::Result<syn::ExprPath> {
    if meta.input.peek(syn::Token![=]) {
        let value = meta.value()?;
        value.parse::<syn::ExprPath>()
    } else {
        syn::parse_str::<syn::ExprPath>(default)
    }
}

fn parse_attributes(attrs: &[syn::Attribute]) -> syn::Result<Attributes> {
    let mut ret = Attributes {
        mutable: true,
        no_entity: false,
        storage: Storage::Dense,
        cloner: Cloner::Default,
        required: None,
        on_add: None,
        on_clone: None,
        on_insert: None,
        on_remove: None,
        on_discard: None,
        on_despawn: None,
        relationship: None,
        relationship_target: None,
    };

    for attr in attrs {
        if attr.path().is_ident("component") {
            // -------------------------------------------------------------
            // #[component(...)]
            // -------------------------------------------------------------
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("storage") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    ret.storage = match lit.value().as_str() {
                        "sparse" => Storage::Sparse,
                        "dense" => Storage::Dense,
                        other => return Err(meta.error(format!(
                            "unsupported storage type `{other}`, expected \"dense\" or \"sparse\"."
                        ))),
                    };
                    Ok(())
                } else if meta.path.is_ident("mutable") {
                    if meta.input.peek(syn::Token![=]) {
                        let value = meta.value()?;
                        let lit: syn::LitBool = value.parse()?;
                        ret.mutable = lit.value;
                    } else {
                        ret.mutable = true;
                    }
                    Ok(())
                } else if meta.path.is_ident("no_entity") {
                    if meta.input.peek(syn::Token![=]) {
                        let value = meta.value()?;
                        let lit: syn::LitBool = value.parse()?;
                        ret.no_entity = lit.value;
                    } else {
                        ret.no_entity = true;
                    }
                    Ok(())
                } else if meta.path.is_ident("required") {
                    let value = meta.value()?;
                    ret.required = Some(value.parse()?);
                    Ok(())
                } else if meta.path.is_ident("on_add") {
                    ret.on_add = Some(parse_hook_path(&meta, "Self::on_add")?);
                    Ok(())
                } else if meta.path.is_ident("on_clone") {
                    ret.on_clone = Some(parse_hook_path(&meta, "Self::on_clone")?);
                    Ok(())
                } else if meta.path.is_ident("on_insert") {
                    ret.on_insert = Some(parse_hook_path(&meta, "Self::on_insert")?);
                    Ok(())
                } else if meta.path.is_ident("on_remove") {
                    ret.on_remove = Some(parse_hook_path(&meta, "Self::on_remove")?);
                    Ok(())
                } else if meta.path.is_ident("on_discard") {
                    ret.on_discard = Some(parse_hook_path(&meta, "Self::on_discard")?);
                    Ok(())
                } else if meta.path.is_ident("on_despawn") {
                    ret.on_despawn = Some(parse_hook_path(&meta, "Self::on_despawn")?);
                    Ok(())
                } else if meta.path.is_ident("cloner") {
                    if meta.input.peek(syn::Token![=]) {
                        let value = meta.value()?;
                        ret.cloner = Cloner::Custom(value.parse::<syn::ExprPath>()?);
                        Ok(())
                    } else {
                        Err(meta.error(
                            "unsupported cloner syntax, expected `cloner = path::function`.",
                        ))
                    }
                } else if meta.path.is_ident("Clone") {
                    match &ret.cloner {
                        Cloner::Default => {
                            ret.cloner = Cloner::Clone;
                            Ok(())
                        }
                        Cloner::Copy => {
                            Err(meta.error("Conflict cloner config `Clone` and `Copy`."))
                        }
                        Cloner::Clone => Err(meta.error("Duplicated cloner config `Clone`.")),
                        Cloner::Relationship => {
                            Err(meta
                                .error("Relationship has default cloner, cannot annotate `Clone`."))
                        }
                        Cloner::RelationshipTarget => Err(meta.error(
                            "RelationshipTarget has default cloner, cannot annotate `Clone`.",
                        )),
                        Cloner::Custom(_) => Err(meta
                            .error("Conflict cloner config `Clone` and `cloner = path::function.")),
                    }
                } else if meta.path.is_ident("Copy") {
                    match &ret.cloner {
                        Cloner::Default => {
                            ret.cloner = Cloner::Copy;
                            Ok(())
                        }
                        Cloner::Copy => Err(meta.error("Duplicated cloner config `Copy`.")),
                        Cloner::Clone => {
                            Err(meta.error("Conflict cloner config `Clone` and `Copy`."))
                        }
                        Cloner::Relationship => {
                            Err(meta
                                .error("Relationship has default cloner, cannot annotate `Copy`."))
                        }
                        Cloner::RelationshipTarget => Err(meta.error(
                            "RelationshipTarget has default cloner, cannot annotate `Copy`.",
                        )),
                        Cloner::Custom(_) => Err(meta
                            .error("Conflict cloner config `Copy` and `cloner = path::function.")),
                    }
                } else {
                    Err(meta.error("Unsupported attribute"))
                }
            })?;
        } else if attr.path().is_ident("relationship") {
            // -------------------------------------------------------------
            // #[relationship(...)]
            // -------------------------------------------------------------
            ret.relationship = Some(attr.parse_args::<Relationship>()?);
            match &ret.cloner {
                Cloner::Default => {
                    ret.cloner = Cloner::Relationship;
                }
                Cloner::Copy => {
                    return Err(syn::Error::new(
                        Span::call_site(),
                        "Relationship has default cloner, cannot annotate `Copy`.",
                    ));
                }
                Cloner::Clone => {
                    return Err(syn::Error::new(
                        Span::call_site(),
                        "Relationship has default cloner, cannot annotate `Clone`.",
                    ));
                }
                Cloner::RelationshipTarget => {
                    return Err(syn::Error::new(
                        Span::call_site(),
                        "Relationship and Target cannot be implemented for same type.",
                    ));
                }
                _ => {}
            }
        } else if attr.path().is_ident("relationship_target") {
            // -------------------------------------------------------------
            // #[relationship_target(...)]
            // -------------------------------------------------------------
            ret.relationship_target = Some(attr.parse_args::<RelationshipTarget>()?);
            match &ret.cloner {
                Cloner::Default => {
                    ret.cloner = Cloner::RelationshipTarget;
                }
                Cloner::Copy => {
                    return Err(syn::Error::new(
                        Span::call_site(),
                        "RelationshipTarget has default cloner, cannot annotate `Copy`.",
                    ));
                }
                Cloner::Clone => {
                    return Err(syn::Error::new(
                        Span::call_site(),
                        "RelationshipTarget has default cloner, cannot annotate `Clone`.",
                    ));
                }
                Cloner::RelationshipTarget => {
                    return Err(syn::Error::new(
                        Span::call_site(),
                        "Relationship and Target cannot be implemented for same type.",
                    ));
                }
                _ => {}
            }
        }
    }

    Ok(ret)
}

// -----------------------------------------------------------------------------
// map_entities_tokens

fn map_entities_tokens(
    data: &syn::Data,
    attrs: &Attributes,
    voker_ecs_path: &syn::Path,
) -> syn::Result<TokenStream2> {
    let map_entities_ = crate::path::map_entities_(voker_ecs_path);
    let entity_mapper_ = crate::path::entity_mapper_(voker_ecs_path);

    let internal = match data {
        syn::Data::Struct(syn::DataStruct { fields, .. }) => {
            let mut map = Vec::with_capacity(fields.len());

            for (index, field) in fields.iter().enumerate() {
                if field
                    .attrs
                    .iter()
                    .map(syn::Attribute::path)
                    .any(|p| p.is_ident("entities") || p.is_ident("related"))
                {
                    let field_member = field
                        .ident
                        .clone()
                        .map_or(syn::Member::from(index), syn::Member::Named);

                    map.push(quote!(#map_entities_::map_entities(&mut __this__.#field_member, __mapper__);));
                }
            }

            if map.is_empty() {
                return Ok(TokenStream2::new());
            };

            quote!(#(#map)*)
        }
        syn::Data::Enum(syn::DataEnum { variants, .. }) => {
            let mut map = Vec::with_capacity(variants.len());

            for variant in variants.iter() {
                let variant_ident = &variant.ident;
                let mut field_members: Vec<syn::Member> = Vec::with_capacity(variant.fields.len());

                for (index, field) in variant.fields.iter().enumerate() {
                    if field.attrs.iter().any(|a| a.path().is_ident("entities")) {
                        let field_member = field
                            .ident
                            .clone()
                            .map_or(syn::Member::from(index), syn::Member::Named);

                        field_members.push(field_member);
                    }
                }

                let mut field_idents = Vec::with_capacity(field_members.len());
                for member in field_members.iter() {
                    field_idents.push(format_ident!("__self{}", member));
                }

                map.push(quote! {
                    Self::#variant_ident { #(#field_members: #field_idents,)* .. } => {
                        #(#map_entities_::map_entities(#field_idents, __mapper__);)*
                    }
                });
            }

            if map.is_empty() {
                return Ok(TokenStream2::new());
            };

            quote!(
                match __this__ {
                    #(#map,)*
                    _ => {}
                }
            )
        }
        syn::Data::Union(_) => return Ok(TokenStream2::new()),
    };

    if attrs.no_entity {
        return Err(syn::Error::new(
            Span::call_site(),
            "The type is annotated `no_entity` but actually contains entity.",
        ));
    }

    Ok(quote! {
        fn map_entities<__MAPPER__: #entity_mapper_>(__this__: &mut Self, __mapper__: &mut __MAPPER__) {
            #internal
        }
    })
}

// -----------------------------------------------------------------------------
// relationship_registrar_tokens

fn relationship_registrar_tokens(
    _ast: &DeriveInput,
    attrs: &Attributes,
    voker_ecs_path: &syn::Path,
) -> syn::Result<TokenStream2> {
    use crate::path::fp::OptionFP;

    if attrs.relationship.is_none() && attrs.relationship_target.is_none() {
        return Ok(TokenStream2::new());
    }

    let relationship_registrar_ = crate::path::relationship_registrar_(voker_ecs_path);

    if attrs.relationship.is_some() {
        Ok(quote! {
            const RELATIONSHIP_REGISTRAR: #OptionFP<#relationship_registrar_> = #OptionFP::Some(
                #relationship_registrar_::relationship::<Self>()
            );
        })
    } else {
        Ok(quote! {
            const RELATIONSHIP_REGISTRAR: #OptionFP<#relationship_registrar_> = #OptionFP::Some(
                #relationship_registrar_::relationship_target::<Self>()
            );
        })
    }
}

// -----------------------------------------------------------------------------
// relationship_tokens

fn relationship_tokens(
    ast: &DeriveInput,
    attrs: &Attributes,
    voker_ecs_path: &syn::Path,
) -> syn::Result<TokenStream2> {
    let Some(relationship) = &attrs.relationship else {
        return Ok(TokenStream2::new());
    };

    let syn::Data::Struct(syn::DataStruct {
        fields,
        struct_token,
        ..
    }) = &ast.data
    else {
        return Err(syn::Error::new(
            ast.span(),
            "Link can only be derived for structs.",
        ));
    };

    let related_field = parse_related_field(fields, struct_token.span())?;

    let link_member = related_field
        .ident
        .clone()
        .map_or(syn::Member::from(0), syn::Member::Named);

    let members = fields.members().filter(|member| member != &link_member);

    let relationship_ = crate::path::relationship_(voker_ecs_path);
    let entity_ = crate::path::entity_(voker_ecs_path);
    let relationship_target = &relationship.relationship_target;
    let allow_self_referential = &relationship.allow_self_referential;

    let struct_name = &ast.ident;

    let (impl_generics, type_generics, where_clause) = ast.generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics #relationship_ for #struct_name #type_generics #where_clause {
            type RelationshipTarget = #relationship_target;

            const TARGET_FIELD_OFFSET: usize = ::core::mem::offset_of!(Self, #link_member);
            const ALLOW_SELF_REFERENTIAL: bool = #allow_self_referential;

            #[inline(always)]
            fn related_target(&self) -> #entity_ {
                self.#link_member
            }

            #[inline]
            fn from_target(entity: #entity_) -> Self {
                Self {
                    #(#members: ::core::default::Default::default(),)*
                    #link_member: entity
                }
            }

            #[inline]
            fn raw_target_mut(this: &mut Self) -> &mut #entity_ {
                &mut this.#link_member
            }
        }
    })
}

// -----------------------------------------------------------------------------
// relationship_target_tokens

fn relationship_target_tokens(
    ast: &DeriveInput,
    attrs: &Attributes,
    voker_ecs_path: &syn::Path,
) -> syn::Result<TokenStream2> {
    let Some(relationship_target) = &attrs.relationship_target else {
        return Ok(TokenStream2::new());
    };

    let syn::Data::Struct(syn::DataStruct {
        fields,
        struct_token,
        ..
    }) = &ast.data
    else {
        return Err(syn::Error::new(
            ast.span(),
            "Link can only be derived for structs.",
        ));
    };

    let related_field = parse_related_field(fields, struct_token.span())?;

    if related_field.vis != syn::Visibility::Inherited {
        return Err(syn::Error::new(
            related_field.span(),
            "The SourceSet in RelationshipTarget must be private to\
            prevent users from directly mutating it, which could \
            invalidate the correctness of link.",
        ));
    }

    let link_member = related_field
        .ident
        .clone()
        .map_or(syn::Member::from(0), syn::Member::Named);

    let members = fields.members().filter(|member| member != &link_member);

    let relationship_target_ = crate::path::relationship_target_(voker_ecs_path);
    let source_set = &related_field.ty;
    let relationship = &relationship_target.relationship;
    let linked_lifecycle = relationship_target.linked_lifecycle;

    let struct_name = &ast.ident;

    let (impl_generics, type_generics, where_clause) = ast.generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics #relationship_target_ for #struct_name #type_generics #where_clause {
            type Relationship = #relationship;
            type SourceSet = #source_set;
            const LINKED_LIFECYCLE: bool = #linked_lifecycle;

            #[inline(always)]
            fn related_sources(&self) -> &<Self as #relationship_target_>::SourceSet {
                &self.#link_member
            }

            #[inline]
            fn from_sources(sources: <Self as #relationship_target_>::SourceSet) -> Self {
                Self {
                    #(#members: ::core::default::Default::default(),)*
                    #link_member: sources
                }
            }

            #[inline]
            fn raw_sources_mut(this: &mut Self) -> &mut <Self as #relationship_target_>::SourceSet {
                &mut this.#link_member
            }


        }
    })
}

// -----------------------------------------------------------------------------
// Derive

pub(crate) fn impl_derive_component(mut ast: DeriveInput) -> proc_macro::TokenStream {
    let attrs = match parse_attributes(&ast.attrs) {
        Ok(a) => a,
        Err(e) => return e.into_compile_error().into(),
    };

    let ast_span = ast.span();

    use crate::path::fp::{CloneFP, CopyFP, OptionFP, SendFP, SyncFP};
    let voker_ecs_path = crate::path::voker_ecs();
    let component_ = crate::path::component_(&voker_ecs_path);
    let component_cloner_ = crate::path::component_cloner_(&voker_ecs_path);
    let storage_mode_ = crate::path::storage_mode_(&voker_ecs_path);
    let required_ = crate::path::required_(&voker_ecs_path);
    let component_hook_ = crate::path::component_hook_(&voker_ecs_path);
    let relationship_ = crate::path::relationship_(&voker_ecs_path);
    let relationship_target_ = crate::path::relationship_target_(&voker_ecs_path);

    // generics
    if ast.generics.type_params().next().is_some() {
        let predicates = &mut ast.generics.make_where_clause().predicates;

        predicates.push(parse_quote! { Self: #SendFP + #SyncFP + Sized + 'static });

        match &attrs.cloner {
            Cloner::Custom(_) => {}
            Cloner::Copy => predicates.push(parse_quote! { Self: #CopyFP }),
            _ => predicates.push(parse_quote! { Self: #CloneFP }),
        }
    } else if ast.generics.lifetimes().next().is_some() {
        ast.generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: 'static });
    }

    // map_entities_tokens
    let map_entities_tokens = match map_entities_tokens(&ast.data, &attrs, &voker_ecs_path) {
        Ok(a) => a,
        Err(e) => return e.into_compile_error().into(),
    };

    // relationship_registrar_tokens
    let relationship_registrar_tokens =
        match relationship_registrar_tokens(&ast, &attrs, &voker_ecs_path) {
            Ok(a) => a,
            Err(e) => return e.into_compile_error().into(),
        };

    // relationship_tokens
    let relationship_tokens = match relationship_tokens(&ast, &attrs, &voker_ecs_path) {
        Ok(a) => a,
        Err(e) => return e.into_compile_error().into(),
    };

    // relationship_target_tokens
    let relationship_target_tokens = match relationship_target_tokens(&ast, &attrs, &voker_ecs_path)
    {
        Ok(a) => a,
        Err(e) => return e.into_compile_error().into(),
    };

    // type_ident
    let type_ident = ast.ident;

    // mutable_tokens
    let mutable_tokens = (!attrs.mutable).then(|| quote! { const MUTABLE: bool = false; });

    // component_cloner_tokens
    let component_cloner_tokens = match attrs.cloner {
        Cloner::Default => Some(
            quote! { const CLONER: #component_cloner_ = #component_cloner_::clonable::<Self>(); },
        ),
        Cloner::Copy => Some(
            quote! { const CLONER: #component_cloner_ = #component_cloner_::copyable::<Self>(); },
        ),
        Cloner::Clone => Some(
            quote! { const CLONER: #component_cloner_ = #component_cloner_::clonable::<Self>(); },
        ),
        Cloner::Relationship => Some(
            quote! { const CLONER: #component_cloner_ = #component_cloner_::relationship::<Self>(); },
        ),
        Cloner::RelationshipTarget => Some(
            quote! { const CLONER: #component_cloner_ = #component_cloner_::relationship_target::<Self>(); },
        ),
        Cloner::Custom(expr_path) => Some(
            quote! { const CLONER: #component_cloner_ = #component_cloner_::custom(#expr_path); },
        ),
    };

    // storage_tokens
    let storage_tokens = match attrs.storage {
        Storage::Sparse => Some(quote! { const STORAGE: #storage_mode_ = #storage_mode_::Sparse; }),
        Storage::Dense => None,
    };

    // no_entity_tokens
    let no_entity = attrs.no_entity || map_entities_tokens.is_empty();
    let no_entity_tokens = no_entity.then(|| {
        quote! {
            const NO_ENTITY: bool = true;
        }
    });

    // required_tokens
    let required_tokens = attrs.required.map(|ty| {
        quote! {
            const REQUIRED: #OptionFP<#required_> = #OptionFP::Some(#required_::from::<#ty>());
        }
    });

    // on_add_tokens
    let on_add_tokens = attrs.on_add.map(|ty| {
        quote! {
            const ON_ADD: #OptionFP<#component_hook_> = #OptionFP::Some(#ty);
        }
    });

    // on_clone_tokens
    let on_clone_tokens = attrs.on_clone.map(|ty| {
        quote! {
            const ON_CLONE: #OptionFP<#component_hook_> = #OptionFP::Some(#ty);
        }
    });

    // on_remove_tokens
    let on_remove_tokens = attrs.on_remove.map(|ty| {
        quote! {
            const ON_REMOVE: #OptionFP<#component_hook_> = #OptionFP::Some(#ty);
        }
    });

    // on_insert_tokens
    let on_insert_tokens = if attrs.relationship.is_none() {
        attrs.on_insert.map(|ty| {
            quote! {
                const ON_INSERT: #OptionFP<#component_hook_> = #OptionFP::Some(#ty);
            }
        })
    } else if attrs.on_insert.is_some() {
        let error = syn::Error::new(
            ast_span,
            "Custom on_insert hooks are not supported as Relationship already defines an on_insert hook",
        );
        return error.into_compile_error().into();
    } else {
        Some(quote!(
            const ON_INSERT: #OptionFP<#component_hook_> = #OptionFP::Some(<Self as #relationship_>::on_insert);
        ))
    };

    // on_discard_tokens
    let on_discard_tokens = if attrs.relationship.is_none() && attrs.relationship_target.is_none() {
        attrs.on_discard.map(|ty| {
            quote! {
                const ON_DISCARD: #OptionFP<#component_hook_> = #OptionFP::Some(#ty);
            }
        })
    } else if attrs.on_discard.is_some() {
        let error = syn::Error::new(
            ast_span,
            "Custom on_discard hooks are not supported as Relationship/RelationshipTarget already defines an on_discard hook",
        );
        return error.into_compile_error().into();
    } else if attrs.relationship.is_some() {
        Some(quote!(
            const ON_DISCARD: #OptionFP<#component_hook_> = #OptionFP::Some(<Self as #relationship_>::on_discard);
        ))
    } else {
        Some(quote!(
            const ON_DISCARD: #OptionFP<#component_hook_> = #OptionFP::Some(<Self as #relationship_target_>::on_discard);
        ))
    };

    let on_despawn_tokens = if attrs.relationship_target.is_none() {
        attrs.on_despawn.map(|ty| {
            quote! {
                const ON_DESPAWN: #OptionFP<#component_hook_> = #OptionFP::Some(#ty);
            }
        })
    } else if attrs.on_despawn.is_some() {
        let error = syn::Error::new(
            ast_span,
            "Custom on_despawn hooks are not supported as RelationshipTarget already defines an on_despawn hook",
        );
        return error.into_compile_error().into();
    } else {
        Some(quote!(
            const ON_DESPAWN: #OptionFP<#component_hook_> = #OptionFP::Some(<Self as #relationship_target_>::on_despawn);
        ))
    };

    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    quote! {
        const _: () = {
            impl #impl_generics #component_ for #type_ident #ty_generics #where_clause {
                #mutable_tokens
                #component_cloner_tokens
                #storage_tokens
                #required_tokens
                #no_entity_tokens

                #on_add_tokens
                #on_clone_tokens
                #on_insert_tokens
                #on_remove_tokens
                #on_discard_tokens
                #on_despawn_tokens

                #relationship_registrar_tokens

                #map_entities_tokens
            }

            #relationship_tokens

            #relationship_target_tokens
        };
    }
    .into()
}
