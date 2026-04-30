//! Batch spawning helpers for relationship-linked entity graphs.
//!
//! [`Spawn`] wraps a [`Bundle`] for deferred relationship spawning.
//! [`SpawnableList`] is the trait that powers `RelatedSpawner` and
//! `RelatedSpawnerCommands` — it lets callers express "spawn N children
//! each with a given bundle" without pre-allocating entity ids.

use core::marker::PhantomData;

use alloc::vec::Vec;

use voker_ptr::OwningPtr;

use crate::bundle::{Bundle, DataBundle};
use crate::component::{ComponentCollector, ComponentWriter};
use crate::entity::Entity;
use crate::relationship::{RelatedSpawner, Relationship, RelationshipTarget};
use crate::utils::DebugLocation;
use crate::world::{EntityOwned, World};

// -----------------------------------------------------------------------------
// Spawn & SpawnableList

#[repr(transparent)]
pub struct Spawn<B: Bundle>(pub B);

pub unsafe trait SpawnableList<R>: Sized {
    unsafe fn spawn(this: OwningPtr<'_>, world: &mut World, entity: Entity);
    fn size_hint(&self) -> usize;
}

// -----------------------------------------------------------------------------
// SpawnableList for Spawn

unsafe impl<R: Relationship, B: Bundle> SpawnableList<R> for Spawn<B> {
    unsafe fn spawn(this: OwningPtr<'_>, world: &mut World, entity: Entity) {
        let caller = DebugLocation::caller();
        this.debug_assert_aligned::<Self>();
        let bundle = unsafe { this.read::<Self>().0 };
        let mut entity = world.spawn_with_caller(R::from_target(entity), caller);
        entity.insert_with_caller(bundle, caller);
    }

    fn size_hint(&self) -> usize {
        1
    }
}

// -----------------------------------------------------------------------------
// SpawnableList for Vec

unsafe impl<R: Relationship, B: DataBundle> SpawnableList<R> for Vec<B> {
    unsafe fn spawn(ptr: OwningPtr<'_>, world: &mut World, entity: Entity) {
        ptr.debug_assert_aligned::<Self>();
        let vec = unsafe { ptr.read::<Self>() };
        let mapped_bundles = vec.into_iter().map(|b| (R::from_target(entity), b));
        world.spawn_batch(mapped_bundles);
    }

    fn size_hint(&self) -> usize {
        self.len()
    }
}

// -----------------------------------------------------------------------------
// SpawnableList for SpawnIter

#[repr(transparent)]
pub struct SpawnIter<I>(pub I);

unsafe impl<R: Relationship, I, B: Bundle> SpawnableList<R> for SpawnIter<I>
where
    I: Iterator<Item = B> + Send + Sync + 'static,
{
    unsafe fn spawn(this: OwningPtr<'_>, world: &mut World, entity: Entity) {
        this.debug_assert_aligned::<Self>();
        let iter = unsafe { this.read::<Self>().0 };
        for bundle in iter {
            world.spawn((R::from_target(entity), bundle));
        }
    }

    fn size_hint(&self) -> usize {
        self.0.size_hint().0
    }
}

// -----------------------------------------------------------------------------
// SpawnableList for SpawnWith

#[repr(transparent)]
pub struct SpawnWith<F>(pub F);

unsafe impl<R: Relationship, F> SpawnableList<R> for SpawnWith<F>
where
    F: FnOnce(&mut RelatedSpawner<R>) + Send + Sync + 'static,
{
    unsafe fn spawn(this: OwningPtr<'_>, world: &mut World, entity: Entity) {
        this.debug_assert_aligned::<Self>();
        let spawn_func = unsafe { this.read::<Self>().0 };
        world.entity_owned(entity).with_related_entities(spawn_func);
    }

    fn size_hint(&self) -> usize {
        1
    }
}

// -----------------------------------------------------------------------------
// SpawnableList for WithRelateds

#[repr(transparent)]
pub struct WithRelateds<I>(pub I);

impl<I> WithRelateds<I> {
    /// Creates a new [`WithRelated`] from a collection of entities.
    pub fn new(iter: impl IntoIterator<IntoIter = I>) -> Self {
        Self(iter.into_iter())
    }
}

unsafe impl<R: Relationship, I: Iterator<Item = Entity>> SpawnableList<R> for WithRelateds<I> {
    unsafe fn spawn(this: OwningPtr<'_>, world: &mut World, entity: Entity) {
        this.debug_assert_aligned::<Self>();
        let spawn_func = unsafe { this.read::<Self>().0 };
        let related = spawn_func.collect::<Vec<Entity>>();
        world.entity_owned(entity).add_relateds::<R>(&related);
    }

    fn size_hint(&self) -> usize {
        self.0.size_hint().0
    }
}

// -----------------------------------------------------------------------------
// SpawnableList for WithRelated

pub struct WithRelated(pub Entity);

unsafe impl<R: Relationship> SpawnableList<R> for WithRelated {
    unsafe fn spawn(this: OwningPtr<'_>, world: &mut World, entity: Entity) {
        this.debug_assert_aligned::<Self>();
        let related = unsafe { this.read::<Self>().0 };
        world.entity_owned(entity).add_related::<R>(related);
    }

    fn size_hint(&self) -> usize {
        1
    }
}

macro_rules! spawnable_list_impl {
    (0: []) => {
        unsafe impl<R: Relationship> SpawnableList<R> for () {
            unsafe fn spawn(_this: OwningPtr<'_>, _world: &mut World, _entity: Entity) {}
            fn size_hint(&self) -> usize { 0 }
       }
    };
    (1: [0: P0]) => {
        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.\n")]
        unsafe impl<R: Relationship, P0: SpawnableList<R>> SpawnableList<R> for (P0, ) {
            unsafe fn spawn(this: OwningPtr<'_>, world: &mut World, entity: Entity) {
                this.debug_assert_aligned::<Self>();
                let offset = ::core::mem::offset_of!(Self, 0);
                unsafe { P0::spawn(this.byte_add(offset), world, entity); }
            }

            fn size_hint(&self) -> usize {
                self.0.size_hint()
            }
       }
    };

    ($num:literal : [$($index:tt : $name:ident),*]) => {
        #[cfg_attr(docsrs, doc(hidden))]
        unsafe impl<R: Relationship, $($name: SpawnableList<R>),*> SpawnableList<R> for ($($name,)*) {
            unsafe fn spawn(mut this: OwningPtr<'_>, world: &mut World, entity: Entity) {
                this.debug_assert_aligned::<Self>();
                $(unsafe {
                    let offset = ::core::mem::offset_of!(Self, $index);
                    <$name>::spawn(this.take_field(offset), world, entity);
                })*
            }

            fn size_hint(&self) -> usize {
                0 $( + self.$index.size_hint() )*
            }
        }
    };
}

voker_utils::range_invoke!(spawnable_list_impl, 12);

// -----------------------------------------------------------------------------
// SpawnRelatedList

pub struct SpawnRelatedList<R: Relationship, L: SpawnableList<R>> {
    list: L,
    _marker: PhantomData<R>,
}

unsafe impl<R, L> Bundle for SpawnRelatedList<R, L>
where
    R: Relationship,
    L: SpawnableList<R> + Send + Sync + 'static,
{
    const NEED_APPLY_EFFECT: bool = true;

    fn collect_explicit(collector: &mut ComponentCollector) {
        <R::RelationshipTarget as Bundle>::collect_explicit(collector)
    }

    fn collect_required(collector: &mut ComponentCollector) {
        <R::RelationshipTarget as Bundle>::collect_required(collector)
    }

    unsafe fn write_explicit(data: OwningPtr<'_>, writer: &mut ComponentWriter) {
        data.debug_assert_aligned::<Self>();
        let size_hint = unsafe { data.as_ref::<Self>().list.size_hint() };
        let target = <R::RelationshipTarget as RelationshipTarget>::with_hint(size_hint);
        unsafe { writer.write_custom(target) };
    }

    unsafe fn write_required(writer: &mut ComponentWriter) {
        unsafe { <R::RelationshipTarget as Bundle>::write_required(writer) }
    }

    unsafe fn apply_effect(ptr: OwningPtr<'_>, entity: &mut EntityOwned) {
        ptr.debug_assert_aligned::<Self>();
        let offset = ::core::mem::offset_of!(Self, list);
        let list = unsafe { ptr.byte_add(offset) };

        if entity.is_despawned() {
            return;
        }

        let id = entity.entity();
        entity.world_scope(|world: &mut World| unsafe {
            L::spawn(list, world, id);
        });
    }
}

// -----------------------------------------------------------------------------
// SpawnRelatedBundle

pub struct SpawnRelatedBundle<R: Relationship, B: Bundle> {
    bundle: B,
    _marker: PhantomData<R>,
}

unsafe impl<R: Relationship, B: Bundle> Bundle for SpawnRelatedBundle<R, B> {
    const NEED_APPLY_EFFECT: bool = true;

    fn collect_explicit(collector: &mut ComponentCollector) {
        <R::RelationshipTarget as Bundle>::collect_explicit(collector)
    }

    fn collect_required(collector: &mut ComponentCollector) {
        <R::RelationshipTarget as Bundle>::collect_required(collector)
    }

    unsafe fn write_explicit(_data: OwningPtr<'_>, writer: &mut ComponentWriter) {
        let target = <R::RelationshipTarget as RelationshipTarget>::with_hint(1);
        unsafe { writer.write_custom(target) };
    }

    unsafe fn write_required(writer: &mut ComponentWriter) {
        unsafe { <R::RelationshipTarget as Bundle>::write_required(writer) }
    }

    unsafe fn apply_effect(ptr: OwningPtr<'_>, entity: &mut EntityOwned) {
        ptr.debug_assert_aligned::<Self>();
        let this = unsafe { ptr.read::<Self>() };

        if entity.is_despawned() {
            return;
        }

        entity.with_related::<R>(this.bundle);
    }
}

// -----------------------------------------------------------------------------
// SpawnRelated

pub trait SpawnRelated: RelationshipTarget {
    fn spawn<L: SpawnableList<Self::Relationship>>(
        list: L,
    ) -> SpawnRelatedList<Self::Relationship, L>;
    fn spawn_one<B: Bundle>(bundle: B) -> SpawnRelatedBundle<Self::Relationship, B>;
}

impl<T: RelationshipTarget> SpawnRelated for T {
    fn spawn<L: SpawnableList<Self::Relationship>>(
        list: L,
    ) -> SpawnRelatedList<Self::Relationship, L> {
        SpawnRelatedList {
            list,
            _marker: PhantomData,
        }
    }

    fn spawn_one<B: Bundle>(bundle: B) -> SpawnRelatedBundle<Self::Relationship, B> {
        SpawnRelatedBundle {
            bundle,
            _marker: PhantomData,
        }
    }
}

// -----------------------------------------------------------------------------
// related

#[macro_export]
macro_rules! related {
    ($relationship_target:ty [$($child:expr),*$(,)?]) => {
       <$relationship_target as $crate::spawn::SpawnRelated>::spawn($crate::recursive_spawn!($($child),*))
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! recursive_spawn {
    // direct expansion
    () => { () };
    ($a:expr) => {
        $crate::spawn::Spawn($a)
    };
    ($a:expr, $b:expr) => {
        (
            $crate::spawn::Spawn($a),
            $crate::spawn::Spawn($b),
        )
    };
    ($a:expr, $b:expr, $c:expr) => {
        (
            $crate::spawn::Spawn($a),
            $crate::spawn::Spawn($b),
            $crate::spawn::Spawn($c),
        )
    };
    ($a:expr, $b:expr, $c:expr, $d:expr) => {
        (
            $crate::spawn::Spawn($a),
            $crate::spawn::Spawn($b),
            $crate::spawn::Spawn($c),
            $crate::spawn::Spawn($d),
        )
    };
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr) => {
        (
            $crate::spawn::Spawn($a),
            $crate::spawn::Spawn($b),
            $crate::spawn::Spawn($c),
            $crate::spawn::Spawn($d),
            $crate::spawn::Spawn($e),
        )
    };
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr) => {
        (
            $crate::spawn::Spawn($a),
            $crate::spawn::Spawn($b),
            $crate::spawn::Spawn($c),
            $crate::spawn::Spawn($d),
            $crate::spawn::Spawn($e),
            $crate::spawn::Spawn($f),
        )
    };
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr, $g:expr) => {
        (
            $crate::spawn::Spawn($a),
            $crate::spawn::Spawn($b),
            $crate::spawn::Spawn($c),
            $crate::spawn::Spawn($d),
            $crate::spawn::Spawn($e),
            $crate::spawn::Spawn($f),
            $crate::spawn::Spawn($g),
        )
    };
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr, $g:expr, $h:expr) => {
        (
            $crate::spawn::Spawn($a),
            $crate::spawn::Spawn($b),
            $crate::spawn::Spawn($c),
            $crate::spawn::Spawn($d),
            $crate::spawn::Spawn($e),
            $crate::spawn::Spawn($f),
            $crate::spawn::Spawn($g),
            $crate::spawn::Spawn($h),
        )
    };
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr, $g:expr, $h:expr, $i:expr) => {
        (
            $crate::spawn::Spawn($a),
            $crate::spawn::Spawn($b),
            $crate::spawn::Spawn($c),
            $crate::spawn::Spawn($d),
            $crate::spawn::Spawn($e),
            $crate::spawn::Spawn($f),
            $crate::spawn::Spawn($g),
            $crate::spawn::Spawn($h),
            $crate::spawn::Spawn($i),
        )
    };
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr, $g:expr, $h:expr, $i:expr, $j:expr) => {
        (
            $crate::spawn::Spawn($a),
            $crate::spawn::Spawn($b),
            $crate::spawn::Spawn($c),
            $crate::spawn::Spawn($d),
            $crate::spawn::Spawn($e),
            $crate::spawn::Spawn($f),
            $crate::spawn::Spawn($g),
            $crate::spawn::Spawn($h),
            $crate::spawn::Spawn($i),
            $crate::spawn::Spawn($j),
        )
    };
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr, $g:expr, $h:expr, $i:expr, $j:expr, $k:expr) => {
        (
            $crate::spawn::Spawn($a),
            $crate::spawn::Spawn($b),
            $crate::spawn::Spawn($c),
            $crate::spawn::Spawn($d),
            $crate::spawn::Spawn($e),
            $crate::spawn::Spawn($f),
            $crate::spawn::Spawn($g),
            $crate::spawn::Spawn($h),
            $crate::spawn::Spawn($i),
            $crate::spawn::Spawn($j),
            $crate::spawn::Spawn($k),
        )
    };

    // recursive expansion
    (
        $a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr,
        $g:expr, $h:expr, $i:expr, $j:expr, $k:expr, $($rest:expr),*
    ) => {
        (
            $crate::spawn::Spawn($a),
            $crate::spawn::Spawn($b),
            $crate::spawn::Spawn($c),
            $crate::spawn::Spawn($d),
            $crate::spawn::Spawn($e),
            $crate::spawn::Spawn($f),
            $crate::spawn::Spawn($g),
            $crate::spawn::Spawn($h),
            $crate::spawn::Spawn($i),
            $crate::spawn::Spawn($j),
            $crate::spawn::Spawn($k),
            $crate::recursive_spawn!($($rest),*)
        )
    };
}
