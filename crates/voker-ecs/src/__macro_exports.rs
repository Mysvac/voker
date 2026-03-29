//! Contents provided to proc macros.
//!
//! Users should not directly use any content here.

// -----------------------------------------------------------------------------
// Macro tools

/// An internal module provided for proc-macro implementation.
pub mod macro_utils {
    pub use ::alloc::boxed::Box;

    pub mod cloner {
        pub use crate::__macro_exports::clone_spec::*;
    }
}

// -----------------------------------------------------------------------------
// CloneSpec

mod clone_spec {
    use crate::voker_ecs::utils::Cloner;
    use core::marker::PhantomData;

    pub struct __CloneSpec<T>(PhantomData<T>);

    impl<T> __CloneSpec<T> {
        pub const INS: Self = Self(PhantomData);
    }

    pub trait __ClonerSpecialization {
        fn __specialized_cloner(&self) -> Option<Cloner>;
    }

    impl<T> __ClonerSpecialization for __CloneSpec<T> {
        fn __specialized_cloner(&self) -> Option<Cloner> {
            None
        }
    }

    impl<T: Clone> __ClonerSpecialization for &__CloneSpec<T> {
        fn __specialized_cloner(&self) -> Option<Cloner> {
            Some(Cloner::clonable::<T>())
        }
    }

    impl<T: Copy> __ClonerSpecialization for &&__CloneSpec<T> {
        fn __specialized_cloner(&self) -> Option<Cloner> {
            Some(Cloner::copyable::<T>())
        }
    }
}
