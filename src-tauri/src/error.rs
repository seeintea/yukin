use std::result::Result;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("model: {0}")]
    Model(#[from] crate::agent::ModelError),
    #[error("agent: {0}")]
    Agent(#[from] crate::agent::RuntimeError),
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
    #[error("run state: {0}")]
    RunState(String),
    #[error("{0}")]
    Other(String),
}

impl AppError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Model(error) => error.code(),
            Self::Agent(error) => error.code(),
            Self::Io(_) => "io",
            Self::Db(_) => "db",
            Self::Migrate(_) => "migrate",
            Self::Tauri(_) => "tauri",
            Self::Keyring(_) => "keyring",
            Self::RunState(_) => "run_state",
            Self::Other(_) => "other",
        }
    }
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("code", self.code())?;
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

    #[test]
    fn serializes_run_state_error_code() {
        let error = AppError::RunState("run is not active".into());

        let value = serde_json::to_value(error).expect("serializable run state error");

        assert_eq!(value["code"], "run_state");
        assert_eq!(value["message"], "run state: run is not active");
    }

    #[test]
    fn serializes_agent_limit_error_code() {
        let error = AppError::from(crate::agent::RuntimeError::StepLimit);

        let value = serde_json::to_value(error).expect("serializable agent error");

        assert_eq!(value["code"], "agent_step_limit");
        assert_eq!(
            value["message"],
            "agent: agent reached the maximum model steps"
        );
    }
}
