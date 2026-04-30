#![expect(clippy::module_inception, reason = "For better structure.")]

use voker_ptr::OwningPtr;
use voker_utils::range_invoke;

use crate::component::{Component, ComponentCollector, ComponentWriter};
use crate::world::EntityOwned;

/// A trait for types that represent a bundle (combination) of component data.
///
/// A `Bundle` describes how to register component types and how to write their
/// data into storage. The [`ComponentCollector`] and [`ComponentWriter`] utilities
/// are provided to simplify these operations.
///
/// Types implementing `Bundle` can be used for entity spawning and component
/// insertion/removal. All [`Component`] types implement this trait, meaning a
/// single component can be used as a bundle.
///
/// For custom types where all fields implement `Bundle`, the `#[derive(Bundle)]`
/// macro can generate a safe implementation automatically.
///
/// ```no_run
/// # use voker_ecs::prelude::*;
/// # let mut world = World::alloc();
///
/// #[derive(Bundle)]
/// struct Foo { /* .. */ }
///
/// world.spawn(Foo { /* .. */ });
/// ```
///
/// The default derive implementation is `DataBundle`. If there are fields
/// inside that require `apply_effect`, please manually annotate with `#[bundle(effect)]`.
///
/// ```no_run
/// # use voker_ecs::prelude::*;
/// #[derive(Bundle)]
/// #[bundle(effect)]
/// struct Foo { /* .. */ }
/// ```
///
/// # Three-Phase Write Model
///
/// Bundle application is split into three ordered phases that together consume
/// the entire bundle value exactly once:
///
/// ```text
/// collect_explicit / collect_required   (type registration — no data movement)
///          ↓
///     write_explicit                    (move component fields into storage)
///          ↓
///     write_required                    (default-initialise missing dependencies)
///          ↓
///     apply_effect   ← only when NEED_APPLY_EFFECT = true
///                       (post-spawn side effects using remaining fields)
/// ```
///
/// ## Phase 1 — `write_explicit`
///
/// Receives `data: OwningPtr<'_>` pointing to the entire bundle allocation.
/// Each component field **must** be moved out of `data` and handed to
/// [`ComponentWriter`]. Any field that is moved here is fully consumed and
/// its bytes become invalid for the rest of the bundle's lifetime.
///
/// For pure-data bundles (`NEED_APPLY_EFFECT = false`) this phase must consume
/// **every** field; leaving any field unconsumed is a memory leak.
///
/// ## Phase 2 — `write_required`
///
/// Default-initialises any dependency components that were not provided
/// explicitly. Takes no `data` pointer — all values are synthesised from
/// default constructors. Must not overwrite data already written in phase 1.
///
/// ## Phase 3 — `apply_effect`
///
/// Receives `ptr: OwningPtr<'_>` pointing to the same allocation as phase 1.
/// Only invoked when [`NEED_APPLY_EFFECT`] is `true`.
///
/// **Ownership invariant across phases 1 and 3:**
///
/// Every byte of the bundle's memory must be consumed **exactly once** across
/// [`write_explicit`] and [`apply_effect`] combined:
///
/// - Fields that were moved out (consumed) in [`write_explicit`] must **not**
///   be accessed in [`apply_effect`] — their bytes are no longer valid.
/// - Fields that were intentionally *not* consumed in [`write_explicit`]
///   (because they carry post-spawn logic rather than component data) **must**
///   be consumed in [`apply_effect`]. Failing to do so leaks memory.
///
/// When `apply_effect` runs, all components are fully committed to storage
/// and accessible through the provided [`EntityOwned`] handle.
///
/// ## Explicit vs. Required Components
///
/// A bundle type may be associated with **two** distinct `BundleId`s:
/// - The **explicit** component set (user-declared components)
/// - The **required** component set (explicit + all dependency components)
///
/// This distinction matters because:
/// - Spawn and insertion need all components (explicit + required).
/// - Removal only removes the explicitly declared components.
///
/// ## ComponentWriter
///
/// [`ComponentWriter`] tracks the write status of every component type with
/// three states: **unwritten**, **default**, and **explicitly written**.
/// Higher-priority writes override lower-priority ones without causing leaks.
/// If a bundle contains the same component type in multiple fields, the last
/// write wins.
///
/// ## Effects and Batch Spawn
///
/// Effect processing requires per-entity world access and cannot be applied
/// during batch spawn. [`spawn_batch`] asserts at **compile time** that
/// [`NEED_APPLY_EFFECT`] is `false` for the bundle type.
///
/// [`spawn_batch`]: crate::world::World::spawn_batch
/// [`NEED_APPLY_EFFECT`]: Self::NEED_APPLY_EFFECT
/// [`apply_effect`]: Self::apply_effect
/// [`write_explicit`]: Self::write_explicit
///
/// # Safety
///
/// Implementations must satisfy the following invariants:
///
/// **Registration:**
/// - `collect_explicit` must register exactly the component types consumed by
///   `write_explicit`.
/// - `collect_required` must register every component type present after full
///   bundle initialisation (explicit + required).
/// - `write_explicit` and `write_required` may only write types registered by
///   `collect_required`.
/// - `write_required` must not overwrite components already written explicitly.
///
/// **Storage writes:**
/// - Every [`ComponentWriter`] call must respect storage bounds, alignment, and
///   the type identity of the target storage slot.
///
/// **Data ownership:**
/// - Every byte of the bundle's memory must be consumed exactly once across
///   [`write_explicit`] and [`apply_effect`] combined:
///   - Bytes moved out by `write_explicit` are invalid in `apply_effect`.
///   - Bytes left unconsumed by `write_explicit` must be consumed by
///     `apply_effect`, or a memory leak results.
/// - If `NEED_APPLY_EFFECT` is `false`, `apply_effect` must be a no-op and
///   `write_explicit` must consume **all** fields. The caller may elide the
///   `apply_effect` call entirely.
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
    /// Whether this bundle type needs [`apply_effect`] to be called after spawn
    /// or insertion.
    ///
    /// When `false`, the effect pass is skipped entirely (zero overhead). Set
    /// this to `true` only for bundles that contain field types with
    /// post-spawn side effects.
    ///
    /// For composite bundles (tuples and `#[derive(Bundle)]` structs), this
    /// constant is derived automatically: it is `true` if **any** constituent
    /// field has `NEED_APPLY_EFFECT = true`, and `false` otherwise.
    ///
    /// # Batch Spawn Restriction
    ///
    /// `spawn_batch` asserts at **compile time** that this constant is `false`.
    /// Bundles with effects cannot be batch-spawned.
    ///
    /// [`apply_effect`]: Self::apply_effect
    const NEED_APPLY_EFFECT: bool;

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
    unsafe fn write_explicit(data: OwningPtr<'_>, writer: &mut ComponentWriter);

    /// Writes required component values that **haven't** been provided explicitly.
    ///
    /// This method initializes required-but-missing components via default
    /// construction, while preserving explicitly provided values.
    ///
    /// # Safety
    /// - All component writes must be to types that were registered.
    /// - All writes must be within allocated storage bounds
    /// - Component data must be properly aligned
    /// - The type being written must match the registered component type
    unsafe fn write_required(writer: &mut ComponentWriter);

    /// Performs post-spawn side effects on the entity after all component data
    /// has been written to storage.
    ///
    /// This is the third phase of bundle application (see the [Effects] section
    /// of the [`Bundle`] documentation for the full execution order). It is only
    /// invoked when [`NEED_APPLY_EFFECT`] is `true`; otherwise the caller skips
    /// the call entirely.
    ///
    /// # Parameters
    ///
    /// - `ptr` — an owning pointer to the raw bundle memory. Only byte ranges
    ///   corresponding to field types that have `NEED_APPLY_EFFECT = true` are
    ///   still valid; all other bytes were moved into component storage by
    ///   [`write_explicit`] and must not be accessed.
    /// - `entity` — a mutable handle to the entity that was just spawned or
    ///   modified. Use it to inspect or further modify the entity (e.g., insert
    ///   additional components, set up relationships).
    ///
    /// # Composite Bundles
    ///
    /// For tuple bundles and `#[derive(Bundle)]` structs, `apply_effect` is
    /// forwarded to each constituent field in declaration order, using
    /// `core::mem::offset_of!` to compute the field's byte offset within `ptr`.
    ///
    /// # Safety
    ///
    /// - `data` must point to a live allocation valid for the full layout of `Self`.
    /// - [`write_explicit`] must have already been called so that component bytes
    ///   at their respective offsets have been moved into storage.
    /// - Only bytes belonging to effect fields (those with `NEED_APPLY_EFFECT =
    ///   true`) may be read through `data`; accessing moved-out bytes is undefined
    ///   behaviour.
    /// - `entity` must refer to a valid, live entity in the world.
    ///
    /// [Effects]: Bundle#effects
    /// [`NEED_APPLY_EFFECT`]: Self::NEED_APPLY_EFFECT
    /// [`write_explicit`]: Self::write_explicit
    unsafe fn apply_effect(data: OwningPtr<'_>, entity: &mut EntityOwned);
}

/// Marker supertrait for [`Bundle`] types that contain only pure data and
/// never produce post-spawn side effects.
///
/// All [`Component`] types and the empty tuple `()` implement this trait
/// automatically. A tuple or `#[derive(Bundle)]` struct also implements
/// `DataBundle` when **every** one of its fields implements `DataBundle`.
///
/// Implementing this trait that [`Bundle::NEED_APPLY_EFFECT`] must be `false`.
pub unsafe trait DataBundle: Bundle {}

/// Automatic implementation of [`Bundle`] for any single component.
///
/// This allows using individual component types directly as bundles for
/// convenience when spawning entities with only one component.
unsafe impl<T: Component> Bundle for T {
    const NEED_APPLY_EFFECT: bool = false;

    fn collect_explicit(collector: &mut ComponentCollector) {
        collector.collect_explicit::<T>();
    }

    fn collect_required(collector: &mut ComponentCollector) {
        collector.collect_required::<T>();
    }

    unsafe fn write_explicit(data: OwningPtr<'_>, writer: &mut ComponentWriter) {
        unsafe { writer.write_explicit::<T>(data) };
    }

    unsafe fn write_required(writer: &mut ComponentWriter) {
        if let Some(required) = T::REQUIRED {
            unsafe {
                required.write(writer);
            }
        }
    }

    unsafe fn apply_effect(_consumed: OwningPtr<'_>, _entity: &mut EntityOwned) {}
}

unsafe impl<T: Component> DataBundle for T {}

macro_rules! impl_bundle_for_tuple {
    (0: []) => {
        unsafe impl DataBundle for () {}

        unsafe impl Bundle for () {
            const NEED_APPLY_EFFECT: bool = false;
            fn collect_explicit(_collector: &mut ComponentCollector) {}
            fn collect_required(_collector: &mut ComponentCollector) {}
            unsafe fn write_explicit(_data: OwningPtr<'_>, _writer: &mut ComponentWriter) {}
            unsafe fn write_required(_writer: &mut ComponentWriter) {}
            unsafe fn apply_effect(_ptr: OwningPtr<'_>, _entity: &mut EntityOwned) {}
        }
    };
    (1 : [ $index:tt : $name:ident ]) => {
        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 15 items long.\n")]
        unsafe impl<$name: DataBundle> DataBundle for ($name,) {}

        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 15 items long.\n")]
        #[cfg_attr(docsrs, doc = "For larger data, consider using #[derive(Bundle)] to create custom types.")]
        unsafe impl<$name: Bundle> Bundle for ($name,) {
            const NEED_APPLY_EFFECT: bool = <$name as Bundle>::NEED_APPLY_EFFECT;

            fn collect_explicit(collector: &mut ComponentCollector) {
                <$name>::collect_explicit(collector)
            }

            fn collect_required(collector: &mut ComponentCollector) {
                <$name>::collect_required(collector)
            }

            unsafe fn write_explicit(data: OwningPtr<'_>, writer: &mut ComponentWriter) {
                let offset = ::core::mem::offset_of!(Self, 0);
                unsafe { <$name>::write_explicit(data.byte_add(offset), writer); }
            }

            unsafe fn write_required(writer: &mut ComponentWriter) {
                unsafe { <$name>::write_required(writer); }
            }

            unsafe fn apply_effect(data: OwningPtr<'_>, entity: &mut EntityOwned) {
                let offset = ::core::mem::offset_of!(Self, 0);
                unsafe { <$name>::apply_effect(data.byte_add(offset), entity); }
            }
        }
    };
    ($num:literal : [$($index:tt : $name:ident),*]) => {
        #[cfg_attr(docsrs, doc(hidden))]
        unsafe impl<$($name: DataBundle),*> DataBundle for ($($name,)*) {}

        #[cfg_attr(docsrs, doc(hidden))]
        unsafe impl<$($name: Bundle),*> Bundle for ($($name,)*) {
            const NEED_APPLY_EFFECT: bool = false $( || <$name as Bundle>::NEED_APPLY_EFFECT )*;

            fn collect_explicit(collector: &mut ComponentCollector) {
                $( <$name>::collect_explicit(collector); )*
            }

            fn collect_required(collector: &mut ComponentCollector) {
                $( <$name>::collect_required(collector); )*
            }

            unsafe fn write_explicit(mut data: OwningPtr<'_>, writer: &mut ComponentWriter) {
                $(unsafe {
                    let offset = ::core::mem::offset_of!(Self, $index);
                    <$name>::write_explicit(data.take_field(offset), writer);
                })*
            }

            unsafe fn write_required(writer: &mut ComponentWriter) {
                $(unsafe { <$name>::write_required(writer); })*
            }

            unsafe fn apply_effect(mut data: OwningPtr<'_>, entity: &mut EntityOwned) {
                $(unsafe {
                    let offset = ::core::mem::offset_of!(Self, $index);
                    <$name>::apply_effect(data.take_field(offset), entity);
                })*
            }
        }
    };
}

range_invoke!(impl_bundle_for_tuple, 15);
