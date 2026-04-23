//! Provide `FixedHasher` and `NoopHasher`.
//!
//! `FixedHasher` based on `foldhash` crate,
//! Provide stable hash results through a fixed hash seed.
//!
//! `NoopHasher` directly use u64 or bit data as hash values.

use core::fmt::Debug;
use core::hash::{BuildHasher, Hasher};

use foldhash::fast::{FixedState, FoldHasher};

// -----------------------------------------------------------------------------
// FixedHasher

/// A fixed hash seed.
const FIXED_HASH_STATE: FixedState = FixedState::with_seed(0x95EE04C4F326B271);

/// A fixed hasher provided hash results that only related on the input.
///
/// A type alias for [`foldhash::fast::FoldHasher`] .
///
/// Which can be created through [`FixedHashState::build_hasher`].
pub type FixedHasher = FoldHasher<'static>;

/// Fixed Hash State based upon a random but fixed seed.
///
/// Based on `foldhash`, but changed the fixed seed.
///
/// # Examples
///
/// ```
/// use core::hash::{Hash, Hasher, BuildHasher};
/// use voker_utils::hash::FixedHashState;
///
/// let mut hasher = FixedHashState.build_hasher();
/// 3.hash(&mut hasher);
/// let result = hasher.finish();
///
/// println!("Hash Result {result}"); // Fixed Result
/// ```
#[derive(Copy, Clone, Default, Debug)]
pub struct FixedHashState;

impl BuildHasher for FixedHashState {
    type Hasher = FixedHasher;

    #[inline(always)]
    fn build_hasher(&self) -> Self::Hasher {
        FIXED_HASH_STATE.build_hasher()
    }

    #[inline(always)]
    fn hash_one<T: core::hash::Hash>(&self, x: T) -> u64
    where
        Self: Sized,
        Self::Hasher: Hasher,
    {
        FIXED_HASH_STATE.hash_one(x)
    }
}

// -----------------------------------------------------------------------------
// NoopHasher

/// A no-op hash that directly pass value through `u64`.
///
/// Which can be created through [`NoopHashState::build_hasher`].
#[derive(Copy, Clone, Default, Debug)]
#[repr(transparent)]
pub struct NoopHasher {
    hash: u64,
}

impl Hasher for NoopHasher {
    #[inline(always)]
    fn finish(&self) -> u64 {
        self.hash
    }

    #[inline(always)]
    fn write_usize(&mut self, i: usize) {
        self.hash = i as u64;
    }

    #[inline(always)]
    fn write_u64(&mut self, i: u64) {
        self.hash = i;
    }

    #[inline(always)]
    fn write_u32(&mut self, i: u32) {
        self.hash = i as u64;
    }

    #[inline(always)]
    fn write_u16(&mut self, i: u16) {
        self.hash = i as u64;
    }

    #[inline(always)]
    fn write_u8(&mut self, i: u8) {
        self.hash = i as u64;
    }

    fn write(&mut self, bytes: &[u8]) {
        // Usually recommended to use `write_u64` directly
        for byte in bytes.iter().rev() {
            // rotate left ensure that `write_u32(10)` is eq to `write_u64(10)`.
            self.hash = self.hash.rotate_left(8).wrapping_add(*byte as u64);
        }
    }
}

/// A fixed hasher without any additional operations.
///
/// Only storing one `u64` and assigning values directly by `writa_u64`.
///
/// Other method will call `write`, which will add the input bytes in reverse
/// order to `u64`, and make it rotate left. Ensure that the results of
/// `write_u64(1234)` and `write_i32(1234)` are the same **if only called once**.
///
/// # Examples
///
/// ```
/// use core::hash::{Hash, Hasher, BuildHasher};
/// use voker_utils::hash::NoopHashState;
///
/// let mut hasher = NoopHashState.build_hasher();
/// 3.hash(&mut hasher);
/// let result = hasher.finish();
///
/// assert_eq!(result, 3_u64);
/// ```
#[derive(Copy, Clone, Default, Debug)]
pub struct NoopHashState;

impl BuildHasher for NoopHashState {
    type Hasher = NoopHasher;

    #[inline(always)]
    fn build_hasher(&self) -> Self::Hasher {
        NoopHasher { hash: 0 }
    }

    #[inline(always)]
    fn hash_one<T: core::hash::Hash>(&self, x: T) -> u64
    where
        Self: Sized,
        Self::Hasher: Hasher,
    {
        let mut hasher = const { NoopHasher { hash: 0 } };
        x.hash(&mut hasher);
        hasher.hash
    }
}

// -----------------------------------------------------------------------------
// SparseHasher

/// A fast hasher that provides uniformly distributed values starting from 0.
///
/// Which can be created through [`SparseHashState::build_hasher`].
#[derive(Copy, Clone, Default, Debug)]
#[repr(transparent)]
pub struct SparseHasher {
    hash: u64,
}

/// From `bevy_ecs`.
///
/// SwissTable (and thus `hashbrown`) cares about two things from the hash:
///
/// - H1: low bits (masked by `2ⁿ-1`) to pick the slot in which to store the item.
/// - H2: high 7 bits are used to SIMD optimize hash collision probing.
///
/// For more see <https://abseil.io/about/design/swisstables#metadata-layout>.
///
/// This hash function assumes that the entity ids are still well-distributed,
/// so for H1 leaves the entity id alone in the low bits so that id locality
/// will also give memory locality for things spawned together.
/// For H2, take advantage of the fact that while multiplication doesn't
/// spread entropy to the low bits, it's incredibly good at spreading it
/// upward, which is exactly where we need it the most.
///
/// While this does include the generation in the output, it doesn't do so
/// *usefully*.  H1 won't care until you have over 3 billion entities in
/// the table, and H2 won't care until something hits generation 33 million.
/// Thus the comment suggesting that this is best for live entities,
/// where there won't be generation conflicts where it would matter.
///
/// The high 32 bits of this are ⅟φ for Fibonacci hashing.  That works
/// particularly well for hashing for the same reason as described in
/// <https://extremelearning.com.au/unreasonable-effectiveness-of-quasirandom-sequences/>
/// It loses no information because it has a modular inverse.
/// (Specifically, `0x144c_bc89_u32 * 0x9e37_79b9_u32 == 1`.)
///
/// The low 32 bits make that part of the just product a pass-through.
const UPPER_PHI: u64 = 0x9e37_79b9_0000_0001;

impl Hasher for SparseHasher {
    #[inline(always)]
    fn finish(&self) -> u64 {
        self.hash
    }

    #[inline(always)]
    fn write_usize(&mut self, i: usize) {
        self.hash = UPPER_PHI.wrapping_mul(i as u64);
    }

    #[inline(always)]
    fn write_u64(&mut self, i: u64) {
        self.hash = UPPER_PHI.wrapping_mul(i);
    }

    #[inline(always)]
    fn write_u32(&mut self, i: u32) {
        self.hash = UPPER_PHI.wrapping_mul(i as u64);
    }

    #[inline(always)]
    fn write_u16(&mut self, i: u16) {
        self.hash = UPPER_PHI.wrapping_mul(i as u64);
    }

    #[inline(always)]
    fn write_u8(&mut self, i: u8) {
        self.hash = UPPER_PHI.wrapping_mul(i as u64);
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.hash <<= 2;
            self.hash |= UPPER_PHI.wrapping_mul(*byte as u64);
        }
    }
}

/// A very fast hash that is only designed to work on generational indices.
///
/// For example, `EntityId` in ECS module, it's uniformly distributed starting from 0.
#[derive(Copy, Clone, Default, Debug)]
pub struct SparseHashState;

impl BuildHasher for SparseHashState {
    type Hasher = SparseHasher;

    #[inline(always)]
    fn build_hasher(&self) -> Self::Hasher {
        SparseHasher { hash: 0 }
    }

    #[inline(always)]
    fn hash_one<T: core::hash::Hash>(&self, x: T) -> u64
    where
        Self: Sized,
        Self::Hasher: Hasher,
    {
        let mut hasher = const { SparseHasher { hash: 0 } };
        x.hash(&mut hasher);
        hasher.hash
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use core::any::TypeId;
    use core::hash::{Hash, Hasher};

    #[test]
    fn noop_typeid_hash() {
        struct TestNoopHasher(u64);
        impl Hasher for TestNoopHasher {
            fn finish(&self) -> u64 {
                self.0
            }
            fn write_u64(&mut self, i: u64) {
                self.0 = i;
            }
            fn write(&mut self, _bytes: &[u8]) {
                panic!()
            }
        }

        let id = TypeId::of::<u32>();
        let mut hasher = TestNoopHasher(0);
        id.hash(&mut hasher);
        core::hint::black_box(id);
    }
}
