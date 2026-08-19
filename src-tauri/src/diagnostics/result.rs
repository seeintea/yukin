use crate::AppResult;

pub trait LogError<T> {
    fn log_error(self, operation: &'static str) -> Self;
}

impl<T> LogError<T> for AppResult<T> {
    fn log_error(self, operation: &'static str) -> Self {
        if let Err(error) = &self {
            tracing::error!(
                operation,
                error_code = error.code(),
                error = %error,
                "application operation failed"
            );
        }
        self
    }
}
