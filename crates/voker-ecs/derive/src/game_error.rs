use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_quote};

#[derive(PartialEq, Eq)]
pub enum Severity {
    Default,
    Ignore,
    Trace,
    Debug,
    Info,
    Warning,
    Error,
    Panic,
    Field(syn::Member),
}

fn invalid_expr(expr: &syn::Expr) -> syn::Error {
    syn::Error::new_spanned(
        expr,
        "Invalid severity value.\n\
        Valid options are: \n\
        - String literal: \"ignore\", \"trace\", \"debug\", \"info\", \"warning\", \"error\", \"panic\"\n\
        - Field access: `self.field` / `self.0`\n\
        For example: `#[game_error(severity = \"warning\")]` or `#[game_error(severity = self.field)]\n`",
    )
}

fn parse_severity_expr(expr: &syn::Expr) -> syn::Result<Severity> {
    if let syn::Expr::Lit(expr_lit) = expr {
        if let syn::Lit::Str(s) = &expr_lit.lit {
            return match s.value().as_str() {
                "ignore" => Ok(Severity::Ignore),
                "trace" => Ok(Severity::Trace),
                "debug" => Ok(Severity::Debug),
                "info" => Ok(Severity::Info),
                "warning" => Ok(Severity::Warning),
                "error" => Ok(Severity::Error),
                "panic" => Ok(Severity::Panic),
                "warn" => Err(syn::Error::new_spanned(expr, "Use \"warning\" instead.")),
                _ => Err(invalid_expr(expr)),
            };
        }

        if let syn::Lit::Int(int) = &expr_lit.lit {
            let index = int.base10_parse::<usize>()?;
            return Ok(Severity::Field(syn::Member::Unnamed(index.into())));
        }
    }

    if let syn::Expr::Field(field) = expr
        && let syn::Expr::Path(base) = &*field.base
        && base.path.is_ident("self")
    {
        return Ok(Severity::Field(field.member.clone()));
    }

    Err(invalid_expr(expr))
}

fn parse_attributes(attrs: &[syn::Attribute]) -> syn::Result<Severity> {
    let mut severity = Severity::Default;

    for attr in attrs {
        if !attr.path().is_ident("game_error") {
            continue;
        }

        if severity != Severity::Default {
            return Err(syn::Error::new_spanned(
                attr,
                "Duplicated game_error severity annotation.",
            ));
        }

        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("severity") {
                return Err(meta.error("Unsupported key. Expected `severity`."));
            }

            let value = meta.value()?;
            let expr: syn::Expr = value.parse()?;
            severity = parse_severity_expr(&expr)?;
            Ok(())
        })?;
    }

    Ok(severity)
}

pub(crate) fn impl_derive_game_error(ast: DeriveInput) -> TokenStream {
    let attrs = match parse_attributes(&ast.attrs) {
        Ok(a) => a,
        Err(e) => return e.into_compile_error().into(),
    };

    use crate::path::fp::{ErrorFP, FromFP, SendFP, SyncFP};
    let voker_ecs_path = crate::path::voker_ecs();
    let game_error_ = crate::path::game_error_(&voker_ecs_path);
    let severity_ = crate::path::severity_(&voker_ecs_path);

    let type_ident = ast.ident;

    let mut generics = ast.generics.clone();
    if generics.type_params().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: #ErrorFP + #SendFP + #SyncFP + 'static });
    } else if generics.lifetimes().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: 'static });
    }

    let severity_tokens = match attrs {
        Severity::Default => quote! { #severity_::Panic },
        Severity::Ignore => quote! { #severity_::Ignore },
        Severity::Trace => quote! { #severity_::Trace },
        Severity::Debug => quote! { #severity_::Debug },
        Severity::Info => quote! { #severity_::Info },
        Severity::Warning => quote! { #severity_::Warning },
        Severity::Error => quote! { #severity_::Error },
        Severity::Panic => quote! { #severity_::Panic },
        Severity::Field(mem) => quote! { value.#mem },
    };

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        const _:() = {
            impl #impl_generics #FromFP<#type_ident #ty_generics> for  #game_error_ #where_clause {
                #[cold]
                fn from(value: #type_ident #ty_generics) -> Self {
                    #game_error_::new(#severity_tokens, value)
                }
            }
        };
    }
    .into()
}
