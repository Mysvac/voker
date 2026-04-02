macro_rules! taskpool {
    ($(#[$attr:meta])* ($static:ident, $type:ident)) => {
        static $static: ::voker_os::sync::OnceLock<$type> = ::voker_os::sync::OnceLock::new();

        $(#[$attr])*
        #[derive(Debug)]
        pub struct $type(TaskPool);

        impl $type {
            #[doc = concat!(
                " Gets the global [`", stringify!($type), "`] instance,",
                " or initializes it with `f`.",
            )]
            pub fn get_or_init(f: impl FnOnce() -> TaskPool) -> &'static Self {
                $static.get_or_init(|| Self(f()))
            }

            #[doc = concat!(
                " Attempts to get the global [`", stringify!($type), "`] instance, ",
                "or returns `None` if it is not initialized.",
            )]
            pub fn try_get() -> Option<&'static Self> {
                $static.get()
            }

            #[doc = concat!(" Gets the global [`", stringify!($type), "`] instance.")]
            #[doc = ""]
            #[doc = " # Panics"]
            #[doc = " Panics if the global instance has not been initialized yet."]
            pub fn get() -> &'static Self {
                $static.get().expect(
                    concat!(
                        "The ",
                        stringify!($type),
                        " has not been initialized yet. Please call ",
                        stringify!($type),
                        "::get_or_init beforehand."
                    )
                )
            }
        }

        impl ::core::ops::Deref for $type {
            type Target = TaskPool;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}

pub(crate) use taskpool;
