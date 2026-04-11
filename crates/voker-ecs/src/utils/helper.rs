use crate::component::ComponentId;
use crate::entity::Entity;
use crate::tick::Tick;

/// A SIMD-optimized `contains` for `ComponentId`.
///
/// See: https://godbolt.org/
///
/// With O3 optimization, it is faster than binary search when the number of elements is less than 100.
#[inline(always)]
pub(crate) fn contains_component(id: ComponentId, slice: &[ComponentId]) -> bool {
    let val = unsafe { core::mem::transmute::<ComponentId, u32>(id) };
    let arr = unsafe { core::mem::transmute::<&[ComponentId], &[u32]>(slice) };
    arr.contains(&val)
}

/// A SIMD-optimized `contains` for `Entity`.
///
/// See: https://godbolt.org/
///
/// For u64, it may only be possible to unroll the loop,
/// but it is still faster than directly contains(&entity).
#[inline(always)]
pub(crate) fn contains_entity(id: Entity, slice: &[Entity]) -> bool {
    let val = unsafe { core::mem::transmute::<Entity, u64>(id) };
    let arr = unsafe { core::mem::transmute::<&[Entity], &[u64]>(slice) };
    arr.contains(&val)
}

/// Clamps a tick slice, optimized for bulk processing.
///
/// See: https://godbolt.org/
///
/// Internal note: this performs representation casts to `u32` for better code
/// generation and assumes `Tick` is layout-compatible with `u32`.
pub(crate) fn check_slice_tick(this: &mut [Tick], now: Tick) {
    use core::mem::transmute;

    // `u32` is more easily optimized by compiler.
    let arr = unsafe { transmute::<&mut [Tick], &mut [u32]>(this) };
    let now: u32 = unsafe { transmute::<Tick, u32>(now) };

    let fall_back = now.wrapping_sub(crate::tick::MAX_TICK_AGE);

    // `for_each` can generate better code than explicit `for` loops.
    // At present, it's guaranteed that `wrapping_sub` and `>` are SIMD.
    arr.iter_mut().for_each(|x| {
        let age = now.wrapping_sub(*x);
        if age > crate::tick::MAX_TICK_AGE {
            *x = fall_back;
        }
    });
}
