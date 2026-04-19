# Platform independent extensions

Platform-agnostic utility crate for collection primitives,
hashing helpers, index containers, and small no_std-friendly helpers.

## Crate Layout

### `hash`
- Hash builders and hash container aliases.
- Re-exports and wrappers around `hashbrown` / `foldhash`.
- Includes fixed-hash, sparse-hash, and no-op hash variants.

### `index`
- Index-preserving map/set utilities.
- Thin integration layer around `indexmap` with crate-aligned defaults.

### `vec`
- Vector implementations for different storage/performance strategies.
- Includes `ArrayVec`, `SmallVec`, and `FastVec`.

### `extra`
- Additional utility containers and data structures.
- Includes `ArrayDeque`, `BlockList`, `BloomFilter`, `PagePool`, and `TypeIdMap`.

### `num`
- Numeric helper types.
- Includes `NonMax` wrappers (niche-value optimization style helpers).

### Top-level helpers
- `cold_path`: Branch hint helper for cold paths.
- `range_invoke`: Macro utility for repeated range-based invocation.

## Notes

- The crate targets `no_std` + `alloc` usage patterns.
- For thread-safe and OS-dependent utilities, use `voker-os`.
