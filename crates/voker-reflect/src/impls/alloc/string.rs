use alloc::borrow::Cow;

crate::derive::impl_reflect_opaque! {
    ::alloc::string::String(
        Default,
        Debug,
        Clone,
        Hash,
        PartialEq,
        PartialOrd,
        Serialize,
        Deserialize,
        Into<Cow<'static, str>>,
        From<Cow<'static, str>>,
        From<&'static str>,
    )
}
