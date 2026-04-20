use alloc::borrow::ToOwned;
use alloc::string::{String, ToString};
use core::fmt::{Debug, Display};
use core::ops::Range;
use std::path::{Path, PathBuf};

use atomicow::CowArc;
use serde::{Deserialize, Serialize, de::Visitor};
use thiserror::Error;
use voker_reflect::Reflect;

use crate::AssetSourceId;

// -----------------------------------------------------------------------------
// AssetPath

/// Normalizes the path by collapsing all occurrences of '.' and '..' dot-segments
/// where possible as per [RFC 1808](https://datatracker.ietf.org/doc/html/rfc1808)
pub(crate) fn normalize_path(path: &Path) -> PathBuf {
    let mut result_path = PathBuf::new();
    for elt in path.iter() {
        if elt == "." {
            // Skip
        } else if elt == ".." {
            // Note: If the result_path ends in `..`, Path::file_name returns None,
            // so we'll end up preserving it.
            if result_path.file_name().is_some() {
                // This assert is just a sanity check - we already know the path
                // has a file_name, so we know there is something to pop.
                assert!(result_path.pop());
            } else {
                // Preserve ".." if insufficient matches (per RFC 1808).
                result_path.push(elt);
            }
        } else {
            result_path.push(elt);
        }
    }
    result_path
}

// -----------------------------------------------------------------------------
// AssetPath

#[derive(Default, Clone, PartialEq, Eq, Hash, Reflect)]
#[reflect(Opaque, Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub struct AssetPath<'a> {
    source: AssetSourceId<'a>,
    path: CowArc<'a, Path>,
    label: Option<CowArc<'a, str>>,
}

/// An error that occurs when parsing a string type to create an [`AssetPath`] fails, such as during [`AssetPath::parse`].
#[derive(Error, Debug, PartialEq, Eq)]
pub enum ParseAssetPathError {
    /// Error that occurs when the [`AssetPath::source`] section of a path string contains the [`AssetPath::label`] delimiter `#`. E.g. `bad#source://file.test`.
    #[error("Asset source must not contain a `#` character")]
    InvalidSourceSyntax,
    /// Error that occurs when the [`AssetPath::label`] section of a path string contains the [`AssetPath::source`] delimiter `://`. E.g. `source://file.test#bad://label`.
    #[error("Asset label must not contain a `://` substring")]
    InvalidLabelSyntax,
    /// Error that occurs when a path string has an [`AssetPath::source`] delimiter `://` with no characters preceding it. E.g. `://file.test`.
    #[error(
        "Asset source must be at least one character. Either specify the source before the '://' or remove the `://`"
    )]
    MissingSource,
    /// Error that occurs when a path string has an [`AssetPath::label`] delimiter `#` with no characters succeeding it. E.g. `file.test#`
    #[error(
        "Asset label must be at least one character. Either specify the label after the '#' or remove the '#'"
    )]
    MissingLabel,
}

impl<'a> AssetPath<'a> {
    // Attempts to Parse a &str into an `AssetPath`'s `AssetPath::source`, `AssetPath::path`, and `AssetPath::label` components.
    fn parse_internal(
        asset_path: &str,
    ) -> Result<(Option<&str>, &Path, Option<&str>), ParseAssetPathError> {
        let chars = asset_path.char_indices();
        let mut source_range: Option<Range<usize>> = None;
        let mut path_range: Range<usize> = 0..asset_path.len();
        let mut label_range: Option<Range<usize>> = None;

        // Loop through the characters of the passed in &str to accomplish the following:
        // 1. Search for the first instance of the `://` substring. If the `://` substring is found,
        //  store the range of indices representing everything before the `://` substring as the `source_range`.
        // 2. Search for the last instance of the `#` character. If the `#` character is found,
        //  store the range of indices representing everything after the `#` character as the `label_range`
        // 3. Set the `path_range` to be everything in between the `source_range` and `label_range`,
        //  excluding the `://` substring and `#` character.
        // 4. Verify that there are no `#` characters in the `AssetPath::source` and no `://` substrings in the `AssetPath::label`
        let mut source_delimiter_chars_matched = 0;
        let mut last_found_source_index = 0;
        for (index, char) in chars {
            match char {
                ':' => {
                    source_delimiter_chars_matched = 1;
                }
                '/' => {
                    match source_delimiter_chars_matched {
                        1 => {
                            source_delimiter_chars_matched = 2;
                        }
                        2 => {
                            // If we haven't found our first `AssetPath::source` yet, check to make sure it is valid and then store it.
                            if source_range.is_none() {
                                // If the `AssetPath::source` contains a `#` character, it is invalid.
                                if label_range.is_some() {
                                    return Err(ParseAssetPathError::InvalidSourceSyntax);
                                }
                                source_range = Some(0..index - 2);
                                path_range.start = index + 1;
                            }
                            last_found_source_index = index - 2;
                            source_delimiter_chars_matched = 0;
                        }
                        _ => {}
                    }
                }
                '#' => {
                    path_range.end = index;
                    label_range = Some(index + 1..asset_path.len());
                    source_delimiter_chars_matched = 0;
                }
                _ => {
                    source_delimiter_chars_matched = 0;
                }
            }
        }
        // If we found an `AssetPath::label`
        if let Some(range) = label_range.clone() {
            // If the `AssetPath::label` contained a `://` substring, it is invalid.
            if range.start <= last_found_source_index {
                return Err(ParseAssetPathError::InvalidLabelSyntax);
            }
        }
        // Try to parse the range of indices that represents the `AssetPath::source` portion of the `AssetPath` to make sure it is not empty.
        // This would be the case if the input &str was something like `://some/file.test`
        let source = match source_range {
            Some(source_range) => {
                if source_range.is_empty() {
                    return Err(ParseAssetPathError::MissingSource);
                }
                Some(&asset_path[source_range])
            }
            None => None,
        };
        // Try to parse the range of indices that represents the `AssetPath::label` portion of the `AssetPath` to make sure it is not empty.
        // This would be the case if the input &str was something like `some/file.test#`.
        let label = match label_range {
            Some(label_range) => {
                if label_range.is_empty() {
                    return Err(ParseAssetPathError::MissingLabel);
                }
                Some(&asset_path[label_range])
            }
            None => None,
        };

        let path = Path::new(&asset_path[path_range]);
        Ok((source, path, label))
    }

    /// Creates a new [`AssetPath`] from a string in the asset path format:
    /// * An asset at the root: `"scene.gltf"`
    /// * An asset nested in some folders: `"some/path/scene.gltf"`
    /// * An asset with a "label": `"some/path/scene.gltf#Mesh0"`
    /// * An asset with a custom "source": `"custom://some/path/scene.gltf#Mesh0"`
    ///
    /// Prefer [`From<'static str>`] or [`AssetPath::try_parse_static`] for static strings,
    /// as this will prevent allocations and reference counting for [`AssetPath::into_owned`].
    ///
    /// This will return a [`ParseAssetPathError`] if `asset_path` is in an invalid format.
    pub fn try_parse(asset_path: &'a str) -> Result<AssetPath<'a>, ParseAssetPathError> {
        let (source, path, label) = Self::parse_internal(asset_path)?;
        Ok(AssetPath {
            source: match source {
                Some(source) => AssetSourceId::Name(CowArc::Borrowed(source)),
                None => AssetSourceId::Default,
            },
            path: CowArc::Borrowed(path),
            label: label.map(CowArc::Borrowed),
        })
    }

    /// Creates a new [`AssetPath`] from a string in the asset path format:
    /// * An asset at the root: `"scene.gltf"`
    /// * An asset nested in some folders: `"some/path/scene.gltf"`
    /// * An asset with a "label": `"some/path/scene.gltf#Mesh0"`
    /// * An asset with a custom "source": `"custom://some/path/scene.gltf#Mesh0"`
    ///
    /// Prefer [`From<'static str>`] or [`AssetPath::parse_static`] for static strings,
    /// as this will prevent allocations and reference counting for [`AssetPath::into_owned`].
    ///
    /// # Panics
    /// Panics if the asset path is in an invalid format. Use [`AssetPath::try_parse`] for a fallible variant
    pub fn parse(asset_path: &'a str) -> AssetPath<'a> {
        Self::try_parse(asset_path).unwrap()
    }

    /// Creates a new [`AssetPath`] from a static string in the asset path format:
    /// * An asset at the root: `"scene.gltf"`
    /// * An asset nested in some folders: `"some/path/scene.gltf"`
    /// * An asset with a "label": `"some/path/scene.gltf#Mesh0"`
    /// * An asset with a custom "source": `"custom://some/path/scene.gltf#Mesh0"`
    ///
    /// This will return a [`ParseAssetPathError`] if `asset_path` is in an invalid format.
    pub fn try_parse_static(
        asset_path: &'static str,
    ) -> Result<AssetPath<'static>, ParseAssetPathError> {
        let (source, path, label) = Self::parse_internal(asset_path)?;
        Ok(AssetPath {
            source: match source {
                Some(source) => AssetSourceId::Name(CowArc::Static(source)),
                None => AssetSourceId::Default,
            },
            path: CowArc::Static(path),
            label: label.map(CowArc::Static),
        })
    }

    /// Creates a new [`AssetPath`] from a static string in the asset path format:
    /// * An asset at the root: `"scene.gltf"`
    /// * An asset nested in some folders: `"some/path/scene.gltf"`
    /// * An asset with a "label": `"some/path/scene.gltf#Mesh0"`
    /// * An asset with a custom "source": `"custom://some/path/scene.gltf#Mesh0"`
    ///
    /// # Panics
    /// Panics if the asset path is in an invalid format. Use [`AssetPath::try_parse`] for a fallible variant
    pub fn parse_static(asset_path: &'static str) -> AssetPath<'static> {
        Self::try_parse_static(asset_path).unwrap()
    }

    /// Creates a new [`AssetPath`] from a [`PathBuf`].
    #[inline]
    pub fn from_path_buf(path_buf: PathBuf) -> AssetPath<'a> {
        AssetPath {
            path: CowArc::Owned(path_buf.into()),
            source: AssetSourceId::Default,
            label: None,
        }
    }

    /// Creates a new [`AssetPath`] from a [`Path`].
    #[inline]
    pub fn from_path(path: &'a Path) -> AssetPath<'a> {
        AssetPath {
            path: CowArc::Borrowed(path),
            source: AssetSourceId::Default,
            label: None,
        }
    }

    /// Gets the "asset source", if one was defined. If none was defined, the default source
    /// will be used.
    #[inline]
    pub fn source(&self) -> &AssetSourceId<'_> {
        &self.source
    }

    /// Gets the "sub-asset label".
    #[inline]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Gets the "sub-asset label".
    #[inline]
    pub fn label_cow(&self) -> Option<CowArc<'a, str>> {
        self.label.clone()
    }

    /// Gets the path to the asset in the "virtual filesystem".
    #[inline]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Gets the path to the asset in the "virtual filesystem" without a label (if a label is currently set).
    #[inline]
    pub fn without_label(&self) -> AssetPath<'_> {
        Self {
            source: self.source.clone(),
            path: self.path.clone(),
            label: None,
        }
    }

    /// Removes a "sub-asset label" from this [`AssetPath`], if one was set.
    #[inline]
    pub fn remove_label(&mut self) {
        self.label = None;
    }

    /// Takes the "sub-asset label" from this [`AssetPath`], if one was set.
    #[inline]
    pub fn take_label(&mut self) -> Option<CowArc<'a, str>> {
        self.label.take()
    }

    /// Returns this asset path with the given label. This will replace the previous
    /// label if it exists.
    #[inline]
    pub fn with_label(self, label: impl Into<CowArc<'a, str>>) -> AssetPath<'a> {
        AssetPath {
            source: self.source,
            path: self.path,
            label: Some(label.into()),
        }
    }

    /// Returns this asset path with the given asset source. This will replace the previous asset
    /// source if it exists.
    #[inline]
    pub fn with_source(self, source: impl Into<AssetSourceId<'a>>) -> AssetPath<'a> {
        AssetPath {
            source: source.into(),
            path: self.path,
            label: self.label,
        }
    }

    /// Returns an [`AssetPath`] for the parent folder of this path, if there is a parent folder in the path.
    pub fn parent(&self) -> Option<AssetPath<'a>> {
        let path = match &self.path {
            CowArc::Borrowed(path) => CowArc::Borrowed(path.parent()?),
            CowArc::Static(path) => CowArc::Static(path.parent()?),
            CowArc::Owned(path) => path.parent()?.to_path_buf().into(),
        };
        Some(AssetPath {
            source: self.source.clone(),
            label: None,
            path,
        })
    }

    /// Converts this into an "owned" value. If internally a value is borrowed, it will be cloned into an "owned [`Arc`]".
    /// If internally a value is a static reference, the static reference will be used unchanged.
    /// If internally a value is an "owned [`Arc`]", it will remain unchanged.
    ///
    /// [`Arc`]: alloc::sync::Arc
    pub fn into_owned(self) -> AssetPath<'static> {
        AssetPath {
            source: self.source.into_owned(),
            path: self.path.into_owned(),
            label: self.label.map(CowArc::into_owned),
        }
    }

    /// Clones this into an "owned" value. If internally a value is borrowed, it will be cloned into an "owned [`Arc`]".
    /// If internally a value is a static reference, the static reference will be used unchanged.
    /// If internally a value is an "owned [`Arc`]", the [`Arc`] will be cloned.
    ///
    /// [`Arc`]: alloc::sync::Arc
    #[inline]
    pub fn clone_owned(&self) -> AssetPath<'static> {
        self.clone().into_owned()
    }

    /// Resolves an [`AssetPath`] relative to `self`.
    ///
    /// Semantics:
    /// - If `path` is label-only (default source, empty path, label set), replace `self`'s label.
    /// - If `path` begins with `/`, treat it as rooted at the asset-source root (not the filesystem).
    /// - If `path` has an explicit source (`name://...`), it replaces the base source.
    /// - Relative segments are concatenated and normalized (`.`/`..` removal), preserving extra `..` if the base underflows.
    ///
    /// ```
    /// # use voker_asset::AssetPath;
    /// let base = AssetPath::parse("a/b");
    /// assert_eq!(base.resolve(&AssetPath::parse("c")), AssetPath::parse("a/b/c"));
    /// assert_eq!(base.resolve(&AssetPath::parse("./c")), AssetPath::parse("a/b/c"));
    /// assert_eq!(base.resolve(&AssetPath::parse("../c")), AssetPath::parse("a/c"));
    /// assert_eq!(base.resolve(&AssetPath::parse("c.png")), AssetPath::parse("a/b/c.png"));
    /// assert_eq!(base.resolve(&AssetPath::parse("/c")), AssetPath::parse("c"));
    /// assert_eq!(AssetPath::parse("a/b.png").resolve(&AssetPath::parse("#c")), AssetPath::parse("a/b.png#c"));
    /// assert_eq!(AssetPath::parse("a/b.png#c").resolve(&AssetPath::parse("#d")), AssetPath::parse("a/b.png#d"));
    /// ```
    ///
    /// See also [`AssetPath::resolve_str`].
    pub fn resolve(&self, path: &AssetPath<'_>) -> AssetPath<'static> {
        let is_label_only = matches!(path.source(), AssetSourceId::Default)
            && path.path().as_os_str().is_empty()
            && path.label().is_some();

        if is_label_only {
            self.clone_owned().with_label(path.label().unwrap().to_owned())
        } else {
            let explicit_source = match path.source() {
                AssetSourceId::Default => None,
                AssetSourceId::Name(name) => Some(name.as_ref()),
            };

            self.resolve_from_parts(false, explicit_source, path.path(), path.label())
        }
    }

    /// Resolves an [`AssetPath`] relative to `self` using embedded (RFC 1808) semantics.
    ///
    /// Semantics:
    /// - Remove the "file portion" of the base before concatenation (unless the base ends with `/`).
    /// - Otherwise identical to [`AssetPath::resolve`].
    ///
    /// ```
    /// # use voker_asset::AssetPath;
    /// let base = AssetPath::parse("a/b");
    /// assert_eq!(base.resolve_embed(&AssetPath::parse("c")), AssetPath::parse("a/c"));
    /// assert_eq!(base.resolve_embed(&AssetPath::parse("./c")), AssetPath::parse("a/c"));
    /// assert_eq!(base.resolve_embed(&AssetPath::parse("../c")), AssetPath::parse("c"));
    /// assert_eq!(base.resolve_embed(&AssetPath::parse("c.png")), AssetPath::parse("a/c.png"));
    /// assert_eq!(base.resolve_embed(&AssetPath::parse("/c")), AssetPath::parse("c"));
    /// assert_eq!(AssetPath::parse("a/b.png").resolve_embed(&AssetPath::parse("#c")), AssetPath::parse("a/b.png#c"));
    /// assert_eq!(AssetPath::parse("a/b.png#c").resolve_embed(&AssetPath::parse("#d")), AssetPath::parse("a/b.png#d"));
    /// ```
    ///
    /// See also [`AssetPath::resolve_embed_str`].
    pub fn resolve_embed(&self, path: &AssetPath<'_>) -> AssetPath<'static> {
        let is_label_only = matches!(path.source(), AssetSourceId::Default)
            && path.path().as_os_str().is_empty()
            && path.label().is_some();

        if is_label_only {
            self.clone_owned().with_label(path.label().unwrap().to_owned())
        } else {
            let explicit_source = match path.source() {
                AssetSourceId::Default => None,
                AssetSourceId::Name(name) => Some(name.as_ref()),
            };

            self.resolve_from_parts(true, explicit_source, path.path(), path.label())
        }
    }

    /// Parses `path` as an [`AssetPath`], then resolves it relative to `self`.
    ///
    /// Returns an error if parsing fails.
    ///
    /// For more details, see [`AssetPath::resolve`].
    pub fn resolve_str(&self, path: &str) -> Result<AssetPath<'static>, ParseAssetPathError> {
        self.resolve_internal(path, false)
    }

    /// Parses `path` as an [`AssetPath`], then resolves it relative to `self` using embedded
    /// (RFC 1808) semantics.
    ///
    /// Returns an error if parsing fails.
    ///
    /// For more details, see [`AssetPath::resolve_embed`].
    pub fn resolve_embed_str(&self, path: &str) -> Result<AssetPath<'static>, ParseAssetPathError> {
        self.resolve_internal(path, true)
    }

    fn resolve_from_parts(
        &self,
        replace: bool,
        source: Option<&str>,
        rpath: &Path,
        rlabel: Option<&str>,
    ) -> AssetPath<'static> {
        let mut base_path = PathBuf::from(self.path());
        if replace && !self.path.to_str().unwrap().ends_with('/') {
            // No error if base is empty (per RFC 1808).
            base_path.pop();
        }

        // Strip off leading slash
        let mut is_absolute = false;
        let rpath = match rpath.strip_prefix("/") {
            Ok(p) => {
                is_absolute = true;
                p
            }
            _ => rpath,
        };

        let mut result_path = if !is_absolute && source.is_none() {
            base_path
        } else {
            PathBuf::new()
        };
        result_path.push(rpath);
        result_path = normalize_path(result_path.as_path());

        AssetPath {
            source: match source {
                Some(source) => AssetSourceId::Name(CowArc::Owned(source.into())),
                None => self.source.clone_owned(),
            },
            path: CowArc::Owned(result_path.into()),
            label: rlabel.map(|l| CowArc::Owned(l.into())),
        }
    }

    fn resolve_internal(
        &self,
        path: &str,
        replace: bool,
    ) -> Result<AssetPath<'static>, ParseAssetPathError> {
        if let Some(label) = path.strip_prefix('#') {
            // It's a label only
            Ok(self.clone_owned().with_label(label.to_owned()))
        } else {
            let (source, rpath, rlabel) = AssetPath::parse_internal(path)?;
            Ok(self.resolve_from_parts(replace, source, rpath, rlabel))
        }
    }

    /// Returns the full extension (including multiple '.' values).
    /// Ex: Returns `"config.ron"` for `"my_asset.config.ron"`
    ///
    /// Also strips out anything following a `?` to handle query parameters in URIs
    pub fn get_full_extension(&self) -> Option<&str> {
        let file_name = self.path().file_name()?.to_str()?;
        let index = file_name.find('.')?;
        let mut extension = &file_name[index + 1..];

        // Strip off any query parameters
        let query = extension.find('?');
        if let Some(offset) = query {
            extension = &extension[..offset];
        }

        Some(extension)
    }

    /// Returns the extension, excluding multiple `.` values.
    ///
    /// Ex: Returns `"ron"` for `"my_asset.config.ron"`
    ///
    /// Also strips out anything follow a `?` to handle query parameters in URIs.
    pub fn get_extension(&self) -> Option<&str> {
        let full_extension = self.get_full_extension()?;
        Some(match full_extension.rfind(".") {
            None => full_extension,
            Some(index) => &full_extension[(index + 1)..],
        })
    }

    pub(crate) fn iter_secondary_extensions(full_extension: &str) -> impl Iterator<Item = &str> {
        full_extension.char_indices().filter_map(|(i, c)| {
            if c == '.' {
                Some(&full_extension[i + 1..])
            } else {
                None
            }
        })
    }

    /// Returns `true` if this [`AssetPath`] points to a file that is
    /// outside of its [`AssetSourceId`](crate::AssetSourceId) folder.
    ///
    /// ## Example
    /// ```
    /// # use voker_asset::AssetPath;
    /// // Inside the default AssetSource.
    /// let path = AssetPath::parse("thingy.png");
    /// assert!( ! path.is_unapproved());
    /// let path = AssetPath::parse("gui/thingy.png");
    /// assert!( ! path.is_unapproved());
    ///
    /// // Inside a different AssetSource.
    /// let path = AssetPath::parse("embedded://thingy.png");
    /// assert!( ! path.is_unapproved());
    ///
    /// // Exits the `AssetSource`s directory.
    /// let path = AssetPath::parse("../thingy.png");
    /// assert!(path.is_unapproved());
    /// let path = AssetPath::parse("folder/../../thingy.png");
    /// assert!(path.is_unapproved());
    ///
    /// // This references the linux root directory.
    /// let path = AssetPath::parse("/home/thingy.png");
    /// assert!(path.is_unapproved());
    /// ```
    pub fn is_unapproved(&self) -> bool {
        use std::path::Component;
        let mut simplified = PathBuf::new();
        for component in self.path.components() {
            match component {
                Component::Prefix(_) | Component::RootDir => return true,
                Component::CurDir => {}
                Component::ParentDir => {
                    if !simplified.pop() {
                        return true;
                    }
                }
                Component::Normal(os_str) => simplified.push(os_str),
            }
        }

        false
    }
}

impl<'a> Debug for AssetPath<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(self, f)
    }
}

impl<'a> Display for AssetPath<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let AssetSourceId::Name(name) = self.source() {
            write!(f, "{name}://")?;
        }
        write!(f, "{}", self.path.display())?;
        if let Some(label) = &self.label {
            write!(f, "#{label}")?;
        }
        Ok(())
    }
}

impl<'a> Serialize for AssetPath<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AssetPath<'static> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct AssetPathVisitor;

        impl<'de> Visitor<'de> for AssetPathVisitor {
            type Value = AssetPath<'static>;

            fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                formatter.write_str("string AssetPath")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(AssetPath::parse(v).into_owned())
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(AssetPath::from(v))
            }
        }

        deserializer.deserialize_string(AssetPathVisitor)
    }
}

// This is only implemented for static lifetimes to ensure `Path::clone` does not allocate
// by ensuring that this is stored as a `CowArc::Static`.
// Please read https://github.com/bevyengine/bevy/issues/19844 before changing this!
impl From<&'static str> for AssetPath<'static> {
    #[inline]
    fn from(asset_path: &'static str) -> Self {
        let (source, path, label) = Self::parse_internal(asset_path).unwrap();
        AssetPath {
            source: source.into(),
            path: CowArc::Static(path),
            label: label.map(CowArc::Static),
        }
    }
}

impl<'a> From<&'a String> for AssetPath<'a> {
    #[inline]
    fn from(asset_path: &'a String) -> Self {
        AssetPath::parse(asset_path.as_str())
    }
}

impl From<String> for AssetPath<'static> {
    #[inline]
    fn from(asset_path: String) -> Self {
        AssetPath::parse(asset_path.as_str()).into_owned()
    }
}

impl From<&'static Path> for AssetPath<'static> {
    #[inline]
    fn from(path: &'static Path) -> Self {
        Self {
            source: AssetSourceId::Default,
            path: CowArc::Static(path),
            label: None,
        }
    }
}

impl From<PathBuf> for AssetPath<'static> {
    #[inline]
    fn from(path: PathBuf) -> Self {
        Self {
            source: AssetSourceId::Default,
            path: path.into(),
            label: None,
        }
    }
}

impl<'a, 'b> From<&'a AssetPath<'b>> for AssetPath<'b> {
    fn from(value: &'a AssetPath<'b>) -> Self {
        value.clone()
    }
}

impl<'a> From<AssetPath<'a>> for PathBuf {
    fn from(value: AssetPath<'a>) -> Self {
        value.path().to_path_buf()
    }
}
