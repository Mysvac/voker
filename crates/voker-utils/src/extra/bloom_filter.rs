use core::fmt::Debug;
use core::hash::{BuildHasher, Hash};

use crate::hash::FixedHashState;

/// A simple but efficient bloom-filter.
///
/// A Bloom filter can test whether an element is possibly in a set, or definitely not.
/// False positives are possible, but false negatives are not. The probability of
/// false positives can be controlled through proper sizing and parameter selection.
///
/// For detailed information, see <https://en.wikipedia.org/wiki/Bloom_filter>.
///
/// # Type Parameters
///
/// - `N`: The number of `u64` segments. The total bit capacity is `N * 64`.
///   **Must be a power of two** (e.g., 1, 2, 4, 8, …); otherwise, `new` will panic.
/// - `K`: The number of hash positions to check per element (default is 2).
///   This is implemented using a single hash computation with derived positions
///   rather than `K` independent hash functions.
///
/// # Trade-offs
///
/// - A larger `N` (more bits) reduces false positives but uses more memory.
/// - A larger `K` (more hash positions) reduces false positives but increases
///   computational cost. `K = 2` works well for most use cases.
///
/// # Examples
///
/// ```
/// use voker_utils::extra::BloomFilter;
///
/// let mut filter = BloomFilter::<1>::new();  // 64-bit filter
/// filter.insert(&"hello");
///
/// // "hello" is definitely in the set (or a false positive)
/// assert!(filter.contains(&"hello"));
///
/// // "world" is definitely NOT in the set (no false negatives)
/// assert!(!filter.contains(&"world"));
/// ```
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct BloomFilter<const N: usize, const K: usize = 2> {
    bits: [u64; N],
}

impl<const N: usize, const K: usize> BloomFilter<N, K> {
    /// Bitmask for mapping hash values to bit positions.
    const MASK: u64 = (N * 64 - 1) as u64;

    /// Creates a new, empty Bloom filter.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::extra::BloomFilter;
    /// let filter = BloomFilter::<2>::new(); // 128-bit filter
    /// assert!(!filter.contains(&"anything"));
    /// ```
    #[inline(always)]
    pub const fn new() -> Self {
        const {
            // Due to generic types, static assertions cannot be
            // placed in the initialization of static fields (useless).
            assert!(
                N.is_power_of_two(),
                "BloomFilter size N must be a power of two.",
            );
        }
        Self { bits: [0; N] }
    }

    /// Inserts an item into the filter.
    ///
    /// After insertion, subsequent calls to [`contains`] for this item will
    /// return `true` (barring extremely rare hash collisions).
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::extra::BloomFilter;
    /// let mut filter = BloomFilter::<2>::new();
    /// filter.insert(&42);
    /// assert!(filter.contains(&42));
    /// ```
    ///
    /// [`contains`]: Self::contains
    pub fn insert(&mut self, item: &impl Hash) {
        let h1 = FixedHashState.hash_one(item);
        let h2 = (h1 >> 32) | 1; // Ensure h2 is odd for better distribution

        for i in 0..K {
            let hash = h1.wrapping_add(h2.wrapping_mul(i as u64)) & Self::MASK;
            let index = (hash >> 6) as usize; // hash / 64
            let bit = 1_u64 << (hash & 63); // hash % 64

            self.bits[index] |= bit;
        }
    }

    /// Clears the Bloom filter, resetting all bits to 0.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::extra::BloomFilter;
    /// let mut filter = BloomFilter::<2>::new();
    ///
    /// filter.insert(&42);
    /// assert!(filter.contains(&42));
    ///
    /// filter.clear();
    /// assert!(!filter.contains(&42));
    /// ```
    pub fn clear(&mut self) {
        #[expect(unsafe_code, reason = "write_bytes is faster then for_each")]
        unsafe {
            core::ptr::write_bytes(self.bits.as_mut_ptr(), 0, N);
        }
    }

    /// Checks whether the item **might** be in the filter.
    ///
    /// Returns `false` if the item is **definitely not** in the filter.
    /// Returns `true` if the item **may be** in the filter (with a small
    /// probability of false positives).
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::extra::BloomFilter;
    /// let mut filter = BloomFilter::<2>::new();
    /// filter.insert(&1);
    ///
    /// assert!(filter.contains(&1));    // Definitely true (inserted)
    /// assert!(!filter.contains(&2));   // Definitely false (not inserted)
    /// // A false positive could occur for other values with low probability
    /// ```
    pub fn contains(&self, item: &impl Hash) -> bool {
        let h1 = FixedHashState.hash_one(item);
        let h2 = (h1 >> 32) | 1;

        for i in 0..K {
            let hash = h1.wrapping_add(h2.wrapping_mul(i as u64)) & Self::MASK;
            let index = (hash >> 6) as usize;
            let bit = 1_u64 << (hash & 63);

            if self.bits[index] & bit == 0 {
                return false;
            }
        }

        true
    }

    /// Atomically checks if an item is in the filter, and inserts it if not.
    ///
    /// This operation is equivalent to [`contains`] followed by [`insert`],
    /// but more efficient as it performs a single pass over the hash positions.
    ///
    /// # Returns
    ///
    /// - `true` if the item was **already present** in the filter (or a false positive)
    /// - `false` if the item was **definitely not present** and has now been inserted
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::extra::BloomFilter;
    /// let mut filter = BloomFilter::<2>::new();
    ///
    /// // First insertion returns false (item was not present)
    /// assert!(!filter.check_insert(&1));
    ///
    /// // Second check returns true (item is now present)
    /// assert!(filter.check_insert(&1));
    /// ```
    ///
    /// [`contains`]: Self::contains
    /// [`insert`]: Self::insert
    pub fn checked_insert(&mut self, item: &impl Hash) -> bool {
        let h1 = FixedHashState.hash_one(item);
        let h2 = (h1 >> 32) | 1;

        let mut was_present = true;

        for i in 0..K {
            let hash = h1.wrapping_add(h2.wrapping_mul(i as u64)) & Self::MASK;
            let index = (hash >> 6) as usize;
            let bit = 1_u64 << (hash & 63);

            let segment = &mut self.bits[index];

            if *segment & bit == 0 {
                was_present = false;
                *segment |= bit;
            }
        }

        was_present
    }
}

impl<const N: usize, const K: usize> Default for BloomFilter<N, K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize, const K: usize> Debug for BloomFilter<N, K> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "BloomFilter<{N}, {K}>([")?;
        for &bits in self.bits.iter() {
            // Use iter to avoid copying data
            write!(f, "{:016x}", bits)?;
        }
        f.write_str("])")
    }
}
