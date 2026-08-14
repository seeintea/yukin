use std::result::Result;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
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
