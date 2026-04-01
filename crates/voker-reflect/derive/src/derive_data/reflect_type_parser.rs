use crate::utils::StringExpr;
use quote::{ToTokens, quote};
use syn::{GenericParam, Generics, Ident, LitStr, Path};
use syn::{punctuated::Punctuated, spanned::Spanned};

/// A container used to parse type paths and generic parameters.
///
/// The container will only be a part of [`ReflectMeta`](crate::derive_data::ReflectMeta),
/// so no interfaces will be exposed.
pub(crate) enum TypeParser<'a> {
    /// Types without a crate/module that can be named from any scope (e.g. `bool`).
    Primitive(&'a Ident),
    /// The type must be able to be reached with just its ident.
    ///
    /// For local types, can use [`module_path!()`](module_path) to get the module path.
    Local {
        ident: &'a Ident,
        custom_path: Option<Path>,
        generics: &'a Generics,
    },
    /// For foreign, [`module_path!()`](module_path) can not be used.
    /// So the user needs to provide the complete path, using `::my_crate::foo::Bar` syntax.
    Foreign {
        path: &'a Path,
        custom_path: Option<Path>,
        generics: &'a Generics,
    },
}

impl<'a> TypeParser<'a> {
    /// See [`ReflectDerive::from_input`](crate::derive_data::ReflectDerive::from_input)
    pub(crate) fn new_local(
        ident: &'a Ident,
        custom_path: Option<Path>,
        generics: &'a Generics,
    ) -> TypeParser<'a> {
        TypeParser::Local {
            ident,
            custom_path,
            generics,
        }
    }

    /// See [`impl_reflect_opaque`](crate::impl_reflect_opaque) and [`impl_type_path`](crate::impl_type_path)
    pub(crate) fn new_foreign(
        ident: &'a Ident,
        path: &'a Path,
        custom_path: Option<Path>,
        generics: &'a Generics,
    ) -> TypeParser<'a> {
        if custom_path.is_none() && path.leading_colon.is_none() {
            TypeParser::Primitive(ident)
        } else {
            TypeParser::Foreign {
                path,
                custom_path,
                generics,
            }
        }
    }

    pub(super) fn generics(&self) -> &'a Generics {
        // Use a constant because we need to return a reference of at least 'a.
        const EMPTY_GENERICS: &Generics = &Generics {
            gt_token: None,
            lt_token: None,
            where_clause: None,
            params: Punctuated::new(),
        };

        match self {
            Self::Local { generics, .. } | Self::Foreign { generics, .. } => generics,
            _ => EMPTY_GENERICS,
        }
    }

    /// Whether an implementation of `Typed` or `TypePath` should be generic.
    pub(super) fn contains_generics(&self) -> bool {
        // exist non-lifecycle generic parameters
        self.generics()
            .params
            .iter()
            .any(|param| !matches!(param, GenericParam::Lifetime(_)))
    }

    /// Whether an implementation of `Typed` or `TypePath` should be generic.
    pub(super) fn without_generics(&self) -> bool {
        self.generics().params.is_empty()
    }

    /// This name is used in `impl ... for #real_ident {...}`.
    pub(super) fn real_ident(&self) -> proc_macro2::TokenStream {
        match self {
            Self::Local { ident, .. } | Self::Primitive(ident) => ident.to_token_stream(),
            Self::Foreign { path, .. } => path.to_token_stream(),
        }
    }

    /// Get (custom) ident
    fn get_ident(&self) -> &Ident {
        match self {
            Self::Primitive(ident) => ident,
            Self::Local {
                ident, custom_path, ..
            } => custom_path
                .as_ref()
                .map(|path| &path.segments.last().unwrap().ident)
                .unwrap_or(ident),
            Self::Foreign {
                path, custom_path, ..
            } => &custom_path.as_ref().unwrap_or(path).segments.last().unwrap().ident,
        }
    }

    /// Try to get full (custom) path, not contain generic params.
    fn get_path(&self) -> Option<&Path> {
        match self {
            Self::Local { custom_path, .. } => custom_path.as_ref(),
            Self::Foreign {
                path, custom_path, ..
            } => Some(custom_path.as_ref().unwrap_or(path)),
            _ => None,
        }
    }

    pub(super) fn module_path(&self) -> Option<StringExpr> {
        if let Some(path) = self.get_path() {
            let path_string = path
                .segments
                .iter()
                .take(path.segments.len() - 1)
                .map(|segment| segment.ident.to_string())
                .reduce(|path, ident| path + "::" + &ident)
                .expect("If Path/CustomPath is exist, can not be empty.");

            let path_lit = LitStr::new(&path_string, path.span());
            return Some(StringExpr::from_lit(&path_lit));
        }

        match self {
            Self::Local { .. } => Some(StringExpr::Const(quote! {
                ::core::module_path!()
            })),
            _ => None,
        }
    }

    pub(super) fn type_ident(&self) -> StringExpr {
        StringExpr::from(self.get_ident())
    }

    /// Combines type generics and const generics into one [`StringExpr`].
    ///
    /// This string can be used with a `GenericTypePathCell` in a `TypePath` implementation.
    ///
    /// The `ty_generic_fn` param maps [`TypeParam`]s to [`StringExpr`]s.
    fn reduce_generics(generics: &Generics, voker_reflect_path: &Path) -> StringExpr {
        let macro_utils_ = crate::path::macro_utils_(voker_reflect_path);
        let type_path_ = crate::path::type_path_(voker_reflect_path);

        let mut params = generics.params.iter().filter_map(|param| match param {
            GenericParam::Type(type_param) => {
                let ident = &type_param.ident;

                Some(StringExpr::Borrowed(quote! {
                    <#ident as #type_path_>::type_name()
                }))
            }
            GenericParam::Const(const_param) => {
                let ident = &const_param.ident;
                let ty = &const_param.ty;

                Some(StringExpr::Owned(quote! {
                    <#ty as #macro_utils_::ToString>::to_string(&#ident)
                }))
            }
            GenericParam::Lifetime(_) => None,
        });

        let first = params.next().into_iter();

        StringExpr::from_iter(
            first.chain(params.flat_map(|x| [StringExpr::from_str(", "), x])),
            voker_reflect_path,
        )
    }

    /// Returns a [`StringExpr`] representing the "type name" of the type.
    ///
    /// For `core::option::Option<core::marker::PhantomData>`, this is `"Option<PhantomData>"`.
    pub(super) fn type_name(&self, voker_reflect_path: &Path) -> StringExpr {
        match self {
            Self::Primitive(ident) => StringExpr::from(ident),
            Self::Local { generics, .. } | Self::Foreign { generics, .. } => {
                let type_ident = self.type_ident();

                if self.contains_generics() {
                    let generics = TypeParser::reduce_generics(generics, voker_reflect_path);

                    StringExpr::from_iter(
                        [
                            type_ident,
                            StringExpr::from_str("<"),
                            generics,
                            StringExpr::from_str(">"),
                        ],
                        voker_reflect_path,
                    )
                } else {
                    type_ident
                }
            }
        }
    }

    /// Returns a [`StringExpr`] representing the "type path" of the type.
    ///
    /// For `Option<PhantomData>`, this is `"core::option::Option<core::marker::PhantomData>"`.
    pub(super) fn type_path(&self, voker_reflect_path: &Path) -> StringExpr {
        match self {
            Self::Primitive(ident) => StringExpr::from(ident),
            Self::Local { generics, .. } | Self::Foreign { generics, .. } => {
                let type_ident = self.type_ident();
                let module_path = self
                    .module_path()
                    .expect("Non-Primitive type, try to parse type_path but get module_path fail.");

                if self.contains_generics() {
                    let generics = TypeParser::reduce_generics(generics, voker_reflect_path);

                    StringExpr::from_iter(
                        [
                            module_path,
                            StringExpr::from_str("::"),
                            type_ident,
                            StringExpr::from_str("<"),
                            generics,
                            StringExpr::from_str(">"),
                        ],
                        voker_reflect_path,
                    )
                } else {
                    StringExpr::from_iter(
                        [module_path, StringExpr::from_str("::"), type_ident],
                        voker_reflect_path,
                    )
                }
            }
        }
    }
}
