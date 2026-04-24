cfg_select! {
    all(feature = "file_watcher", not(target_arch = "wasm32"), not(target_os = "android")) => {
        mod notifier;

        mod file_watcher;
        mod embedded_watcher;

        pub use file_watcher::FileWatcher;
        pub use embedded_watcher::EmbeddedWatcher;
    }
}

pub trait AssetWatcher: Send + Sync + 'static {}
