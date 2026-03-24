// -----------------------------------------------------------------------------
// Modules

mod cloner;
mod debug_name;
mod debug_unwrap;
mod dropper;

// -----------------------------------------------------------------------------
// Exports

pub use cloner::Cloner;
pub use debug_name::DebugName;
pub use debug_unwrap::DebugCheckedUnwrap;
pub use dropper::Dropper;

// -----------------------------------------------------------------------------
// Inline

use crate::cfg;

pub(crate) fn thread_hash() -> u64 {
    cfg::std! {
        if {
            use core::hash::BuildHasher;
            let state = ::voker_utils::hash::FixedHashState;
            let thread_id = std::thread::current().id();
            state.hash_one(thread_id)
        } else{
            0
        }
    }
}
