use alloc::boxed::Box;
use core::error::Error;
use core::fmt::{Debug, Display};
use thiserror::Error;

// -----------------------------------------------------------------------------
// Severity

/// Indicates how severe a [`GameError`] is.
///
/// These levels correspond to traditional logging levels,
/// but the severity is advisory metadata used by error handlers
/// to decide how to react (for example: ignore, log, or panic).
///
/// To change the behavior of unhandled errors returned from systems,
/// you can modify the [fallback error handler], and read the [`Severity`]
/// stored inside of each [`GameError`].
///
/// You can change the severity of an error (including assigning an error severity)
/// to an ordinary result by calling [`GameError::with_severity`].
///
/// [`with_severity`]: ResultSeverityExt::with_severity
/// [fallback error handler]: crate::error::handler::FallbackErrorHandler
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
    pub fn new<E>(severity: Severity, error: E) -> Self
    where
        Box<dyn Error + Sync + Send>: From<E>,
    {
        Self::from(error).with_severity(severity)
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
    pub fn downcast<E: Error + 'static>(mut self) -> Result<Box<E>, Self> {
        #[derive(Debug, Error)]
        #[error("PLACEHOLDER")]
        struct Placeholder;

        let error = core::mem::replace(&mut self.inner.error, Box::new(Placeholder));

        match error.downcast::<E>() {
            Ok(e) => Ok(e),
            Err(e) => {
                self.inner.error = e;
                Err(self)
            }
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
    /// let err = GameError::panic("fatal").with_severity(Severity::Warning);
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
    /// let err = GameError::info("fatal").with_severity(Severity::Warning);
    /// assert_eq!(err.severity(), Severity::Warning);
    /// ```
    #[inline]
    pub fn map_severity(mut self, f: impl FnOnce(Severity) -> Severity) -> Self {
        self.inner.severity = f(self.inner.severity);
        self
    }
}

impl<E> From<E> for GameError
where
    Box<dyn Error + Send + Sync + 'static>: From<E>,
{
    #[cold]
    fn from(error: E) -> Self {
        GameError {
            inner: Box::new(InnerError {
                error: error.into(),
                severity: Severity::Panic,
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            }),
        }
    }
}

impl Display for GameError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.inner.error, f)?;
        #[cfg(feature = "backtrace")]
        Display::fmt(&self.inner.backtrace, f)?;
        Ok(())
    }
}

impl Debug for GameError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.inner.error, f)?;
        #[cfg(feature = "backtrace")]
        Debug::fmt(&self.inner.backtrace, f)?;
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// ToGameError

/// A trait for types that can be converted into an optional [`GameError`].
///
/// **Important**: `ToGameError` is not equivalent to `Into<GameError>`.
///
/// This trait provides default implementations that enable elegant error handling patterns:
///
/// # Type Conversions
///
/// | Type | Conversion Result |
/// |------|-------------------|
/// | `()` | `None` (no error) |
/// | `GameError` | `Some(error)` |
/// | `Option<T>` where `T: ToGameError` | `None` if `None`, otherwise `T::to_err()` |
/// | `Result<T, E>` where `T: ToGameError`, `E: Into<GameError>` | `T::to_err()` if `Ok`, `Some(error.into())` if `Err` |
///
/// # Usage Patterns
///
/// ```rust
/// # use voker_ecs::error::{GameError, Severity, ToGameError};
/// #
/// // No error
/// let result: Option<GameError> = ().to_err();
/// assert!(result.is_none());
///
/// // Direct error
/// let err = GameError::warning("something wrong");
/// assert!(err.to_err().is_some());
///
/// // Result with unit Ok type
/// let result: Result<(), &str> = Err("failed");
/// assert!(result.to_err().is_some());
///
/// // Nesting
/// let result: Result<Option<&str>, &str> = Ok(Some("failed"));
/// assert!(result.to_err().is_some());
///
/// // Simply with severity
/// let _: Option<GameError> = Err("failed").with_severity(Severity::Info);
/// ```
///
/// # Remember
/// - `()` represents "no error"
/// - `GameError` represents "an error occurred"
/// - `Result<(), E: Into<GameError>>` is the standard pattern for fallible operations
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid `ToGameError` type",
    note = "the type should be `()`, `GameError`, or an `Option/Result` that can be converted into `Option<GameError>`"
)]
pub trait ToGameError: Sized {
    /// Converts the value into an optional [`GameError`].
    ///
    /// Returns `None` if the value represents success or absence of error,
    /// or `Some(GameError)` if an error occurred.
    fn to_err(self) -> Option<GameError>;

    /// Converts the value into an optional [`GameError`] with the specified severity.
    ///
    /// - If the value represents an error, the severity is applied to the resulting [`GameError`].
    /// - If the value represents success, `None` is returned regardless of the severity.
    #[inline]
    fn with_severity(self, severity: Severity) -> Option<GameError> {
        self.to_err().map(|e| GameError::with_severity(e, severity))
    }

    /// Converts the value into an optional [`GameError`] and map it's severity through given function.
    ///
    /// - If the value represents an error, the severity is applied to the resulting [`GameError`].
    /// - If the value represents success, `None` is returned regardless of the severity.
    #[inline]
    fn map_severity(self, f: impl FnOnce(Severity) -> Severity) -> Option<GameError> {
        self.to_err().map(|e| GameError::map_severity(e, f))
    }
}

impl ToGameError for () {
    /// The unit type `()` represents the absence of an error.
    ///
    /// Always returns `None`.
    #[inline(always)]
    fn to_err(self) -> Option<GameError> {
        None
    }

    #[inline(always)]
    fn with_severity(self, _: Severity) -> Option<GameError> {
        None
    }

    #[inline(always)]
    fn map_severity(self, _: impl FnOnce(Severity) -> Severity) -> Option<GameError> {
        None
    }
}

impl ToGameError for GameError {
    /// A [`GameError`] represents an error that occurred.
    ///
    /// Always returns `Some(self)`.
    #[inline(always)]
    fn to_err(self) -> Option<GameError> {
        Some(self)
    }
}

impl<T: ToGameError> ToGameError for Option<T> {
    /// Propagates `None` or converts `Some(T)` using `T::to_err()`.
    ///
    /// Returns `None` if the option is `None`, otherwise returns `T::to_err()`.
    fn to_err(self) -> Option<GameError> {
        self.and_then(|e| e.to_err())
    }
}

impl<T: ToGameError, E: Into<GameError>> ToGameError for Result<T, E> {
    /// Converts a `Result` into an optional error.
    ///
    /// - `Ok(value)` -> `value.to_err()` (propagates success)
    /// - `Err(error)` -> `Some(error.into())` (error occurred)
    fn to_err(self) -> Option<GameError> {
        match self {
            Ok(v) => v.to_err(),
            Err(e) => Some(e.into()),
        }
    }
}
