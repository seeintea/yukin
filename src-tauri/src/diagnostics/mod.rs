pub mod crash;
pub mod result;
pub mod tracing;

pub struct Guard {
    _tracing: tracing::Guard,
    _crash_reporter: Option<crash::Guard>,
}

pub fn init(log_dir: Option<&std::path::Path>) -> Guard {
    let tracing = tracing::init(log_dir);
    let crash_reporter = match crash::start() {
        Ok(guard) => {
            ::tracing::info!(crash_dir = ?crash::directory(), "native crash reporter initialized");
            Some(guard)
        }
        Err(error) => {
            ::tracing::error!(%error, "failed to initialize native crash reporter");
            None
        }
    };

    Guard {
        _tracing: tracing,
        _crash_reporter: crash_reporter,
    }
}
