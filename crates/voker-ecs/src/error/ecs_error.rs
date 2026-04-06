use alloc::boxed::Box;
use core::error::Error;
use core::fmt::{Debug, Display};

// -----------------------------------------------------------------------------
// Severity

#[derive(Debug, Clone, Copy, Hash)]
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// The error can be safely ignored,
    /// and can be completely discarded.
    Ignore,
    /// The error can be ignored, unless
    /// verbose debugging is required.
    Trace,
    /// The error can be safely ignored, but
    /// may need to be surfaced during debugging.
    Debug,
    /// Nothing has gone wrong, but the error is
    /// useful to the user and should be reported.
    Info,
    /// Something unexpected but recoverable
    /// happened. Something has probably gone wrong.
    Warning,
    /// A real error occurred, but the program
    /// may continue.
    Error,
    /// A fatal error; the program cannot continue.
    Panic,
}

// -----------------------------------------------------------------------------
// EcsError

struct InnerError {
    error: Box<dyn Error + Send + Sync + 'static>,
    severity: Severity,
    #[cfg(feature = "backtrace")]
    backtrace: std::backtrace::Backtrace,
}

pub struct EcsError {
    inner: Box<InnerError>,
}

impl EcsError {
    pub fn severity(&self) -> Severity {
        self.inner.severity
    }

    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.inner.severity = severity;
        self
    }
}

impl<E> From<E> for EcsError
where
    Box<dyn Error + Send + Sync + 'static>: From<E>,
{
    #[cold]
    fn from(error: E) -> Self {
        EcsError {
            inner: Box::new(InnerError {
                error: error.into(),
                severity: Severity::Panic,
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            }),
        }
    }
}

impl Display for EcsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.inner.error, f)?;
        #[cfg(feature = "backtrace")]
        Display::fmt(&self.inner.backtrace, f)?;
        Ok(())
    }
}

impl Debug for EcsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.inner.error, f)?;
        #[cfg(feature = "backtrace")]
        Debug::fmt(&self.inner.backtrace, f)?;
        Ok(())
    }
}
