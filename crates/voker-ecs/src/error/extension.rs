use super::{GameError, Severity};

// -----------------------------------------------------------------------------
// ext

pub trait ResultSeverityExt<T> {
    fn with_severity(self, severity: Severity) -> Result<T, GameError>;
}

impl<T, E> ResultSeverityExt<T> for Result<T, E>
where
    E: Into<GameError>,
{
    fn with_severity(self, severity: Severity) -> Result<T, GameError> {
        self.map_err(|e| e.into().with_severity(severity))
    }
}
