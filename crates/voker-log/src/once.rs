/// Call [`trace!`](crate::trace) once per call site.
#[macro_export]
macro_rules! trace_once {
    ($($arg:tt)+) => ({
        $crate::once_expr!($crate::trace!($($arg)+))
    });
}

/// Call [`debug!`](crate::debug) once per call site.
#[macro_export]
macro_rules! debug_once {
    ($($arg:tt)+) => ({
        $crate::once_expr!($crate::debug!($($arg)+))
    });
}

/// Call [`info!`](crate::info) once per call site.
#[macro_export]
macro_rules! info_once {
    ($($arg:tt)+) => ({
        $crate::once_expr!($crate::info!($($arg)+))
    });
}

/// Call [`warn!`](crate::warn) once per call site.
#[macro_export]
macro_rules! warn_once {
    ($($arg:tt)+) => ({
        $crate::once_expr!($crate::warn!($($arg)+))
    });
}

/// Call [`error!`](crate::error) once per call site.
#[macro_export]
macro_rules! error_once {
    ($($arg:tt)+) => ({
        $crate::once_expr!($crate::error!($($arg)+))
    });
}
