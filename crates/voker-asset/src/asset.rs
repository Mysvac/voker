use alloc::vec::Vec;

use voker_reflect::info::TypePath;
use voker_utils::hash::HashSet;

use crate::UntypedAssetId;

// -----------------------------------------------------------------------------
// Asset

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not an `Asset`",
    label = "invalid `Asset`",
    note = "consider annotating `{Self}` with `#[derive(Asset)]`"
)]
pub trait Asset: VisitAssetDependencies + TypePath + Send + Sync + 'static {}

// -----------------------------------------------------------------------------
// VisitAssetDependencies

pub trait VisitAssetDependencies {
    fn visit_dependencies(&self, visit: &mut dyn FnMut(UntypedAssetId));
}

impl VisitAssetDependencies for UntypedAssetId {
    fn visit_dependencies(&self, visit: &mut dyn FnMut(UntypedAssetId)) {
        visit(*self);
    }
}

impl<V: VisitAssetDependencies> VisitAssetDependencies for Vec<V> {
    fn visit_dependencies(&self, visit: &mut dyn FnMut(UntypedAssetId)) {
        for dependency in self {
            dependency.visit_dependencies(visit);
        }
    }
}

impl<V: VisitAssetDependencies> VisitAssetDependencies for HashSet<V> {
    fn visit_dependencies(&self, visit: &mut dyn FnMut(UntypedAssetId)) {
        for dependency in self {
            dependency.visit_dependencies(visit);
        }
    }
}
