use std::time::Duration;

use chrono::{SecondsFormat, Utc};
use futures_util::StreamExt;
use reqwest::Client;
use sqlx::SqlitePool;
use tokio::{sync::watch, time::Instant};
use uuid::Uuid;

use crate::{
    agent::{self, CompletionRequest, Message, ModelError, Role, ThinkingMode, TokenUsage},
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
            Ok(Message {
                role,
                content: message.content,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    messages.push(Message {
        role: Role::User,
        content: request.content,
    });

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
    let mut request = CompletionRequest::new(prepared.model_id.clone(), prepared.messages.clone());
    if let Some(reasoning_effort) = prepared.reasoning_effort {
        request.thinking = Some(ThinkingMode::Enabled);
        request.reasoning_effort = Some(reasoning_effort.into());
    }

    let stream_result = tokio::select! {
        changed = cancellation.changed() => {
            if changed.is_ok() && *cancellation.borrow() {
                return StreamOutcome::Cancelled { content: String::new() };
            }
            return failed("", AppError::Other("run cancellation channel closed".into()));
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
        Err(error) => return failed("", error.into()),
    };

    let mut phase = if prepared.reasoning_effort.is_some() {
        Phase::Thinking
    } else {
        Phase::Responding
    };
    emitter.emit(EventKind::PhaseChanged { phase });
    let mut content = String::new();
    let mut usage = None;
    let mut last_write = Instant::now();
    let mut pending_chars = 0;

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
            return StreamOutcome::Completed { content, usage };
        };
        match event {
            Ok(agent::StreamEvent::ReasoningDelta { .. }) => {
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
            Ok(agent::StreamEvent::Completed {
                usage: completed_usage,
                ..
            }) => {
                usage = completed_usage;
                if let Some(usage) = completed_usage {
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
