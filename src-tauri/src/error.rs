#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("workspace not set")]
    NoWorkspace,
    #[error("path escapes workspace: {0}")]
    PathEscape(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("keyring: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("dialog cancelled")]
    DialogCancelled,
    #[error("shell: {0}")]
    Shell(String),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("llm: {0}")]
    Llm(String),
    #[error("cancelled")]
    Cancelled,
    #[error("migrate: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("tauri: {0}")]
    Tauri(#[from] tauri::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
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
            AppError::NoWorkspace => "no_workspace",
            AppError::PathEscape(_) => "path_escape",
            AppError::Io(_) => "io",
            AppError::Db(_) => "db",
            AppError::Keyring(_) => "keyring",
            AppError::DialogCancelled => "dialog_cancelled",
            AppError::Shell(_) => "shell",
            AppError::Http(_) => "http",
            AppError::Llm(_) => "llm",
            AppError::Cancelled => "cancelled",
            AppError::Migrate(_) => "migrate",
            AppError::Tauri(_) => "tauri",
            AppError::Json(_) => "json",
            AppError::Other(_) => "other",
        };
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("code", code)?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

pub type AppResult<T> = std::result::Result<T, AppError>;
