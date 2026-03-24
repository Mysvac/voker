# voker-ptr

Lightweight pointer wrappers for internal runtime code.

This crate provides small, type-erased pointer utilities used by ECS/reflection internals to reduce data movement and keep APIs explicit about safety boundaries.

## What This Crate Provides

- `Ptr<'a>`: type-erased shared pointer, conceptually similar to `&'a T`.
- `PtrMut<'a>`: type-erased exclusive pointer, conceptually similar to `&'a mut T`.
- `OwningPtr<'a>`: type-erased ownership pointer for read/drop handoff patterns.
- `ThinSlice<'a, T>`: thin shared slice pointer (stores pointer only, no length).
- `ThinSliceMut<'a, T>`: thin mutable slice pointer (stores pointer only, no length).

## Safety Model

These types are intentionally low-level. The compiler enforces lifetime shape through `PhantomData`, but callers still own key runtime responsibilities:

1. Use the correct target type when casting from erased pointers.
2. Ensure pointer alignment for the target type.
3. Ensure pointee validity and initialization state.
4. Respect aliasing/exclusivity rules for mutable access.

In debug builds, prefer calling alignment checks before unsafe casts.

## Minimal Usage

Shared erased pointer:

```rust
use voker_ptr::Ptr;

let x = 10_i32;
let ptr = Ptr::from_ref(&x);
ptr.debug_assert_aligned::<i32>();
let rx = unsafe { ptr.as_ref::<i32>() };
assert_eq!(*rx, 10);
```

Owning handoff:

```rust
use core::mem::ManuallyDrop;
use voker_ptr::OwningPtr;

let mut value = ManuallyDrop::new(42_i32);
let ptr = OwningPtr::from_value(&mut value);
let out = unsafe { ptr.read::<i32>() };
assert_eq!(out, 42);
```

Thin mutable slice:

```rust
use voker_ptr::ThinSliceMut;

let mut data = [1, 2, 3];
let mut thin = ThinSliceMut::from_mut(&mut data);
unsafe {
    *thin.get_mut(1) = 20;
    assert_eq!(thin.as_slice(3), &[1, 20, 3]);
}
```
