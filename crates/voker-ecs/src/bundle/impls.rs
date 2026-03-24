use voker_utils::range_invoke;

use crate::component::{Component, ComponentCollector, ComponentWriter};

/// A trait for types that can be used as bundles of components.
///
/// # Overview
/// Bundles provide a way to group multiple components together for efficient
/// insertion and spawning operations. They serve as a convenience layer that
/// abstracts away the complexity of managing individual components when creating
/// or modifying entities.
///
/// # Implementation
/// Bundles can be created in two ways:
/// - **Derived automatically**: For tuples of components (up to 16 elements)
/// - **Manual implementation**: For custom bundle types with special requirements
///
/// # Bundle Lifecycle
/// When a bundle is used to spawn an entity, the following process occurs:
/// 1. **Component Collection**: All component types in the bundle are collected
///    via [`collect_components`](Self::collect_components). This phase determines
///    which component types this bundle provides.
/// 2. **Archetype Resolution**: The ECS finds or creates an archetype matching
///    the collected component set.
/// 3. **Storage Allocation**: Space is allocated in the appropriate archetype.
/// 4. **Component Writing**: Component values are written to storage via
///    [`write_explicit`](Self::write_explicit) and [`write_required`](Self::write_required).
///
/// # Writing Semantics
/// The bundle trait distinguishes between two types of component writes:
///
/// ## Explicit Fields ([`write_explicit`](Self::write_explicit))
/// - Writes components that are explicitly provided in the bundle
/// - Later fields override earlier ones if duplicates occur
/// - Used for primary component initialization
///
/// ## Required Components ([`write_required`](Self::write_required))
/// - Writes components that must exist but may not be explicitly provided
/// - Only writes if the component hasn't been written yet
/// - Useful for default values or required dependencies
///
/// # Safety
///
/// Implementing this trait manually requires careful attention to memory layout
/// and component invariants. The implementor must ensure:
///
/// ## Collection Safety
/// - [`collect_components`](Self::collect_components) must register **all**
///   component types that this bundle can write (both explicit and required)
/// - Component IDs must be valid and properly initialized
/// - Duplicate registrations are allowed and will be deduplicated
/// - Registration order does **not** need to match write order
///
/// ## Writing Safety
/// - [`write_explicit`](Self::write_explicit) and [`write_required`](Self::write_required)
///   must write components at the correct memory offsets
/// - Writes must not overflow the allocated storage
/// - Component data must be properly aligned
/// - Type safety: The written data must match the registered component type
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a bundle",
    label = "invalid bundle",
    note = "Consider annotating `{Self}` with `#[derive(Bundle)]`."
)]
pub unsafe trait Bundle: Sized + Sync + Send + 'static {
    /// Collects all component types that this bundle can provide.
    ///
    /// This method is called during bundle processing to determine the complete
    /// set of component types that this bundle might write. The collected set
    /// is used to find or create the appropriate archetype for entities spawned
    /// with this bundle.
    ///
    /// # Safety
    /// - Every component type written in [`write_explicit`](Self::write_explicit) or
    ///   [`write_required`](Self::write_required) **must** be registered here.
    /// - Registering extra component types that are never written is disallowed.
    fn collect_components(collector: &mut ComponentCollector);

    /// Writes explicitly provided component values to storage.
    ///
    /// This method handles components that are directly provided in the bundle.
    /// If duplicate components exist (e.g., in tuple implementations), later
    /// fields override earlier ones.
    ///
    /// # Safety
    /// - All component writes must be to types that were registered in
    ///   [`collect_components`](Self::collect_components)
    /// - All writes must be within allocated storage bounds
    /// - Component data must be properly aligned
    /// - The type being written must match the registered component type
    /// - The `base` offset must be valid for the current storage context
    unsafe fn write_explicit(writer: &mut ComponentWriter, base: usize);

    /// Writes required component values that haven't been provided explicitly.
    ///
    /// This method ensures that all necessary components exist for an entity,
    /// even if they weren't included in the explicit bundle fields. It only
    /// writes components that haven't been written yet, preserving any
    /// user-provided values.
    ///
    /// # Safety
    /// - All component writes must be to types that were registered in
    ///   [`collect_components`](Self::collect_components)
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
    fn collect_components(collector: &mut ComponentCollector) {
        collector.collect::<T>();
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
            fn collect_components(_collector: &mut ComponentCollector) {}
            unsafe fn write_explicit( _writer: &mut ComponentWriter, _base: usize,) {}
            unsafe fn write_required(_writer: &mut ComponentWriter) {}
        }
    };
    (1 : [ $index:tt : $name:ident ]) => {
        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        unsafe impl<$name: Bundle> Bundle for ($name,) {
            fn collect_components(collector: &mut ComponentCollector) {
                <$name>::collect_components(collector)
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
            fn collect_components(collector: &mut ComponentCollector) {
                $( <$name>::collect_components(collector); )*
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
