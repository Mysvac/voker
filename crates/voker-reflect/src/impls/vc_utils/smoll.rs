use alloc::string::String;
use voker_utils::smol::SmolStr;

crate::derive::impl_reflect_opaque! {
    (in smol_str)
    SmolStr(
        Clone,
        Debug,
        Hash,
        PartialEq,
        PartialOrd,
        Default,
        Serialize,
        Deserialize,
        From<&'static str>,
        From<String>,
        Into<String>,
    )
}
