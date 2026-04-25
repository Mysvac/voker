use core::fmt::Debug;

use voker_ecs::derive::Message;

use crate::asset::Asset;
use crate::ident::{AssetId, ErasedAssetId};
use crate::path::AssetPath;
use crate::server::AssetLoadError;

// -----------------------------------------------------------------------------
// AssetEvent

#[derive(Message)]
pub enum AssetEvent<A: Asset> {
    /// Emitted whenever an [`Asset`] is added.
    Added { id: AssetId<A> },
    /// Emitted whenever an [`Asset`] value is modified.
    Modified { id: AssetId<A> },
    /// Emitted whenever an [`Asset`] is removed.
    Removed { id: AssetId<A> },
    /// Emitted when the last [`Handle::Strong`](`super::Handle::Strong`) of an [`Asset`] is dropped.
    Unused { id: AssetId<A> },
    /// Emitted whenever an [`Asset`] has been fully loaded (including its dependencies and all "recursive dependencies").
    FullyLoaded { id: AssetId<A> },
}

impl<A: Asset> AssetEvent<A> {
    /// Returns `true` if this event is [`AssetEvent::FullyLoaded`] and matches the given `id`.
    pub fn is_fully_loaded(&self, asset_id: impl Into<AssetId<A>>) -> bool {
        matches!(self, AssetEvent::FullyLoaded { id } if *id == asset_id.into())
    }

    /// Returns `true` if this event is [`AssetEvent::Added`] and matches the given `id`.
    pub fn is_added(&self, asset_id: impl Into<AssetId<A>>) -> bool {
        matches!(self, AssetEvent::Added { id } if *id == asset_id.into())
    }

    /// Returns `true` if this event is [`AssetEvent::Modified`] and matches the given `id`.
    pub fn is_modified(&self, asset_id: impl Into<AssetId<A>>) -> bool {
        matches!(self, AssetEvent::Modified { id } if *id == asset_id.into())
    }

    /// Returns `true` if this event is [`AssetEvent::Removed`] and matches the given `id`.
    pub fn is_removed(&self, asset_id: impl Into<AssetId<A>>) -> bool {
        matches!(self, AssetEvent::Removed { id } if *id == asset_id.into())
    }

    /// Returns `true` if this event is [`AssetEvent::Unused`] and matches the given `id`.
    pub fn is_unused(&self, asset_id: impl Into<AssetId<A>>) -> bool {
        matches!(self, AssetEvent::Unused { id } if *id == asset_id.into())
    }
}

impl<A: Asset> Clone for AssetEvent<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Asset> Copy for AssetEvent<A> {}

impl<A: Asset> Debug for AssetEvent<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Added { id } => f.debug_struct("Added").field("id", id).finish(),
            Self::Modified { id } => f.debug_struct("Modified").field("id", id).finish(),
            Self::Removed { id } => f.debug_struct("Removed").field("id", id).finish(),
            Self::Unused { id } => f.debug_struct("Unused").field("id", id).finish(),
            Self::FullyLoaded { id } => f.debug_struct("FullyLoaded").field("id", id).finish(),
        }
    }
}

impl<A: Asset> PartialEq for AssetEvent<A> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Added { id: l_id }, Self::Added { id: r_id })
            | (Self::Modified { id: l_id }, Self::Modified { id: r_id })
            | (Self::Removed { id: l_id }, Self::Removed { id: r_id })
            | (Self::Unused { id: l_id }, Self::Unused { id: r_id })
            | (Self::FullyLoaded { id: l_id }, Self::FullyLoaded { id: r_id }) => l_id == r_id,
            _ => false,
        }
    }
}

impl<A: Asset> Eq for AssetEvent<A> {}

// -----------------------------------------------------------------------------
// AssetLoadFailedEvent

#[derive(Message, Debug)]
pub struct AssetLoadFailedEvent<A: Asset> {
    /// The stable identifier of the asset that failed to load.
    pub id: AssetId<A>,
    /// The asset path that was attempted.
    pub path: AssetPath<'static>,
    /// Why the asset failed to load.
    pub error: AssetLoadError,
}

impl<A: Asset> AssetLoadFailedEvent<A> {
    /// Converts this to an "untyped" / "generic-less" asset error event that stores the type information.
    pub fn untyped(&self) -> ErasedAssetLoadFailedEvent {
        self.into()
    }
}

impl<A: Asset> Clone for AssetLoadFailedEvent<A> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            path: self.path.clone(),
            error: self.error.clone(),
        }
    }
}

// -----------------------------------------------------------------------------
// ErasedAssetLoadFailedEvent

/// An untyped version of [`AssetLoadFailedEvent`].
#[derive(Message, Clone, Debug)]
pub struct ErasedAssetLoadFailedEvent {
    /// The stable identifier of the asset that failed to load.
    pub id: ErasedAssetId,
    /// The asset path that was attempted.
    pub path: AssetPath<'static>,
    /// Why the asset failed to load.
    pub error: AssetLoadError,
}

impl<A: Asset> From<&AssetLoadFailedEvent<A>> for ErasedAssetLoadFailedEvent {
    fn from(value: &AssetLoadFailedEvent<A>) -> Self {
        ErasedAssetLoadFailedEvent {
            id: value.id.erased(),
            path: value.path.clone(),
            error: value.error.clone(),
        }
    }
}
