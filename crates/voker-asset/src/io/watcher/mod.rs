cfg_select! {
    target_arch = "wasm32" => { /* not support */ }
    target_os = "android" => { /* not support */ }
    feature = "file_watcher" => {
        mod notifier;

        mod file_watcher;
        mod embedded_watcher;

        pub use file_watcher::FileWatcher;
        pub use embedded_watcher::EmbeddedWatcher;
    }
    _ => {}
}

pub trait AssetWatcher: Send + Sync + 'static {}
