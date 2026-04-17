#![expect(clippy::print_stderr, reason = "Allowed during logger setup")]

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;

use tracing_log::LogTracer;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, Layer};
use tracing_subscriber::{layer::Layered, registry::Registry};
use voker_app::{App, Plugin};

// -----------------------------------------------------------------------------
// Filter

/// Default [`EnvFilter`] directives used by [`LogPlugin`].
///
/// These reduce noise from common dependencies while keeping engine-level logs visible.
pub const DEFAULT_FILTER: &str = concat!(
    "wgpu=error,",
    "naga=warn,",
    "symphonia_bundle_mp3::demuxer=warn,",
    "symphonia_format_caf::demuxer=warn,",
    "symphonia_format_isompf4::demuxer=warn,",
    "symphonia_format_mkv::demuxer=warn,",
    "symphonia_format_ogg::demuxer=warn,",
    "symphonia_format_riff::demuxer=warn,",
    "symphonia_format_wav::demuxer=warn,",
    "calloop::loop_logic=error,",
    "calloop::sources=debug,",
);

// -----------------------------------------------------------------------------
// Alias

#[cfg(feature = "trace")]
type BaseSubscriber =
    Layered<EnvFilter, Layered<Option<Box<dyn Layer<Registry> + Send + Sync>>, Registry>>;

#[cfg(feature = "trace")]
type PreFmtSubscriber = Layered<tracing_error::ErrorLayer<BaseSubscriber>, BaseSubscriber>;

#[cfg(not(feature = "trace"))]
type PreFmtSubscriber =
    Layered<EnvFilter, Layered<Option<Box<dyn Layer<Registry> + Send + Sync>>, Registry>>;

/// A boxed layer that can be added through [`LogPlugin::custom_layer`].
pub type BoxedLayer = Box<dyn Layer<Registry> + Send + Sync + 'static>;
/// A boxed formatting layer that can replace the default formatter via [`LogPlugin::fmt_layer`].
pub type BoxedFmtLayer = Box<dyn Layer<PreFmtSubscriber> + Send + Sync + 'static>;

// -----------------------------------------------------------------------------
// Memory

/// Configures global logging for a voker application.
///
/// This plugin installs a process-wide tracing subscriber and a `log` bridge.
/// It should normally be added only once per process.
pub struct LogPlugin {
    /// Additional filter directives in [`EnvFilter`] syntax.
    ///
    /// The final filter is composed from `level`, `filter`, and `RUST_LOG`.
    pub filter: String,
    /// Base level used when composing default filter directives.
    pub level: tracing::Level,
    /// Optional extra layer attached before filtering and formatting.
    pub custom_layer: fn(app: &mut App) -> Option<BoxedLayer>,
    /// Optional replacement for the default `fmt` layer.
    pub fmt_layer: fn(app: &mut App) -> Option<BoxedFmtLayer>,
}

impl Default for LogPlugin {
    fn default() -> Self {
        Self {
            filter: String::from(DEFAULT_FILTER),
            level: tracing::Level::INFO,
            custom_layer: |_| None,
            fmt_layer: |_| None,
        }
    }
}

impl Plugin for LogPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "trace")]
        {
            let old_handler = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |infos| {
                std::eprintln!("{}", tracing_error::SpanTrace::capture());
                old_handler(infos);
            }));
        }

        let finished_subscriber;
        let subscriber = Registry::default();

        let subscriber = subscriber.with((self.custom_layer)(app));
        let subscriber = subscriber.with(self.build_filter_layer());

        #[cfg(feature = "trace")]
        let subscriber = subscriber.with(tracing_error::ErrorLayer::default());

        #[cfg(not(target_os = "ios"))]
        {
            let fmt_layer = (self.fmt_layer)(app).unwrap_or_else(|| {
                Box::new(tracing_subscriber::fmt::Layer::default().with_writer(std::io::stderr))
            });

            let subscriber = subscriber.with(fmt_layer);

            #[cfg(target_os = "android")]
            let subscriber = subscriber.with(crate::android::AndroidLayer::default());

            finished_subscriber = subscriber;
        }

        #[cfg(target_os = "ios")]
        {
            finished_subscriber = subscriber.with(tracing_oslog::OsLogger::default());
        }

        let logger_already_set = LogTracer::init().is_err();
        let subscriber_already_set =
            tracing::subscriber::set_global_default(finished_subscriber).is_err();

        match (logger_already_set, subscriber_already_set) {
            (true, true) => tracing::error!(
                "Could not set global logger and tracing subscriber as they are already set. Consider disabling LogPlugin."
            ),
            (true, false) => tracing::error!(
                "Could not set global logger as it is already set. Consider disabling LogPlugin."
            ),
            (false, true) => tracing::error!(
                "Could not set global tracing subscriber as it is already set. Consider disabling LogPlugin."
            ),
            (false, false) => (),
        }
    }
}

impl LogPlugin {
    /// Builds the effective [`EnvFilter`] by combining defaults and `RUST_LOG` directives.
    ///
    /// If parsing `RUST_LOG` fails, this falls back to default directives and writes
    /// the parse error to stderr.
    fn build_filter_layer(&self) -> EnvFilter {
        let default_filters =
            EnvFilter::builder().parse_lossy(format!("{},{}", self.level, self.filter));
        let env_filters = std::env::var(EnvFilter::DEFAULT_ENV).unwrap_or_default();

        let result = env_filters
            .split(',')
            .filter(|s| !s.is_empty())
            .try_fold(default_filters.clone(), |filters, directive| {
                directive.parse().map(|d| filters.add_directive(d))
            });

        match result {
            Ok(combined_filters) => combined_filters,
            Err(e) => {
                std::eprintln!("LogPlugin failed to parse filter from env: {e}");
                default_filters
            }
        }
    }
}
