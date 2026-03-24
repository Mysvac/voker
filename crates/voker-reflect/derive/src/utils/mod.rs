mod string_expr;
pub(crate) use string_expr::StringExpr;

/// Empty Token Stream
#[inline(always)]
pub(crate) fn empty() -> proc_macro2::TokenStream {
    proc_macro2::TokenStream::new()
}
