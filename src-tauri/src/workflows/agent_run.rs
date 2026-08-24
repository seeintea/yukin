use std::{collections::BTreeMap, collections::HashSet, path::PathBuf, time::Duration};

use chrono::{SecondsFormat, Utc};
use futures_util::StreamExt;
use reqwest::Client;
use sqlx::SqlitePool;
use tokio::{sync::watch, time::timeout, time::Instant};
use uuid::Uuid;

use crate::{
    agent::{
        self,
        skills::SkillRegistry,
        tools::{arguments_digest, ApprovalPolicy, ExecutionAuthorization, ToolRegistry},
        CompletionRequest, Message, ModelError, Role, RuntimeError, ThinkingMode, TokenUsage,
        ToolCall, ToolCallFunction, ToolCallType,
    },
    files::{AuthorizedDirectory, AuthorizedFile, SelectedDirectories, SelectedFiles},
    protocol::{
        agent_run::{Event, EventKind, Phase, StartRequest, StartResponse, ToolCallDecision},
        conversation::{Attachment, DirectoryScope},
        model_provider::ReasoningEffort,
    },
    security::keychain,
    state::ActiveRuns,
    storage::{model_provider, model_response, tool_call},
    AppError, AppResult,
};

const PARTIAL_WRITE_INTERVAL: Duration = Duration::from_millis(500);
const PARTIAL_WRITE_CHARS: usize = 256;
const MAX_MODEL_STEPS: usize = 4;
const MAX_TOOL_CALLS: usize = 4;
const TOOL_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_TOOL_OUTPUT_BYTES: usize = 40 * 1024;
const APPROVAL_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub(crate) type EventSink = Box<dyn Fn(Event) + Send + Sync>;

pub(crate) struct PreparedRun {
    pub response: StartResponse,
    conversation_id: String,
    provider_id: String,
    model_id: String,
    reasoning_effort: Option<ReasoningEffort>,
    messages: Vec<Message>,
    allowed_tools: HashSet<String>,
    authorized_files: Vec<AuthorizedFile>,
    authorized_directories: Vec<AuthorizedDirectory>,
}

pub(crate) async fn prepare(
    pool: &SqlitePool,
    selected_files: &SelectedFiles,
    selected_directories: &SelectedDirectories,
    request: StartRequest,
) -> AppResult<PreparedRun> {
    if request.attachments.len() > 1 {
        return Err(AppError::Validation(
            "only one text file can be attached to a message".into(),
        ));
    }
    if request.directory_scopes.len() > 1 {
        return Err(AppError::Validation(
            "only one directory can be authorized for a message".into(),
        ));
    }
    let run_id = Uuid::now_v7().to_string();
    let user_message_id = Uuid::now_v7().to_string();
    let assistant_message_id = Uuid::now_v7().to_string();
    let mut available_tools = ToolRegistry::built_in(PathBuf::new()).names();
    available_tools.remove("read_selected_text_file");
    available_tools.remove("list_selected_directory");
    available_tools.remove("search_selected_directory");
    available_tools.remove("get_directory_entry_metadata");
    available_tools.remove("open_directory_entry");
    available_tools.remove("reveal_directory_entry");
    available_tools.remove("create_text_file_in_selected_directory");
    available_tools.remove("create_directory_in_selected_directory");
    let mut resolved_skills = SkillRegistry::resolve(&request.skill_ids, &available_tools)?;
    let authorized_files = request
        .attachments
        .iter()
        .map(|reference| selected_files.take(reference))
        .collect::<Result<Vec<_>, _>>()?;
    let authorized_directories = request
        .directory_scopes
        .iter()
        .map(|reference| selected_directories.take(reference))
        .collect::<Result<Vec<_>, _>>()?;
    if !authorized_files.is_empty() {
        resolved_skills
            .allowed_tools
            .insert("read_selected_text_file".into());
        for file in &authorized_files {
            let reference = file.reference();
            resolved_skills.instructions.push_str(&format!(
                "\n\nThe user attached the text file {:?} ({} bytes). Call read_selected_text_file with referenceId {:?} when its contents are needed. Never expose or invent a local path.",
                reference.name, reference.size, reference.reference_id
            ));
        }
    }
    if !authorized_directories.is_empty() {
        resolved_skills
            .allowed_tools
            .insert("list_selected_directory".into());
        resolved_skills
            .allowed_tools
            .insert("search_selected_directory".into());
        resolved_skills
            .allowed_tools
            .insert("get_directory_entry_metadata".into());
        resolved_skills
            .allowed_tools
            .insert("open_directory_entry".into());
        resolved_skills
            .allowed_tools
            .insert("reveal_directory_entry".into());
        resolved_skills
            .allowed_tools
            .insert("create_text_file_in_selected_directory".into());
        resolved_skills
            .allowed_tools
            .insert("create_directory_in_selected_directory".into());
        for directory in &authorized_directories {
            let reference = directory.reference();
            resolved_skills.instructions.push_str(&format!(
                "\n\nThe user authorized the directory {:?}. Call list_selected_directory with referenceId {:?} to inspect direct children or search_selected_directory with the same referenceId to search names recursively. Use create_text_file_in_selected_directory to create a new .txt file or create_directory_in_selected_directory to create a child directory at this authorized root. Results include an opaque targetReferenceId and relativePath. Pass both unchanged to get_directory_entry_metadata, open_directory_entry, or reveal_directory_entry. Creating, opening, and revealing require user approval. Never expose or invent a local path.",
                reference.name, reference.reference_id
            ));
        }
    }
    let attachments = authorized_files
        .iter()
        .map(|file| Attachment {
            name: file.reference().name.clone(),
            size: file.reference().size,
        })
        .collect();
    let directory_scopes = authorized_directories
        .iter()
        .map(|directory| DirectoryScope {
            name: directory.reference().name.clone(),
        })
        .collect();
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
            skills: resolved_skills.selected,
            attachments,
            directory_scopes,
        },
    )
    .await?;
    let mut messages = vec![Message::text(Role::System, resolved_skills.instructions)];
    messages.extend(
        history
            .into_iter()
            .map(|message| {
                let role = match message.role.as_str() {
                    "user" => Role::User,
                    "assistant" => Role::Assistant,
                    value => return Err(AppError::Other(format!("invalid message role: {value}"))),
                };
                Ok(Message::text(role, message.content))
            })
            .collect::<AppResult<Vec<_>>>()?,
    );
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
        allowed_tools: resolved_skills.allowed_tools,
        authorized_files,
        authorized_directories,
    })
}

pub(crate) async fn execute(
    pool: SqlitePool,
    client: Client,
    prepared: PreparedRun,
    mut cancellation: watch::Receiver<bool>,
    active_runs: ActiveRuns,
    tool_data_dir: PathBuf,
    events: EventSink,
) {
    tracing::info!(
        conversation_id = %prepared.conversation_id,
        run_id = %prepared.response.run_id,
        provider_id = %prepared.provider_id,
        model_id = %prepared.model_id,
        "agent run started"
    );
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

    let result = execute_stream(
        &pool,
        &client,
        &prepared,
        &mut cancellation,
        &active_runs,
        tool_data_dir,
        &mut emitter,
    )
    .await;
    match result {
        StreamOutcome::Completed { content, usage } => {
            let prompt_tokens = usage.as_ref().map(|value| value.prompt_tokens);
            let completion_tokens = usage.as_ref().map(|value| value.completion_tokens);
            let output_chars = content.chars().count();
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
            tracing::info!(
                conversation_id = %prepared.conversation_id,
                run_id = %prepared.response.run_id,
                provider_id = %prepared.provider_id,
                model_id = %prepared.model_id,
                output_chars,
                prompt_tokens,
                completion_tokens,
                "agent run completed"
            );
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
    active_runs: &ActiveRuns,
    tool_data_dir: PathBuf,
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
    let registry = ToolRegistry::with_authorizations(
        tool_data_dir,
        prepared.authorized_files.clone(),
        prepared.authorized_directories.clone(),
    );
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
        request.tools = registry.definitions_for(&prepared.allowed_tools);
        if let Some(reasoning_effort) = prepared.reasoning_effort {
            request.thinking = Some(ThinkingMode::Enabled);
            request.reasoning_effort = Some(reasoning_effort.into());
        }

        let stream_result = tokio::select! {
            changed = cancellation.changed() => {
                return cancellation_outcome(changed, cancellation, content);
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
                    return cancellation_outcome(changed, cancellation, content);
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
            if !prepared.allowed_tools.contains(&tool_call.function.name) {
                return failed(
                    content,
                    RuntimeError::ToolNotAllowed(tool_call.function.name).into(),
                );
            }
            let arguments =
                match serde_json::from_str::<serde_json::Value>(&tool_call.function.arguments) {
                    Ok(arguments) => arguments,
                    Err(error) => {
                        return failed(
                            content,
                            RuntimeError::InvalidToolArguments {
                                name: tool_call.function.name,
                                message: error.to_string(),
                            }
                            .into(),
                        );
                    }
                };
            let (risk_level, approval_policy) = match registry.metadata(&tool_call.function.name) {
                Ok(metadata) => metadata,
                Err(error) => return failed(content, error.into()),
            };
            let (arguments_json, digest) = match arguments_digest(&arguments) {
                Ok(value) => value,
                Err(error) => return failed(content, error.into()),
            };
            if let Err(error) = tool_call::create(
                pool,
                tool_call::CreateParams {
                    id: &tool_call.id,
                    run_id: &prepared.response.run_id,
                    name: &tool_call.function.name,
                    arguments_json: &arguments_json,
                    arguments_digest: &digest,
                    risk_level: risk_level.as_str(),
                    approval_policy: approval_policy.as_str(),
                },
            )
            .await
            {
                return failed(content, error);
            }
            emitter.emit(EventKind::ToolCallRequested {
                tool_call_id: tool_call.id.clone(),
                name: tool_call.function.name.clone(),
                arguments: arguments.clone(),
                arguments_digest: digest.clone(),
                risk_level: risk_level.into(),
                approval_policy: approval_policy.into(),
            });
            tracing::debug!(
                conversation_id = %prepared.conversation_id,
                run_id = %prepared.response.run_id,
                tool_call_id = %tool_call.id,
                tool_name = %tool_call.function.name,
                risk_level = risk_level.as_str(),
                approval_policy = approval_policy.as_str(),
                "tool call requested"
            );

            let validation = registry.validate(&tool_call.function.name, &arguments);
            tool_call_count += 1;
            let signature = format!("{}:{arguments_json}", tool_call.function.name);
            let policy_error = if tool_call_count > MAX_TOOL_CALLS {
                Some(RuntimeError::ToolCallLimit)
            } else if !prior_tool_calls.insert(signature) {
                Some(RuntimeError::RepeatedToolCall(
                    tool_call.function.name.clone(),
                ))
            } else {
                validation.err()
            };
            if let Some(error) = policy_error {
                persist_tool_failure(pool, emitter, &tool_call.id, &error).await;
                return failed(content, error.into());
            }

            let mut authorization = ExecutionAuthorization::NotRequired;
            if approval_policy == ApprovalPolicy::Always {
                let approval = active_runs
                    .wait_for_approval(prepared.response.run_id.clone(), tool_call.id.clone());
                let expires_at = (Utc::now() + chrono::Duration::minutes(5))
                    .to_rfc3339_opts(SecondsFormat::Millis, true);
                if let Err(error) = tool_call::wait_for_approval(
                    pool,
                    &prepared.response.run_id,
                    &tool_call.id,
                    &expires_at,
                )
                .await
                {
                    return failed(content, error);
                }
                emitter.emit(EventKind::ToolApprovalRequired {
                    tool_call_id: tool_call.id.clone(),
                    arguments_digest: digest.clone(),
                    expires_at,
                });
                tracing::info!(
                    conversation_id = %prepared.conversation_id,
                    run_id = %prepared.response.run_id,
                    tool_call_id = %tool_call.id,
                    tool_name = %tool_call.function.name,
                    "tool approval required"
                );

                let decision = tokio::select! {
                    changed = cancellation.changed() => {
                        return cancellation_outcome(changed, cancellation, content);
                    }
                    result = timeout(APPROVAL_TIMEOUT, approval) => result,
                };
                match decision {
                    Ok(Ok(ToolCallDecision::Allow)) => {
                        if let Err(error) = registry.validate(&tool_call.function.name, &arguments)
                        {
                            persist_tool_failure(pool, emitter, &tool_call.id, &error).await;
                            return failed(content, error.into());
                        }
                        authorization = ExecutionAuthorization::Approved {
                            arguments_digest: digest,
                        };
                    }
                    Ok(Ok(ToolCallDecision::Reject)) => {
                        emitter.emit(EventKind::ToolCallRejected {
                            tool_call_id: tool_call.id.clone(),
                        });
                        messages.push(Message::tool(
                            tool_call.id,
                            r#"{"status":"rejected","message":"User rejected this tool call"}"#
                                .into(),
                        ));
                        continue;
                    }
                    Ok(Err(_)) => {
                        let error = RuntimeError::ToolExecution {
                            name: tool_call.function.name,
                            message: "approval channel closed".into(),
                        };
                        persist_tool_failure(pool, emitter, &tool_call.id, &error).await;
                        return failed(content, error.into());
                    }
                    Err(_) => {
                        let error = RuntimeError::ApprovalExpired(tool_call.function.name);
                        if let Err(storage_error) =
                            tool_call::expire(pool, &prepared.response.run_id, &tool_call.id).await
                        {
                            return failed(content, storage_error);
                        }
                        emit_tool_failure(emitter, &tool_call.id, &error);
                        return failed(content, error.into());
                    }
                }
            }

            if let Err(error) =
                tool_call::mark_running(pool, &prepared.response.run_id, &tool_call.id).await
            {
                return failed(content, error);
            }
            emitter.emit(EventKind::ToolCallStarted {
                tool_call_id: tool_call.id.clone(),
            });

            let execution = tokio::select! {
                changed = cancellation.changed() => {
                    return cancellation_outcome(changed, cancellation, content);
                }
                result = timeout(
                    TOOL_TIMEOUT,
                    registry.execute(&tool_call.function.name, &arguments, authorization),
                ) => result,
            };
            let result = match execution {
                Ok(Ok(result)) => result,
                Ok(Err(error)) => {
                    persist_tool_failure(pool, emitter, &tool_call.id, &error).await;
                    return failed(content, error.into());
                }
                Err(_) => {
                    let error = RuntimeError::ToolTimeout(tool_call.function.name);
                    persist_tool_failure(pool, emitter, &tool_call.id, &error).await;
                    return failed(content, error.into());
                }
            };
            let result_json = match serde_json::to_string(&result) {
                Ok(result_json) if result_json.len() <= MAX_TOOL_OUTPUT_BYTES => result_json,
                Ok(_) => {
                    let error = RuntimeError::ToolOutputLimit(tool_call.function.name);
                    persist_tool_failure(pool, emitter, &tool_call.id, &error).await;
                    return failed(content, error.into());
                }
                Err(error) => {
                    let error = RuntimeError::ToolExecution {
                        name: tool_call.function.name,
                        message: error.to_string(),
                    };
                    persist_tool_failure(pool, emitter, &tool_call.id, &error).await;
                    return failed(content, error.into());
                }
            };
            let result_summary = registry.result_summary(&tool_call.function.name, &result);
            let result_summary_json = match serde_json::to_string(&result_summary) {
                Ok(value) => value,
                Err(error) => {
                    let error = RuntimeError::ToolExecution {
                        name: tool_call.function.name,
                        message: error.to_string(),
                    };
                    persist_tool_failure(pool, emitter, &tool_call.id, &error).await;
                    return failed(content, error.into());
                }
            };
            if let Err(error) = tool_call::complete(pool, &tool_call.id, &result_summary_json).await
            {
                return failed(content, error);
            }
            emitter.emit(EventKind::ToolCallCompleted {
                tool_call_id: tool_call.id.clone(),
                result: result_summary,
            });
            tracing::debug!(
                conversation_id = %prepared.conversation_id,
                run_id = %prepared.response.run_id,
                tool_call_id = %tool_call.id,
                tool_name = %tool_call.function.name,
                "tool call completed"
            );
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

async fn persist_tool_failure(
    pool: &SqlitePool,
    emitter: &mut EventEmitter,
    tool_call_id: &str,
    error: &RuntimeError,
) {
    if let Err(storage_error) =
        tool_call::fail(pool, tool_call_id, error.code(), &error.to_string()).await
    {
        tracing::error!(
            %storage_error,
            conversation_id = %emitter.conversation_id,
            run_id = %emitter.run_id,
            %tool_call_id,
            "failed to persist tool call failure"
        );
    }
    tracing::warn!(
        conversation_id = %emitter.conversation_id,
        run_id = %emitter.run_id,
        %tool_call_id,
        error_code = error.code(),
        error = %error,
        "tool call failed"
    );
    emit_tool_failure(emitter, tool_call_id, error);
}

fn failed(content: impl Into<String>, error: AppError) -> StreamOutcome {
    StreamOutcome::Failed {
        content: content.into(),
        error,
    }
}

fn cancellation_outcome(
    changed: Result<(), watch::error::RecvError>,
    cancellation: &watch::Receiver<bool>,
    content: String,
) -> StreamOutcome {
    if changed.is_ok() && *cancellation.borrow() {
        StreamOutcome::Cancelled { content }
    } else {
        failed(
            content,
            AppError::Other("run cancellation channel closed".into()),
        )
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
        tracing::error!(
            %storage_error,
            conversation_id = %prepared.conversation_id,
            run_id = %prepared.response.run_id,
            provider_id = %prepared.provider_id,
            model_id = %prepared.model_id,
            "failed to persist run failure"
        );
    }
    tracing::error!(
        conversation_id = %prepared.conversation_id,
        run_id = %prepared.response.run_id,
        provider_id = %prepared.provider_id,
        model_id = %prepared.model_id,
        %error_code,
        error = %error_message,
        "agent run failed"
    );
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
    tracing::info!(
        conversation_id = %prepared.conversation_id,
        run_id = %prepared.response.run_id,
        provider_id = %prepared.provider_id,
        model_id = %prepared.model_id,
        output_chars = content.chars().count(),
        "agent run cancelled"
    );
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
