use core::fmt::Display;
use serde_core::ser::Error;

crate::cfg::debug! {
    std::thread_local! {
        pub(super) static TYPE_INFO_STACK: core::cell::RefCell<crate::serde::TypeInfoStack> =
            const { core::cell::RefCell::new(crate::serde::TypeInfoStack::new()) };
    }
}

/// A helper function for generating a custom deserialization error message.
///
/// This function should be preferred over [`Error::custom`] as it will include
/// other useful information, such as the [type info stack].
///
/// [type info stack]: crate::type_info_stack::TypeInfoStack
#[cold]
pub(super) fn make_custom_error<E: Error>(msg: impl Display) -> E {
    crate::cfg::debug! {
        if {
            TYPE_INFO_STACK.with_borrow(|stack|
                E::custom(format_args!("{msg} (stack:\n{stack:?}\n)"))
            )
        } else {
            E::custom(msg)
        }
    }
}
