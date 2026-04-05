use voker_utils::range_invoke;

use crate::component::{Component, ComponentCollector, ComponentWriter};

/// A trait for types that represent a bundle (combination) of component data.
///
/// # Safety
///
/// Implementations must satisfy the following invariants:
/// - `collect_explicit` must register exactly the component types that can be
///   written by `write_explicit`.
/// - `collect_required` must register every component type that may exist after
///   bundle initialization, including explicit types and dependency-required
///   types.
/// - `write_explicit` and `write_required` must only write component values for
///   types previously registered by `collect_required`.
/// - `write_required` must not overwrite explicitly provided values; it should
///   only initialize required components that are still missing.
/// - Every write performed through `ComponentWriter` must respect storage
///   bounds, alignment, and type correctness expected by the target storage.
///
/// [`collect_explicit`]: Self::collect_explicit
/// [`collect_required`]: Self::collect_required
/// [`write_explicit`]: Self::write_explicit
/// [`write_required`]: Self::write_required
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a bundle",
    label = "invalid bundle",
    note = "Consider annotating `{Self}` with `#[derive(Bundle)]`."
)]
pub unsafe trait Bundle: Sized + Sync + Send + 'static {
    /// Registers and collects all explicitly declared component types
    /// provided by this bundle.
    ///
    /// This is usually used for removing components.
    fn collect_explicit(collector: &mut ComponentCollector);

    /// Registers and collects all required component types for this bundle.
    ///
    /// Required components include both explicitly declared components and
    /// their dependency-required components.
    fn collect_required(collector: &mut ComponentCollector);

    /// Writes all explicitly provided component data to storage.
    ///
    /// This method handles components that are directly provided in the bundle.
    /// If duplicate components exist (e.g., in tuple implementations), later
    /// fields override earlier ones.
    ///
    /// # Safety
    /// - All component writes must be to types that were registered in
    ///   `collect_explicit` or `collect_required` .
    /// - All writes must be within allocated storage bounds
    /// - Component data must be properly aligned
    /// - The type being written must match the registered component type
    /// - The `base` offset must be valid for the current storage context
    unsafe fn write_explicit(writer: &mut ComponentWriter, base: usize);

    /// Writes required component values that **haven't** been provided explicitly.
    ///
    /// This method initializes required-but-missing components via default
    /// construction, while preserving explicitly provided values.
    ///
    /// # Safety
    /// - All component writes must be to types that were registered in
    ///   [`collect_required`](Self::collect_required).
    /// - All writes must be within allocated storage bounds
    /// - Component data must be properly aligned
    /// - The type being written must match the registered component type
    unsafe fn write_required(writer: &mut ComponentWriter);
}

/// Automatic implementation of [`Bundle`] for any single component.
///
/// This allows using individual component types directly as bundles for
/// convenience when spawning entities with only one component.
unsafe impl<T: Component> Bundle for T {
    fn collect_explicit(collector: &mut ComponentCollector) {
        collector.collect_explicit::<T>();
    }

    fn collect_required(collector: &mut ComponentCollector) {
        collector.collect_required::<T>();
    }

    unsafe fn write_explicit(writer: &mut ComponentWriter, base: usize) {
        unsafe {
            writer.write_explicit::<T>(base);
        }
    }

    unsafe fn write_required(writer: &mut ComponentWriter) {
        if let Some(required) = T::REQUIRED {
            unsafe {
                required.write(writer);
            }
        }
    }
}

macro_rules! impl_bundle_for_tuple {
    (0: []) => {
        unsafe impl Bundle for () {
            fn collect_explicit(_collector: &mut ComponentCollector) {}
            fn collect_required(_collector: &mut ComponentCollector) {}
            unsafe fn write_explicit( _writer: &mut ComponentWriter, _base: usize,) {}
            unsafe fn write_required(_writer: &mut ComponentWriter) {}
        }
    };
    (1 : [ $index:tt : $name:ident ]) => {
        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        unsafe impl<$name: Bundle> Bundle for ($name,) {
            fn collect_explicit(collector: &mut ComponentCollector) {
                <$name>::collect_explicit(collector)
            }

            fn collect_required(collector: &mut ComponentCollector) {
                <$name>::collect_required(collector)
            }

            unsafe fn write_explicit(writer: &mut ComponentWriter, base: usize) {
                const { assert!(::core::mem::offset_of!(Self, 0) == 0); }
                unsafe { <$name>::write_explicit(writer, base); }
            }

            unsafe fn write_required(writer: &mut ComponentWriter) {
                unsafe { <$name>::write_required(writer); }
            }
        }
    };
    ($num:literal : [$($index:tt : $name:ident),*]) => {
        #[cfg_attr(docsrs, doc(hidden))]
        unsafe impl<$($name: Bundle),*> Bundle for ($($name,)*) {
            fn collect_explicit(collector: &mut ComponentCollector) {
                $( <$name>::collect_explicit(collector); )*
            }

            fn collect_required(collector: &mut ComponentCollector) {
                $( <$name>::collect_required(collector); )*
            }

            unsafe fn write_explicit(writer: &mut ComponentWriter, base: usize) {
                $(unsafe {
                    let offset = ::core::mem::offset_of!(Self, $index) + base;
                    <$name>::write_explicit(writer, offset);
                })*
            }

            unsafe fn write_required(writer: &mut ComponentWriter) {
                $(unsafe { <$name>::write_required(writer); })*
            }
        }
    };
}

range_invoke!(impl_bundle_for_tuple, 12);
