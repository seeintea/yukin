use crate::protocol::diagnostics::FrontendErrorReportRequest;

const MAX_MESSAGE_CHARS: usize = 4 * 1024;
const MAX_STACK_CHARS: usize = 16 * 1024;
const MAX_SOURCE_CHARS: usize = 2 * 1024;

#[tauri::command]
pub fn diagnostics_frontend_error_report(request: FrontendErrorReportRequest) {
    let kind = request.kind.as_str();
    let message = truncate(request.message, MAX_MESSAGE_CHARS);
    let stack = request.stack.map(|value| truncate(value, MAX_STACK_CHARS));
    let component_stack = request
        .component_stack
        .map(|value| truncate(value, MAX_STACK_CHARS));
    let source = request
        .source
        .map(|value| truncate(value, MAX_SOURCE_CHARS));

    if request.kind.is_recoverable() {
        tracing::warn!(
            frontend.error_kind = kind,
            frontend.message = message,
            frontend.stack = stack,
            frontend.component_stack = component_stack,
            frontend.source = source,
            frontend.line = request.line,
            frontend.column = request.column,
            "frontend error"
        );
    } else {
        tracing::error!(
            frontend.error_kind = kind,
            frontend.message = message,
            frontend.stack = stack,
            frontend.component_stack = component_stack,
            frontend.source = source,
            frontend.line = request.line,
            frontend.column = request.column,
            "frontend error"
        );
    }
}

fn truncate(value: String, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value
    } else {
        value.chars().take(max_chars).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn truncates_frontend_diagnostics_on_character_boundaries() {
        assert_eq!(truncate("异常信息".into(), 2), "异常");
        assert_eq!(truncate("short".into(), 10), "short");
    }
}
