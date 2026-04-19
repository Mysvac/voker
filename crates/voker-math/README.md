# Mathematical tools

> From [bevy_math](https://docs.rs/bevy/latest/bevy/math).

`voker-math` provides math types and geometry utilities:

- linear algebra re-exports (`Vec*`, `Mat*`, `Quat`, swizzles, constructors),
- 2D/3D geometry primitives and bounding volumes,
- transforms and orientation helpers (`Affine*`, `Isometry*`, `Rot2`),
- curve/spline tooling for continuous parametrized values,
- optional random shape sampling utilities.

## Feature Flags

- `std` (default): Enables standard-library integrations.
- `rand` (default): Enables random sampling support (`sampling` module).
- `approx`: Approximate equality integration for float-heavy tests/assertions.
- `mint`: Interop with `mint`-compatible libraries.
- `libm`: Forces libm math paths for deterministic behavior across platforms.
- `nostd-libm`: `no_std` fallback math implementation via `libm`.
- `glam_assert`: Always-on `glam` argument validity checks.
- `debug_glam_assert`: Debug-only `glam` argument validity checks.
