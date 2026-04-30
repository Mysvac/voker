use alloc::boxed::Box;
use std::path::{Path, PathBuf};

use voker_app::{App, Plugin};

use super::{AssetReader, AssetReaderError, Reader};
use crate::PathStream;

// -----------------------------------------------------------------------------
// WebAssetPlugin

#[derive(Default)]
pub struct WebAssetPlugin {
    pub silence_startup_warning: bool,
}

impl Plugin for WebAssetPlugin {
    #[cfg(any(feature = "http", feature = "https"))]
    fn build(&self, app: &mut App) {
        use crate::io::AssetSourceBuilder;
        use crate::plugin::{AppAssetExt, AssetPlugin};

        if !self.silence_startup_warning {
            tracing::warn!(
                "WebAssetPlugin is potentially insecure! Make sure to verify asset URLs are \
                safe to load before loading them. If you promise you know what you're doing, \
                you can silence this warning by setting silence_startup_warning: true in the \
                WebAssetPlugin construction."
            );
        }

        if app.is_plugin_added::<AssetPlugin>() {
            tracing::error!("WebAssetPlugin must be added before AssetPlugin for it to work!");
        }

        #[cfg(feature = "http")]
        app.register_asset_source(
            "http",
            AssetSourceBuilder::new(move || Box::new(WebAssetReader::Http))
                .with_processed_reader(move || Box::new(WebAssetReader::Http)),
        );

        #[cfg(feature = "https")]
        app.register_asset_source(
            "https",
            AssetSourceBuilder::new(move || Box::new(WebAssetReader::Https))
                .with_processed_reader(move || Box::new(WebAssetReader::Https)),
        );
    }

    #[cfg(not(any(feature = "http", feature = "https")))]
    fn build(&self, app: &mut App) {
        if app.is_plugin_added::<AssetPlugin>() {
            tracing::warn!("WebAssetPlugin must be added before AssetPlugin for it to work!");
        }

        tracing::warn!(
            "`http` and `https` cargo features are not enabled, WebAssetPlugin is NO-OP."
        );
    }
}

// -----------------------------------------------------------------------------
// WebAssetReader

pub enum WebAssetReader {
    /// Unencrypted connections.
    Http,
    /// Use TLS for setting up connections.
    Https,
}

impl WebAssetReader {
    /// - Http  -> "http://"
    /// - Https -> "https://"
    fn make_uri(&self, path: &Path) -> PathBuf {
        let prefix = match self {
            Self::Http => "http://",
            Self::Https => "https://",
        };
        PathBuf::from(prefix).join(path)
    }

    /// As same as [`crate::utils::append_meta_extension`].
    fn make_meta_uri(&self, path: &Path) -> PathBuf {
        self.make_uri(&crate::utils::append_meta_extension(path))
    }
}

impl AssetReader for WebAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader, AssetReaderError> {
        get(self.make_uri(path)).await
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader, AssetReaderError> {
        let uri = self.make_meta_uri(path);
        get(uri).await
    }

    async fn is_directory<'a>(&'a self, _path: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(false)
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        Err(AssetReaderError::NotFound(self.make_uri(path)))
    }
}

// -----------------------------------------------------------------------------
// WebAssetReader

#[cfg(target_arch = "wasm32")]
async fn get<'a>(path: PathBuf) -> Result<Box<dyn Reader>, AssetReaderError> {
    use crate::io::wasm::HttpWasmAssetReader;

    HttpWasmAssetReader::new("")
        .fetch_bytes(path)
        .await
        .map(|r| Box::new(r) as Box<dyn Reader>)
}

#[cfg(not(target_arch = "wasm32"))]
async fn get(path: PathBuf) -> Result<Box<dyn Reader>, AssetReaderError> {
    use alloc::{borrow::ToOwned, boxed::Box, vec::Vec};
    use std::io::{self, BufReader, Read};

    use blocking::unblock;
    use ureq::Agent;
    use ureq::tls::{RootCerts, TlsConfig};
    use voker_os::sync::LazyLock;

    use crate::io::VecReader;

    let str_path = path.to_str().ok_or_else(|| {
        AssetReaderError::Io(io::Error::other(alloc::format!(
            "non-utf8 path: {}",
            path.display()
        )))
    })?;

    #[cfg(windows)]
    let str_path = &str_path.replace(std::path::MAIN_SEPARATOR, "/");

    static AGENT: LazyLock<Agent> = LazyLock::new(|| {
        Agent::config_builder()
            .tls_config(TlsConfig::builder().root_certs(RootCerts::PlatformVerifier).build())
            .build()
            .new_agent()
    });

    let uri = str_path.to_owned();
    // Use [`unblock`] to run the http request on a separately spawned thread
    // as to not block voker's async executor.
    let response = unblock(|| AGENT.get(uri).call()).await;

    match response {
        Ok(mut response) => {
            let mut reader = BufReader::new(response.body_mut().with_config().reader());

            let mut buffer = Vec::new();
            reader.read_to_end(&mut buffer)?;

            Ok(Box::new(VecReader::new(buffer)))
        }
        // ureq considers all >=400 status codes as errors
        Err(ureq::Error::StatusCode(code)) => {
            if code == 404 {
                Err(AssetReaderError::NotFound(path))
            } else {
                Err(AssetReaderError::HttpError(code))
            }
        }
        Err(err) => Err(AssetReaderError::Io(io::Error::other(alloc::format!(
            "unexpected error while loading asset {}: {err}",
            path.display(),
        )))),
    }
}
