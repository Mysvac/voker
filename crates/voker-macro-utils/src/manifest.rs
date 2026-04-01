//! Cargo manifest based crate path resolution.
//!
//! This module reads the active `Cargo.toml` from `CARGO_MANIFEST_DIR` and
//! resolves crate names into absolute [`syn::Path`] values.
//!
//! A small cache keyed by manifest path and modified time avoids reparsing on
//! repeated lookups in the same process.

use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{PoisonError, RwLock};
use std::time::SystemTime;

use toml_edit::{Document, Item, Table};

// -----------------------------------------------------------------------------
// Config

const ENGINE_NAME: &str = "voker";
const ENGINE_PATH: &str = "::voker";
const ENGINE_PREFIX: &str = "voker_";

// -----------------------------------------------------------------------------
// Manifest

struct Manifest {
    manifest: Document<Box<str>>,
    modified_time: SystemTime,
}

/// Resolve a crate name to an absolute [`syn::Path`].
///
/// Can **not** be used for third-party crates that with `voker_` prefix.
///
/// Internal aliasing rule:
/// if dependency `voker` exists and the requested name starts with `voker_`,
/// the prefix is mapped to `::voker::<module>`.
///
/// Practical flow:
///
/// 1. If target does not start with `voker_`, use direct dependency lookup;
///    if not found, return input form.
///
/// 2. If target starts with `voker_` and a same-name dependency exists,
///    return that direct dependency path.
///
/// 3. If target starts with `voker_`, same-name dependency does not exist,
///    but dependency `voker` exists, return `::voker::<module>`.
///
/// 4. Otherwise, return the input form as absolute path (`::name`).
///
/// # Panics
///
/// Panics if `CARGO_MANIFEST_DIR` is missing, if `Cargo.toml` cannot be read,
/// or if the manifest content cannot be parsed.
///
/// # Examples
///
/// ```ignore
/// let ecs_path = voker_macro_utils::crate_path("voker_ecs");
/// ```
pub fn crate_path(path: &'static str) -> syn::Path {
    Manifest::shared(|manifest| manifest.find_crate_path(path))
}

impl Manifest {
    fn shared<R>(func: impl FnOnce(&Self) -> R) -> R {
        static MANIFESTS: RwLock<BTreeMap<PathBuf, Manifest>> = RwLock::new(BTreeMap::new());

        #[cold]
        #[inline(always)]
        const fn cold_path() {}

        fn manifest_meta() -> (PathBuf, SystemTime) {
            let mut path = env::var_os("CARGO_MANIFEST_DIR")
                .map(PathBuf::from)
                .expect("CARGO_MANIFEST_DIR should be auto-defined by cargo.");

            path.push("Cargo.toml");

            let modified_time = std::fs::metadata(&path)
                .map_err(|_| panic!("Cargo manifest does not exist at path {path:?}"))
                .and_then(|metadata| metadata.modified())
                .expect("The Cargo.toml should have a modified time.");

            (path, modified_time)
        }

        fn read_manifest(path: &Path) -> Document<Box<str>> {
            std::fs::read_to_string(path)
                .map(String::into_boxed_str)
                .map(Document::parse)
                .unwrap_or_else(|_| panic!("Failed to read cargo manifest: {}", path.display()))
                .unwrap_or_else(|e| panic!("Failed to parse cargo manifest({path:?}): {e}"))
        }

        let (path, time) = manifest_meta();

        let manifests = MANIFESTS.read().unwrap_or_else(PoisonError::into_inner);

        if let Some(manifest) = manifests.get(&path)
            && manifest.modified_time == time
        {
            return func(manifest);
        }

        cold_path();
        drop(manifests);

        let manifest = Manifest {
            manifest: read_manifest(&path),
            modified_time: time,
        };

        let result = func(&manifest);

        MANIFESTS
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(path, manifest);

        result
    }

    fn find_crate_path(&self, name: &'static str) -> syn::Path {
        fn find_path_in_deps(deps: &Table, name: &str) -> Option<syn::Path> {
            if deps.contains_key(name) {
                let mut path: syn::Path = syn::parse_str(name).unwrap();
                path.leading_colon = Some(Default::default());
                Some(path)
            } else if deps.contains_key(ENGINE_NAME)
                && let Some(module) = name.strip_prefix(ENGINE_PREFIX)
            {
                let mut path: syn::Path = syn::parse_str(ENGINE_PATH).unwrap();
                let module: syn::PathSegment = syn::parse_str(module).unwrap();
                path.segments.push(module);
                Some(path)
            } else {
                None
            }
        }

        if let Some(Item::Table(deps)) = self.manifest.get("dependencies")
            && let Some(val) = find_path_in_deps(deps, name)
        {
            return val;
        }

        if let Some(Item::Table(deps)) = self.manifest.get("dev-dependencies")
            && let Some(val) = find_path_in_deps(deps, name)
        {
            return val;
        }

        let mut path: syn::Path = syn::parse_str(name).unwrap();
        path.leading_colon = Some(Default::default());
        path
    }
}
