use std::{collections::BTreeMap, collections::HashSet, time::Duration};

use chrono::{SecondsFormat, Utc};
use futures_util::StreamExt;
use reqwest::Client;
use sqlx::SqlitePool;
use tokio::{sync::watch, time::timeout, time::Instant};
use uuid::Uuid;

use crate::{
    agent::{
        self, tools::ToolRegistry, CompletionRequest, Message, ModelError, Role, RuntimeError,
        ThinkingMode, TokenUsage, ToolCall, ToolCallFunction, ToolCallType,
    },
    protocol::{
        agent_run::{Event, EventKind, Phase, StartRequest, StartResponse},
        model_provider::ReasoningEffort,
    },
    security::keychain,
    storage::{model_provider, model_response},
    AppError, AppResult,
};

const PARTIAL_WRITE_INTERVAL: Duration = Duration::from_millis(500);
const PARTIAL_WRITE_CHARS: usize = 256;
const MAX_MODEL_STEPS: usize = 4;
const MAX_TOOL_CALLS: usize = 4;
const TOOL_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_TOOL_OUTPUT_BYTES: usize = 32 * 1024;

pub(crate) type EventSink = Box<dyn Fn(Event) + Send + Sync>;

pub(crate) struct PreparedRun {
    pub response: StartResponse,
    conversation_id: String,
    provider_id: String,
    model_id: String,
    reasoning_effort: Option<ReasoningEffort>,
    messages: Vec<Message>,
}

pub(crate) async fn prepare(pool: &SqlitePool, request: StartRequest) -> AppResult<PreparedRun> {
    let run_id = Uuid::now_v7().to_string();
    let user_message_id = Uuid::now_v7().to_string();
    let assistant_message_id = Uuid::now_v7().to_string();
    let history = model_response::start(
        pool,
        model_response::StartParams {
            conversation_id: request.conversation_id.clone(),
            run_id: run_id.clone(),
            user_message_id: user_message_id.clone(),
            assistant_message_id: assistant_message_id.clone(),
            provider_id: request.provider_id.clone(),
            model_id: request.model_id.clone(),
            reasoning_effort: request
                .reasoning_effort
                .map(|effort| effort.as_str().into()),
            content: request.content.clone(),
        },
    )
    .await?;
    let mut messages = history
        .into_iter()
        .map(|message| {
            let role = match message.role.as_str() {
                "user" => Role::User,
                "assistant" => Role::Assistant,
                value => return Err(AppError::Other(format!("invalid message role: {value}"))),
            };
            Ok(Message::text(role, message.content))
        })
        .collect::<AppResult<Vec<_>>>()?;
    messages.push(Message::text(Role::User, request.content));

    Ok(PreparedRun {
        response: StartResponse {
            run_id,
            user_message_id,
            assistant_message_id,
        },
        conversation_id: request.conversation_id,
        provider_id: request.provider_id,
        model_id: request.model_id,
        reasoning_effort: request.reasoning_effort,
        messages,
    })
}

pub(crate) async fn execute(
    pool: SqlitePool,
    client: Client,
    prepared: PreparedRun,
    mut cancellation: watch::Receiver<bool>,
    events: EventSink,
) {
    let mut emitter = EventEmitter::new(
        prepared.conversation_id.clone(),
        prepared.response.run_id.clone(),
        events,
    );
    emitter.emit(EventKind::RunStarted {
        user_message_id: prepared.response.user_message_id.clone(),
        assistant_message_id: prepared.response.assistant_message_id.clone(),
    });

    if *cancellation.borrow() {
        finish_cancelled(&pool, &prepared, "", &mut emitter).await;
        return;
    }

    if let Err(error) = model_response::mark_started(&pool, &prepared.response.run_id).await {
        finish_failed(&pool, &prepared, "", error, &mut emitter).await;
        return;
    }

    let result = execute_stream(&pool, &client, &prepared, &mut cancellation, &mut emitter).await;
    match result {
        StreamOutcome::Completed { content, usage } => {
            if let Err(error) = model_response::complete(
                &pool,
                &prepared.response.run_id,
                &prepared.response.assistant_message_id,
                &content,
                usage,
            )
            .await
            {
                finish_failed(&pool, &prepared, &content, error, &mut emitter).await;
                return;
            }
            emitter.emit(EventKind::RunCompleted {});
        }
        StreamOutcome::Cancelled { content } => {
            finish_cancelled(&pool, &prepared, &content, &mut emitter).await;
        }
        StreamOutcome::Failed { content, error } => {
            finish_failed(&pool, &prepared, &content, error, &mut emitter).await;
        }
    }
}

enum StreamOutcome {
    Completed {
        content: String,
        usage: Option<TokenUsage>,
    },
    Cancelled {
        content: String,
    },
    Failed {
        content: String,
        error: AppError,
    },
}

async fn execute_stream(
    pool: &SqlitePool,
    client: &Client,
    prepared: &PreparedRun,
    cancellation: &mut watch::Receiver<bool>,
    emitter: &mut EventEmitter,
) -> StreamOutcome {
    let config = match model_provider::find_runtime_config(pool, &prepared.provider_id).await {
        Ok(config) => config,
        Err(error) => return failed("", error),
    };
    let api_key = match keychain::get(&config.api_key_alias) {
        Ok(Some(api_key)) => api_key,
        Ok(None) => return failed("", ModelError::MissingCredential.into()),
        Err(error) => return failed("", error),
    };
    let registry = ToolRegistry::built_in();
    let mut messages = prepared.messages.clone();
    let mut content = String::new();
    let mut usage = TokenUsage {
        prompt_tokens: 0,
        completion_tokens: 0,
        total_tokens: 0,
    };
    let mut has_usage = false;
    let mut last_write = Instant::now();
    let mut pending_chars = 0;
    let mut tool_call_count = 0;
    let mut prior_tool_calls = HashSet::new();

    for step in 0..MAX_MODEL_STEPS {
        let mut request = CompletionRequest::new(prepared.model_id.clone(), messages.clone());
        request.tools = registry.definitions();
        if let Some(reasoning_effort) = prepared.reasoning_effort {
            request.thinking = Some(ThinkingMode::Enabled);
            request.reasoning_effort = Some(reasoning_effort.into());
        }

        let stream_result = tokio::select! {
            changed = cancellation.changed() => {
                if changed.is_ok() && *cancellation.borrow() {
                    return StreamOutcome::Cancelled { content };
                }
                return failed(content, AppError::Other("run cancellation channel closed".into()));
            }
            result = agent::stream_completion(
                client,
                config.api_format,
                &config.base_url,
                &api_key,
                request,
            ) => result,
        };
        let mut stream = match stream_result {
            Ok(stream) => stream,
            Err(error) => return failed(content, error.into()),
        };

        let mut phase = if prepared.reasoning_effort.is_some() {
            Phase::Thinking
        } else {
            Phase::Responding
        };
        emitter.emit(EventKind::PhaseChanged { phase });
        let mut step_content = String::new();
        let mut reasoning_content = String::new();
        let mut tool_calls = BTreeMap::<usize, PendingToolCall>::new();

        loop {
            let event = tokio::select! {
                changed = cancellation.changed() => {
                    if changed.is_ok() && *cancellation.borrow() {
                        return StreamOutcome::Cancelled { content };
                    }
                    return failed(content, AppError::Other("run cancellation channel closed".into()));
                }
                event = stream.next() => event,
            };
            let Some(event) = event else {
                break;
            };
            match event {
                Ok(agent::StreamEvent::ReasoningDelta { content: delta }) => {
                    reasoning_content.push_str(&delta);
                    if phase != Phase::Thinking {
                        phase = Phase::Thinking;
                        emitter.emit(EventKind::PhaseChanged { phase });
                    }
                }
                Ok(agent::StreamEvent::TextDelta { content: delta }) => {
                    if phase != Phase::Responding {
                        phase = Phase::Responding;
                        emitter.emit(EventKind::PhaseChanged { phase });
                    }
                    step_content.push_str(&delta);
                    content.push_str(&delta);
                    pending_chars += delta.chars().count();
                    emitter.emit(EventKind::OutputTextDelta { content: delta });
                    if pending_chars >= PARTIAL_WRITE_CHARS
                        || last_write.elapsed() >= PARTIAL_WRITE_INTERVAL
                    {
                        if let Err(error) = model_response::update_partial(
                            pool,
                            &prepared.response.assistant_message_id,
                            &content,
                        )
                        .await
                        {
                            return failed(content, error);
                        }
                        pending_chars = 0;
                        last_write = Instant::now();
                    }
                }
                Ok(agent::StreamEvent::ToolCallDelta {
                    index,
                    id,
                    name,
                    arguments,
                }) => {
                    let tool_call = tool_calls.entry(index).or_default();
                    if let Some(id) = id {
                        tool_call.id = Some(id);
                    }
                    if let Some(name) = name {
                        tool_call.name.push_str(&name);
                    }
                    tool_call.arguments.push_str(&arguments);
                }
                Ok(agent::StreamEvent::Completed {
                    usage: completed_usage,
                    ..
                }) => {
                    if let Some(completed_usage) = completed_usage {
                        has_usage = true;
                        usage.prompt_tokens += completed_usage.prompt_tokens;
                        usage.completion_tokens += completed_usage.completion_tokens;
                        usage.total_tokens += completed_usage.total_tokens;
                        emitter.emit(EventKind::UsageUpdated {
                            prompt_tokens: usage.prompt_tokens,
                            completion_tokens: usage.completion_tokens,
                            total_tokens: usage.total_tokens,
                        });
                    }
                }
                Err(error) => return failed(content, error.into()),
            }
        }

        if tool_calls.is_empty() {
            return StreamOutcome::Completed {
                content,
                usage: has_usage.then_some(usage),
            };
        }
        if step + 1 == MAX_MODEL_STEPS {
            return failed(content, RuntimeError::StepLimit.into());
        }

        let tool_calls = match assemble_tool_calls(tool_calls) {
            Ok(tool_calls) => tool_calls,
            Err(error) => return failed(content, error.into()),
        };
        messages.push(Message::assistant_tool_calls(
            step_content,
            (!reasoning_content.is_empty()).then_some(reasoning_content),
            tool_calls.clone(),
        ));

        for tool_call in tool_calls {
            let arguments =
                match serde_json::from_str::<serde_json::Value>(&tool_call.function.arguments) {
                    Ok(arguments) => arguments,
                    Err(error) => {
                        emitter.emit(EventKind::ToolCallRequested {
                            tool_call_id: tool_call.id.clone(),
                            name: tool_call.function.name.clone(),
                            arguments: serde_json::Value::String(
                                tool_call.function.arguments.clone(),
                            ),
                        });
                        let error = RuntimeError::InvalidToolArguments {
                            name: tool_call.function.name,
                            message: error.to_string(),
                        };
                        emit_tool_failure(emitter, &tool_call.id, &error);
                        return failed(content, error.into());
                    }
                };
            emitter.emit(EventKind::ToolCallRequested {
                tool_call_id: tool_call.id.clone(),
                name: tool_call.function.name.clone(),
                arguments: arguments.clone(),
            });
            tool_call_count += 1;
            if tool_call_count > MAX_TOOL_CALLS {
                let error = RuntimeError::ToolCallLimit;
                emit_tool_failure(emitter, &tool_call.id, &error);
                return failed(content, error.into());
            }
            let signature = format!("{}:{arguments}", tool_call.function.name);
            if !prior_tool_calls.insert(signature) {
                let error = RuntimeError::RepeatedToolCall(tool_call.function.name);
                emit_tool_failure(emitter, &tool_call.id, &error);
                return failed(content, error.into());
            }
            if let Err(error) = registry.metadata(&tool_call.function.name) {
                emit_tool_failure(emitter, &tool_call.id, &error);
                return failed(content, error.into());
            }
            emitter.emit(EventKind::ToolCallStarted {
                tool_call_id: tool_call.id.clone(),
            });

            let execution = tokio::select! {
                changed = cancellation.changed() => {
                    if changed.is_ok() && *cancellation.borrow() {
                        return StreamOutcome::Cancelled { content };
                    }
                    return failed(content, AppError::Other("run cancellation channel closed".into()));
                }
                result = timeout(
                    TOOL_TIMEOUT,
                    registry.execute(&tool_call.function.name, &arguments),
                ) => result,
            };
            let result = match execution {
                Ok(Ok(result)) => result,
                Ok(Err(error)) => {
                    emit_tool_failure(emitter, &tool_call.id, &error);
                    return failed(content, error.into());
                }
                Err(_) => {
                    let error = RuntimeError::ToolTimeout(tool_call.function.name);
                    emit_tool_failure(emitter, &tool_call.id, &error);
                    return failed(content, error.into());
                }
            };
            let result_json = match serde_json::to_string(&result) {
                Ok(result_json) if result_json.len() <= MAX_TOOL_OUTPUT_BYTES => result_json,
                Ok(_) => {
                    let error = RuntimeError::ToolOutputLimit(tool_call.function.name);
                    emit_tool_failure(emitter, &tool_call.id, &error);
                    return failed(content, error.into());
                }
                Err(error) => {
                    let error = RuntimeError::ToolExecution {
                        name: tool_call.function.name,
                        message: error.to_string(),
                    };
                    emit_tool_failure(emitter, &tool_call.id, &error);
                    return failed(content, error.into());
                }
            };
            emitter.emit(EventKind::ToolCallCompleted {
                tool_call_id: tool_call.id.clone(),
                result,
            });
            messages.push(Message::tool(tool_call.id, result_json));
        }
    }

    failed(content, RuntimeError::StepLimit.into())
}

#[derive(Default)]
struct PendingToolCall {
    id: Option<String>,
    name: String,
    arguments: String,
}

fn assemble_tool_calls(
    tool_calls: BTreeMap<usize, PendingToolCall>,
) -> Result<Vec<ToolCall>, RuntimeError> {
    tool_calls
        .into_values()
        .map(|tool_call| {
            let id = tool_call.id.ok_or_else(|| RuntimeError::ToolExecution {
                name: tool_call.name.clone(),
                message: "model omitted tool call id".into(),
            })?;
            if tool_call.name.is_empty() {
                return Err(RuntimeError::ToolExecution {
                    name: "unknown".into(),
                    message: "model omitted tool name".into(),
                });
            }
            Ok(ToolCall {
                id,
                kind: ToolCallType::Function,
                function: ToolCallFunction {
                    name: tool_call.name,
                    arguments: tool_call.arguments,
                },
            })
        })
        .collect()
}

fn emit_tool_failure(emitter: &mut EventEmitter, tool_call_id: &str, error: &RuntimeError) {
    emitter.emit(EventKind::ToolCallFailed {
        tool_call_id: tool_call_id.into(),
        error_code: error.code().into(),
        error_message: error.to_string(),
    });
}

fn failed(content: impl Into<String>, error: AppError) -> StreamOutcome {
    StreamOutcome::Failed {
        content: content.into(),
        error,
    }
}

async fn finish_failed(
    pool: &SqlitePool,
    prepared: &PreparedRun,
    content: &str,
    error: AppError,
    emitter: &mut EventEmitter,
) {
    let error_code = error.code().to_string();
    let error_message = error.to_string();
    if let Err(storage_error) = model_response::fail(
        pool,
        &prepared.response.run_id,
        &prepared.response.assistant_message_id,
        content,
        &error_code,
        &error_message,
    )
    .await
    {
        tracing::error!(%storage_error, run_id = %prepared.response.run_id, "failed to persist run failure");
    }
    emitter.emit(EventKind::RunFailed {
        error_code,
        error_message,
    });
}

async fn finish_cancelled(
    pool: &SqlitePool,
    prepared: &PreparedRun,
    content: &str,
    emitter: &mut EventEmitter,
) {
    if let Err(error) = model_response::cancel(
        pool,
        &prepared.response.run_id,
        &prepared.response.assistant_message_id,
        content,
    )
    .await
    {
        finish_failed(pool, prepared, content, error, emitter).await;
        return;
    }
    emitter.emit(EventKind::RunCancelled {});
}

struct EventEmitter {
    conversation_id: String,
    run_id: String,
    sequence: u64,
    events: EventSink,
}

impl EventEmitter {
    fn new(conversation_id: String, run_id: String, events: EventSink) -> Self {
        Self {
            conversation_id,
            run_id,
            sequence: 0,
            events,
        }
    }

    fn emit(&mut self, kind: EventKind) {
        self.sequence += 1;
        let event = Event {
            schema_version: 1,
            event_id: Uuid::now_v7().to_string(),
            conversation_id: self.conversation_id.clone(),
            run_id: self.run_id.clone(),
            sequence: self.sequence,
            timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            kind,
        };
        (self.events)(event);
    }
}
