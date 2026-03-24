/// An alternative to experimental [`core::hint::cold_path`].
///
/// Hints to the compiler that given path is cold, i.e., unlikely to be taken.
/// The compiler may choose to optimize paths that are not cold at the expense
/// of paths that are cold.
///
/// # Examples
///
/// Consider this simple branching code:
///
/// ```no_run
/// # fn f1() {}
/// # fn f2() {}
/// # let condition = false;
/// if condition {
///     f1();
/// } else {
///     f2();
/// }
/// ```
///
/// The generated assembly code may look like this:
///
/// ```ignore, asm
/// example:
///     test  rdi, rdi
///     je    .Lelse
///     call  f1
///     ret
/// .Lelse:
///     call  f2
///     ret
/// ```
///
/// When functions are inlined, the placement of code blocks affects instruction
/// cache performance—the block closer to the conditional jump (the fallthrough
/// path) enjoys better cache locality.
///
/// By default, the compiler decides which branch becomes the fallthrough path,
/// which may not align with actual execution frequencies.
///
/// Rust's `#[cold]` attribute marks rarely-called functions. When a branch calls
/// a `#[cold]` function, the compiler typically moves that branch far from the
/// jump, making the other branch the cache-friendly fallthrough path.
///
/// However, `#[cold]` only applies to functions, not statements inside a function.
/// [`cold_path`] is a stable alternative to the experimental `core::hint::cold_path`,
/// letting you mark cold paths anywhere. Use it like this:
///
/// ```no_run
/// # fn f1() {}
/// # fn f2() {}
/// # let condition = false;
/// if condition {
///     voker_utils::cold_path();
///     f1();
/// } else {
///     f2();
/// }
/// ```
///
/// This allows the compiler to better optimize the code layout:
///
/// ```ignore, asm
/// example:
///     test  rdi, rdi
///     jne   .Lcold
///     call  f2
///     ret
/// .Lcold:
///     call  f1
///     ret
/// ```
#[cold]
#[inline(always)]
pub const fn cold_path() {}
