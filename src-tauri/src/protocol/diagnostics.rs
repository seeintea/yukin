use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontendErrorKind {
    Window,
    UnhandledRejection,
    ReactUncaught,
    ReactCaught,
    ReactRecoverable,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendErrorReportRequest {
    pub kind: FrontendErrorKind,
    pub message: String,
    pub stack: Option<String>,
    pub component_stack: Option<String>,
    pub source: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

impl FrontendErrorKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::UnhandledRejection => "unhandled_rejection",
            Self::ReactUncaught => "react_uncaught",
            Self::ReactCaught => "react_caught",
            Self::ReactRecoverable => "react_recoverable",
        }
    }

    pub const fn is_recoverable(self) -> bool {
        matches!(self, Self::ReactCaught | Self::ReactRecoverable)
    }
}
