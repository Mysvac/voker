use super::{EcsError, Severity};

// -----------------------------------------------------------------------------
// ext

pub trait ResultSeverityExt<T> {
    fn with_severity(self, severity: Severity) -> Result<T, EcsError>;
}

impl<T, E> ResultSeverityExt<T> for Result<T, E>
where
    E: Into<EcsError>,
{
    fn with_severity(self, severity: Severity) -> Result<T, EcsError> {
        self.map_err(|e| e.into().with_severity(severity))
    }
}
