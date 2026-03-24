use syn::{Attribute, Generics, Ident, Path, PathSegment};
use syn::{Token, parenthesized, parse::ParseStream, token::Paren};

use super::TypeAttributes;

// -----------------------------------------------------------------------------
// Path Parser

/// Format: `(in module_path as alias_name)`
fn parse_custom_path(input: ParseStream) -> syn::Result<(Option<Path>, Option<Ident>)> {
    if input.peek(Paren) {
        let inner;
        parenthesized!(inner in input);
        inner.parse::<Token![in]>()?;
        if inner.peek(Token![::]) {
            return Err(inner.error("did not expect a leading double colon (`::`)"));
        }
        let path = Path::parse_mod_style(&inner)?;
        if path.segments.is_empty() {
            return Err(inner.error("expected a path"));
        }

        if !inner.peek(Token![as]) {
            return Ok((Some(path), None));
        }

        inner.parse::<Token![as]>()?;
        let name: Ident = inner.parse()?;
        Ok((Some(path), Some(name)))
    } else {
        Ok((None, None))
    }
}

pub(crate) struct ReflectTypePathParser {
    pub custom_path: Option<Path>,
    pub type_ident: Ident,
    pub type_path: Path,
    pub generics: Generics,
}

impl ReflectTypePathParser {
    /// Parse the input stream of [`impl_type_path`](crate::impl_type_path).
    ///
    /// Format: `(in module_path as alias_name) ident`
    pub fn parse(input: ParseStream) -> syn::Result<Self> {
        let (custom_path, custom_name) = parse_custom_path(input)?;

        let type_path = Path::parse_mod_style(input)?;

        let type_ident = type_path.segments.last().unwrap().ident.clone();

        let custom_path = if let Some(mut path) = custom_path {
            let name = PathSegment::from(custom_name.unwrap_or_else(|| type_ident.clone()));
            path.segments.push(name);
            Some(path)
        } else {
            None
        };

        let mut generics = input.parse::<Generics>()?;
        generics.where_clause = input.parse()?;

        Ok(Self {
            custom_path,
            type_ident,
            type_path,
            generics,
        })
    }
}

// -----------------------------------------------------------------------------
// Opaque Parser

/// A struct used to define a simple reflection-opaque types (including primitives).
pub(crate) struct ReflectOpaqueParser {
    pub attrs: TypeAttributes,
    pub custom_path: Option<Path>,
    pub type_ident: Ident,
    pub type_path: Path,
    pub generics: Generics,
}

impl ReflectOpaqueParser {
    /// Parse the input stream of [`impl_reflect_opaque`](crate::impl_reflect_opaque).
    ///
    /// Format: `(in module_path as alias_name) ident (..attrs..)`
    pub fn parse(input: ParseStream) -> syn::Result<Self> {
        let origin_span = input.span();
        // For outer document comments.
        let origin_attrs = input.call(Attribute::parse_outer)?;
        // Parse outer document comments.
        let mut attrs = TypeAttributes::parse_attrs(origin_attrs.as_slice())?;

        let ReflectTypePathParser {
            custom_path,
            type_ident,
            type_path,
            generics,
        } = ReflectTypePathParser::parse(input)?;

        // Parse inner attributes.
        if input.peek(Paren) {
            let content;
            parenthesized!(content in input);
            attrs.parse_stream(&content)?;
        }
        attrs.is_opaque = Some(origin_span);
        attrs.validity()?;

        Ok(Self {
            attrs,
            custom_path,
            type_ident,
            type_path,
            generics,
        })
    }
}
