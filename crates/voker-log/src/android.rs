use alloc::ffi::CString;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::{Debug, Write};

use tracing::field::Field;
use tracing::span::{Attributes, Record};
use tracing::{Event, Id, Level, Subscriber};
use tracing_subscriber::{Layer, field::Visit};
use tracing_subscriber::{layer::Context, registry::LookupSpan};

#[derive(Default)]
pub(crate) struct AndroidLayer;

struct StringRecorder(String, bool);

impl StringRecorder {
    fn new() -> Self {
        StringRecorder(String::new(), false)
    }
}

impl Visit for StringRecorder {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if field.name() == "message" {
            if !self.0.is_empty() {
                self.0 = alloc::format!("{:?}\n{}", value, self.0)
            } else {
                self.0 = alloc::format!("{:?}", value)
            }
        } else {
            if self.1 {
                write!(self.0, " ").expect("write to string should not fail");
            } else {
                self.1 = true;
            }
            write!(self.0, "{} = {:?};", field.name(), value)
                .expect("write to string should not fail");
        }
    }
}

impl Default for StringRecorder {
    fn default() -> Self {
        StringRecorder::new()
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for AndroidLayer {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut new_debug_record = StringRecorder::new();
        attrs.record(&mut new_debug_record);

        if let Some(span_ref) = ctx.span(id) {
            span_ref.extensions_mut().insert::<StringRecorder>(new_debug_record);
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        if let Some(span_ref) = ctx.span(id)
            && let Some(debug_record) = span_ref.extensions_mut().get_mut::<StringRecorder>()
        {
            values.record(debug_record);
        }
    }

    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        fn sanitize(string: &str) -> CString {
            let bytes: Vec<u8> =
                string.as_bytes().iter().copied().filter(|byte| *byte != 0).collect();
            CString::new(bytes).expect("filtered bytes contain no NUL")
        }

        let mut recorder = StringRecorder::new();
        event.record(&mut recorder);
        let meta = event.metadata();

        let priority = match *meta.level() {
            Level::TRACE => android_log_sys::LogPriority::VERBOSE,
            Level::DEBUG => android_log_sys::LogPriority::DEBUG,
            Level::INFO => android_log_sys::LogPriority::INFO,
            Level::WARN => android_log_sys::LogPriority::WARN,
            Level::ERROR => android_log_sys::LogPriority::ERROR,
        };

        #[expect(unsafe_code, reason = "FFI call into Android logging API")]
        unsafe {
            android_log_sys::__android_log_write(
                priority as android_log_sys::c_int,
                sanitize(meta.name()).as_ptr(),
                sanitize(&recorder.0).as_ptr(),
            );
        }
    }
}
