use std::result::Result;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("model: {0}")]
    Model(#[from] crate::agent::ModelError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("migrate: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("tauri: {0}")]
    Tauri(#[from] tauri::Error),
    #[error("keyring: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("{0}")]
    Other(String),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let code = match self {
            AppError::Model(error) => error.code(),
            AppError::Io(_) => "io",
            AppError::Db(_) => "db",
            AppError::Migrate(_) => "migrate",
            AppError::Tauri(_) => "tauri",
            AppError::Keyring(_) => "keyring",
            AppError::Other(_) => "other",
        };
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("code", code)?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use crate::agent::ModelError;

    use super::AppError;

    #[test]
    fn serializes_model_error_code() {
        let error = AppError::from(ModelError::RateLimited {
            message: "slow down".into(),
        });

        let value = serde_json::to_value(error).expect("serializable model error");

        assert_eq!(value["code"], "model_rate_limited");
        assert_eq!(
            value["message"],
            "model: model request was rate limited: slow down"
        );
    }
}
