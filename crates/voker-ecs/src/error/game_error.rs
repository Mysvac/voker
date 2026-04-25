use alloc::boxed::Box;
use core::error::Error;
use core::fmt::{Debug, Display};

// -----------------------------------------------------------------------------
// Severity

/// Indicates how severe a [`GameError`] is.
///
/// These levels correspond to traditional logging levels,
/// but the severity is advisory metadata used by error handlers
/// to decide how to react (for example: ignore, log, or panic).
///
/// To change the behavior of unhandled errors returned from systems,
/// you can replace the [fallback error handler], and read the [`Severity`]
/// stored inside of each [`GameError`].
///
/// You can override the severity of an existing [`GameError`] by calling
/// [`GameError::with_severity`].
///
/// [`with_severity`]: GameError::with_severity
/// [fallback error handler]: crate::error::FallbackErrorHandler
#[derive(Debug, Clone, Copy, Hash)]
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// The error can be safely ignored and completely discarded.
    Ignore = 0,
    /// The error can be ignored unless verbose debugging is required.
    Trace = 1,
    /// The error can be safely ignored but may need to be surfaced during debugging.
    Debug = 2,
    /// Nothing has gone wrong, but the error is useful to the user and should be reported.
    Info = 3,
    /// Something unexpected but recoverable happened. Something has probably gone wrong.
    Warning = 4,
    /// A real error occurred, but the program may continue.
    Error = 5,
    /// A fatal error; the program cannot continue.
    Panic = 6,
}

// -----------------------------------------------------------------------------
// GameError

struct InnerError {
    error: Box<dyn Error + Send + Sync + 'static>,
    severity: Severity,
    #[cfg(feature = "backtrace")]
    backtrace: std::backtrace::Backtrace,
}

/// A game-oriented error type that combines an underlying error with a severity level.
///
/// `GameError` wraps any `Error` type and attaches a [`Severity`] level, allowing
/// error handling systems to categorize and respond to errors appropriately.
///
/// # Examples
///
/// ```
/// # use voker_ecs::error::{GameError, Severity};
/// #
/// fn validate_value(val: i64) -> Result<(), GameError> {
///     if val < 0 {
///         return Err(GameError::new(
///             Severity::Panic,
///             format!("Value cannot be negative: {val}"),
///         ));
///     }
///     Ok(())
/// }
/// ```
pub struct GameError {
    inner: Box<InnerError>,
}

impl GameError {
    /// Creates a new [`GameError`] with the specified [`Severity`].
    ///
    /// The error is stored as a `Box<dyn Error + Send + Sync>`.
    ///
    /// Any type that can be converted into `Box<dyn Error + Send + Sync>` can be used,
    /// including string literals and `String`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::error::{GameError, Severity};
    /// #
    /// let err = GameError::new(Severity::Warning, "Configuration file not found");
    /// assert_eq!(err.severity(), Severity::Warning);
    /// ```
    #[cold]
    pub fn new<E>(severity: Severity, error: E) -> Self
    where
        Box<dyn Error + Sync + Send>: From<E>,
    {
        GameError {
            inner: Box::new(InnerError {
                severity,
                error: error.into(),
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            }),
        }
    }

    /// Creates a new [`GameError`] with [`Severity::Ignore`].
    ///
    /// This is a convenience shorthand for [`GameError::new(Severity::Ignore, error)`].
    pub fn ignore<E>(error: E) -> Self
    where
        Box<dyn Error + Send + Sync>: From<E>,
    {
        Self::new(Severity::Ignore, error)
    }

    /// Creates a new [`GameError`] with [`Severity::Trace`].
    ///
    /// This is a convenience shorthand for [`GameError::new(Severity::Trace, error)`].
    pub fn trace<E>(error: E) -> Self
    where
        Box<dyn Error + Send + Sync>: From<E>,
    {
        Self::new(Severity::Trace, error)
    }

    /// Creates a new [`GameError`] with [`Severity::Debug`].
    ///
    /// This is a convenience shorthand for [`GameError::new(Severity::Debug, error)`].
    pub fn debug<E>(error: E) -> Self
    where
        Box<dyn Error + Send + Sync>: From<E>,
    {
        Self::new(Severity::Debug, error)
    }

    /// Creates a new [`GameError`] with [`Severity::Info`].
    ///
    /// This is a convenience shorthand for [`GameError::new(Severity::Info, error)`].
    pub fn info<E>(error: E) -> Self
    where
        Box<dyn Error + Send + Sync>: From<E>,
    {
        Self::new(Severity::Info, error)
    }

    /// Creates a new [`GameError`] with [`Severity::Warning`].
    ///
    /// This is a convenience shorthand for [`GameError::new(Severity::Warning, error)`].
    pub fn warning<E>(error: E) -> Self
    where
        Box<dyn Error + Send + Sync>: From<E>,
    {
        Self::new(Severity::Warning, error)
    }

    /// Creates a new [`GameError`] with [`Severity::Error`].
    ///
    /// This is a convenience shorthand for [`GameError::new(Severity::Error, error)`].
    pub fn error<E>(error: E) -> Self
    where
        Box<dyn Error + Send + Sync>: From<E>,
    {
        Self::new(Severity::Error, error)
    }

    /// Creates a new [`GameError`] with [`Severity::Panic`].
    ///
    /// This is a convenience shorthand for [`GameError::new(Severity::Panic, error)`].
    pub fn panic<E>(error: E) -> Self
    where
        Box<dyn Error + Send + Sync>: From<E>,
    {
        Self::new(Severity::Panic, error)
    }

    /// Checks if the underlying error is of the specified type.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::error::GameError;
    /// #
    /// #[derive(Debug, thiserror::Error)]
    /// #[error("custom error")]
    /// struct CustomError;
    ///
    /// let err = GameError::panic(CustomError);
    /// assert!(err.is::<CustomError>());
    /// ```
    pub fn is<E: Error + 'static>(&self) -> bool {
        self.inner.error.is::<E>()
    }

    /// Attempts to downcast the underlying error to a reference of the specified type.
    ///
    /// Returns `Some(&E)` if the downcast succeeds, otherwise `None`.
    pub fn downcast_ref<E: Error + 'static>(&self) -> Option<&E> {
        self.inner.error.downcast_ref::<E>()
    }

    /// Attempts to downcast the underlying error to a mutable reference of the specified type.
    ///
    /// Returns `Some(&mut E)` if the downcast succeeds, otherwise `None`.
    pub fn downcast_mut<E: Error + 'static>(&mut self) -> Option<&mut E> {
        self.inner.error.downcast_mut::<E>()
    }

    /// Attempts to downcast the underlying error to a boxed value of the specified type.
    ///
    /// Returns `Ok(Box<E>)` if the downcast succeeds, otherwise returns `Err(self)` unchanged.
    pub fn downcast<E: Error + 'static>(self) -> Result<Box<E>, Self> {
        if self.inner.error.as_ref().is::<E>() {
            Ok(self.inner.error.downcast::<E>().unwrap())
        } else {
            Err(self)
        }
    }

    /// Returns the severity level of this error.
    #[inline]
    pub fn severity(&self) -> Severity {
        self.inner.severity
    }

    /// Overrides the severity level of this error.
    ///
    /// This only changes the metadata; the underlying error value remains unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::error::{GameError, Severity};
    /// #
    /// let err = GameError::panic("..").with_severity(Severity::Warning);
    /// assert_eq!(err.severity(), Severity::Warning);
    /// ```
    #[inline]
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.inner.severity = severity;
        self
    }

    /// Set the severity level to the maximum value of old level and given level.
    ///
    /// This only changes the metadata; the underlying error value remains unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::error::{GameError, Severity};
    /// #
    /// let err = GameError::info("..").with_severity(Severity::Warning);
    /// assert_eq!(err.severity(), Severity::Warning);
    /// ```
    #[inline]
    pub fn merge_severity(mut self, severity: Severity) -> Self {
        self.inner.severity = severity.max(self.inner.severity);
        self
    }

    /// Map severity through given function.
    #[inline]
    pub fn map_severity(mut self, f: impl FnOnce(Severity) -> Severity) -> Self {
        self.inner.severity = f(self.inner.severity);
        self
    }
}

impl Display for GameError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.inner.error, f)?;
        self.format_backtrace(f)
    }
}

impl Debug for GameError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.inner.error, f)?;
        self.format_backtrace(f)
    }
}

// -----------------------------------------------------------------------------
// IntoGameError

/// Converts a value into an optional [`GameError`].
///
/// This trait unifies different error-returning styles into one interface used
/// by ECS internals.
///
/// - `Some(GameError)` means an error is present.
/// - `None` means no error should be reported.
///
/// # Implementations
///
/// The crate provides these blanket implementations:
/// - `E` where `E: Into<GameError>` -> always `Some(...)`
/// - `()` -> always `None`
/// - `Option<T>` where `T: IntoGameError` -> propagates inner value
/// - `Result<T, E>` where `T: IntoGameError`, `E: IntoGameError`
///
/// This allows APIs to accept flexible return forms.
///
/// # Examples
///
/// ```
/// # use voker_ecs::error::{GameError, IntoGameError, Severity};
/// #
/// let e = GameError::warning("bad state");
/// assert!(e.to_err().is_some());
///
/// let ok = ().to_err();
/// assert!(ok.is_none());
///
/// let nested: Option<Result<(), GameError>> = Some(Err(GameError::panic("boom")));
/// assert_eq!(nested.to_err().unwrap().severity(), Severity::Panic);
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a GameError",
    label = "invalid GameError",
    note = "Consider annotating `{Self}` with `#[derive(GameError)]`."
)]
pub trait IntoGameError: Sized {
    /// Converts `self` into `Option<GameError>`.
    ///
    /// Return `None` to represent success/no error.
    fn to_err(self) -> Option<GameError>;

    /// Overrides the severity of the produced error, if any.
    ///
    /// If `self.to_err()` is `None`, this method also returns `None`.
    #[inline]
    fn with_severity(self, severity: Severity) -> Option<GameError> {
        self.to_err().map(|e| GameError::with_severity(e, severity))
    }

    /// Raises severity to `max(current, severity)` for the produced error, if any.
    #[inline]
    fn merge_severity(self, severity: Severity) -> Option<GameError> {
        self.to_err().map(|e| GameError::merge_severity(e, severity))
    }

    /// Maps the severity of the produced error through a function, if any.
    #[inline]
    fn map_severity(self, f: impl FnOnce(Severity) -> Severity) -> Option<GameError> {
        self.to_err().map(|e| GameError::map_severity(e, f))
    }
}

impl<E> IntoGameError for E
where
    E: Into<GameError>,
{
    fn to_err(self) -> Option<GameError> {
        Some(self.into())
    }
}

impl IntoGameError for () {
    #[inline(always)]
    fn to_err(self) -> Option<GameError> {
        None
    }

    #[inline(always)]
    fn with_severity(self, _: Severity) -> Option<GameError> {
        None
    }

    #[inline(always)]
    fn merge_severity(self, _: Severity) -> Option<GameError> {
        None
    }

    #[inline(always)]
    fn map_severity(self, _: impl FnOnce(Severity) -> Severity) -> Option<GameError> {
        None
    }
}

impl<T: IntoGameError> IntoGameError for Option<T> {
    #[inline]
    fn to_err(self) -> Option<GameError> {
        self.and_then(IntoGameError::to_err)
    }
}

impl<T: IntoGameError, E: IntoGameError> IntoGameError for Result<T, E> {
    #[inline]
    fn to_err(self) -> Option<GameError> {
        match self {
            Ok(x) => x.to_err(),
            Err(y) => y.to_err(),
        }
    }
}

// -----------------------------------------------------------------------------
// IntoGameError

#[cfg(feature = "backtrace")]
const FILTER_MESSAGE: &str = "note: Some \"noisy\" backtrace lines have been filtered out. Run with `VOKER_BACKTRACE=full` for a verbose backtrace.";

#[cfg(feature = "backtrace")]
std::thread_local! {
    static SKIP_NORMAL_BACKTRACE: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

/// When called, this will skip the currently configured panic hook when a
/// [`GameError`] backtrace has already been printed.
#[cfg(feature = "backtrace")]
#[expect(clippy::print_stdout, reason = "Allowed behind `std` feature gate.")]
pub fn game_error_panic_hook(
    current_hook: impl Fn(&std::panic::PanicHookInfo),
) -> impl Fn(&std::panic::PanicHookInfo) {
    move |info| {
        if SKIP_NORMAL_BACKTRACE.replace(false) {
            if let Some(payload) = info.payload().downcast_ref::<&str>() {
                std::println!("{payload}");
            } else if let Some(payload) = info.payload().downcast_ref::<alloc::string::String>() {
                std::println!("{payload}");
            }
            return;
        }

        current_hook(info);
    }
}

impl GameError {
    #[inline(always)]
    #[cfg(not(feature = "backtrace"))]
    fn format_backtrace(&self, _: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Ok(())
    }

    #[cfg(feature = "backtrace")]
    fn format_backtrace(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let backtrace = &self.inner.backtrace;
        if let std::backtrace::BacktraceStatus::Captured = backtrace.status() {
            let full_backtrace = std::env::var("VOKER_BACKTRACE").is_ok_and(|val| val == "full");

            let backtrace_str = alloc::string::ToString::to_string(backtrace);
            let mut skip_next_location_line = false;
            for line in backtrace_str.split('\n') {
                if !full_backtrace {
                    if skip_next_location_line {
                        if line.starts_with("             at") {
                            continue;
                        }
                        skip_next_location_line = false;
                    }
                    if line.contains("std::backtrace_rs::backtrace::") {
                        skip_next_location_line = true;
                        continue;
                    }
                    if line.contains("std::backtrace::Backtrace::") {
                        skip_next_location_line = true;
                        continue;
                    }
                    if line.contains(
                        "<voker_ecs::error::game_error::GameError as core::convert::From<E>>::from",
                    ) {
                        skip_next_location_line = true;
                        continue;
                    }
                    if line.contains("<core::result::Result<T,F> as core::ops::try_trait::FromResidual<core::result::Result<core::convert::Infallible,E>>>::from_residual") {
                        skip_next_location_line = true;
                        continue;
                    }
                    if line.contains("__rust_begin_short_backtrace") {
                        break;
                    }
                }
                writeln!(f, "{line}")?;
            }
            if !full_backtrace {
                if std::thread::panicking() {
                    SKIP_NORMAL_BACKTRACE.set(true);
                }
                writeln!(f, "{FILTER_MESSAGE}")?;
            }
        }

        Ok(())
    }
}
