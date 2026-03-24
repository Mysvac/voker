# voker-path

Path helpers for proc-macro crates.

This crate provides:

1. Full-path marker types for common `core` items.
2. `Cargo.toml` based crate path resolution.

## Core APIs

- `full_path`: marker types like `AnyFP`, `OptionFP`, `ResultFP`.
- `crate_path`: resolve a crate name to absolute `syn::Path`.
- `crate_path!`: macro wrapper of `crate_path`.

## crate_path Rules

Resolution flow:

1. If target does not start with `voker_`, use direct dependency if found; otherwise return `::name`.
2. If target starts with `voker_` and same-name dependency exists, return that direct dependency path.
3. If target starts with `voker_`, same-name dependency is missing, but `voker` exists, return `::voker::<module>`.
4. Otherwise, return `::name`.
