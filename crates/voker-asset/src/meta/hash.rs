use serde::de::Visitor;
use serde::{Deserialize, Serialize};

// -----------------------------------------------------------------------------
// AssetHash

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetHash(pub [u8; 32]);

// -----------------------------------------------------------------------------
// Serialize & Deserialize

impl Serialize for AssetHash {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'a> Deserialize<'a> for AssetHash {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        struct AssetHashVisitor;

        impl<'de> Visitor<'de> for AssetHashVisitor {
            type Value = AssetHash;

            fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                formatter.write_str("a byte array of length 32")
            }

            #[inline]
            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if let Some(val) = v.as_array::<32>() {
                    Ok(AssetHash(*val))
                } else {
                    // `invalid_length` already marked `cold`
                    Err(serde::de::Error::invalid_length(v.len(), &self))
                }
            }
        }

        deserializer.deserialize_bytes(AssetHashVisitor)
    }
}

// -----------------------------------------------------------------------------
// Normal

impl AssetHash {
    pub const ZERO: AssetHash = AssetHash([0; 32]);
}

impl Default for AssetHash {
    #[inline(always)]
    fn default() -> Self {
        AssetHash::ZERO
    }
}

impl From<[u8; 32]> for AssetHash {
    #[inline(always)]
    fn from(value: [u8; 32]) -> Self {
        Self(value)
    }
}

impl From<AssetHash> for [u8; 32] {
    #[inline(always)]
    fn from(value: AssetHash) -> Self {
        value.0
    }
}
