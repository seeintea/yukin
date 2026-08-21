import { skipToken, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useReducer, useRef } from "react";

import { agentRunCancel, agentRunSnapshot, agentRunStart, toolCallDecide } from "#/api/agent-run";
import type { ChatInputValue } from "#/components/chat-input";
import type {
  ActiveToolCall,
  AgentRunEvent,
  AgentRunStartResponse,
  RunStatus,
  ToolCallDecision,
} from "#/protocol/agent-run";
import type { ConversationMessage, ConversationSnapshot } from "#/protocol/conversation";
import { showErrorToast } from "#/utils/toast";

import { agentRunKeys, conversationKeys, conversationQueryOptions } from "./queries";

type ActiveStatus = RunStatus | "idle";

interface ActiveRunState {
  runId: string | null;
  status: ActiveStatus;
  phase: "thinking" | "responding" | null;
  lastSequence: number;
  toolCalls: ActiveToolCall[];
}

type ActiveRunAction =
  | { type: "started"; runId: string }
  | { type: "event"; event: AgentRunEvent }
  | { type: "snapshot"; runId: string; status: RunStatus; toolCalls: ActiveToolCall[] };

const initialActiveRun: ActiveRunState = {
  runId: null,
  status: "idle",
  phase: null,
  lastSequence: 0,
  toolCalls: [],
};

function activeRunReducer(state: ActiveRunState, action: ActiveRunAction): ActiveRunState {
  if (action.type === "started") {
    if (state.runId === action.runId && state.lastSequence > 0) {
      return state;
    }
    return { runId: action.runId, status: "pending", phase: null, lastSequence: 0, toolCalls: [] };
  }
  if (action.type === "snapshot") {
    return { ...state, runId: action.runId, status: action.status, toolCalls: action.toolCalls };
  }
  if (state.runId === action.event.runId && action.event.sequence <= state.lastSequence) {
    return state;
  }

  let status = state.status;
  let phase = state.phase;
  let toolCalls = state.toolCalls;
  switch (action.event.event) {
    case "run_started":
      status = "running";
      break;
    case "phase_changed":
      status = "running";
      phase = action.event.data.phase;
      break;
    case "tool_call_requested":
      {
        status = "running";
        const {
          toolCallId,
          name,
          arguments: toolArguments,
          argumentsDigest,
          riskLevel,
          approvalPolicy,
        } = action.event.data;
        const timestamp = action.event.timestamp;
        toolCalls = [
          ...toolCalls.filter((toolCall) => toolCall.id !== toolCallId),
          {
            id: toolCallId,
            name,
            runId: action.event.runId,
            arguments: toolArguments,
            argumentsDigest,
            status: "requested",
            result: null,
            riskLevel,
            approvalPolicy,
            errorCode: null,
            errorMessage: null,
            approvalExpiresAt: null,
            createdAt: timestamp,
            completedAt: null,
          },
        ];
      }
      break;
    case "tool_approval_required":
      {
        const { toolCallId, argumentsDigest, expiresAt } = action.event.data;
        status = "waiting_approval";
        toolCalls = toolCalls.map((toolCall) =>
          toolCall.id === toolCallId
            ? {
                ...toolCall,
                status: "waiting_approval",
                argumentsDigest,
                approvalExpiresAt: expiresAt,
              }
            : toolCall,
        );
      }
      break;
    case "tool_call_started":
      {
        status = "running";
        const { toolCallId } = action.event.data;
        toolCalls = toolCalls.map((toolCall) =>
          toolCall.id === toolCallId ? { ...toolCall, status: "running" } : toolCall,
        );
      }
      break;
    case "tool_call_completed":
      {
        status = "running";
        const { toolCallId, result } = action.event.data;
        toolCalls = toolCalls.map((toolCall) =>
          toolCall.id === toolCallId ? { ...toolCall, status: "completed", result } : toolCall,
        );
      }
      break;
    case "tool_call_failed":
      {
        const { toolCallId, errorCode, errorMessage } = action.event.data;
        toolCalls = toolCalls.map((toolCall) =>
          toolCall.id === toolCallId
            ? { ...toolCall, status: "failed", errorCode, errorMessage }
            : toolCall,
        );
      }
      break;
    case "tool_call_rejected":
      {
        const { toolCallId } = action.event.data;
        status = "running";
        toolCalls = toolCalls.map((toolCall) =>
          toolCall.id === toolCallId ? { ...toolCall, status: "rejected" } : toolCall,
        );
      }
      break;
    case "run_completed":
      status = "completed";
      break;
    case "run_failed":
      status = "failed";
      break;
    case "run_cancelled":
      status = "cancelled";
      break;
  }

  return {
    runId: action.event.runId,
    status,
    phase,
    lastSequence: action.event.sequence,
    toolCalls,
  };
}

function isActiveStatus(status: ActiveStatus | undefined) {
  return status === "pending" || status === "running" || status === "waiting_approval";
}

function updateAssistantRunMessage(
  snapshot: ConversationSnapshot | undefined,
  runId: string,
  update: (message: ConversationMessage) => ConversationMessage,
) {
  if (!snapshot) {
    return snapshot;
  }

  return {
    ...snapshot,
    messages: snapshot.messages.map((message) =>
      message.runId === runId && message.role === "assistant" ? update(message) : message,
    ),
  };
}

function appendRunMessages(
  snapshot: ConversationSnapshot | undefined,
  response: AgentRunStartResponse,
  content: string,
  attachments: ConversationMessage["attachments"],
) {
  if (!snapshot || snapshot.messages.some((message) => message.id === response.userMessageId)) {
    return snapshot;
  }

  const timestamp = new Date().toISOString();
  const nextSequence = (snapshot.messages[snapshot.messages.length - 1]?.sequence ?? 0) + 1;
  const messages: ConversationMessage[] = [
    {
      id: response.userMessageId,
      runId: response.runId,
      role: "user",
      content,
      attachments,
      status: "completed",
      sequence: nextSequence,
      createdAt: timestamp,
      updatedAt: timestamp,
    },
    {
      id: response.assistantMessageId,
      runId: response.runId,
      role: "assistant",
      content: "",
      attachments: [],
      status: "streaming",
      sequence: nextSequence + 1,
      createdAt: timestamp,
      updatedAt: timestamp,
    },
  ];

  return { ...snapshot, messages: [...snapshot.messages, ...messages] };
}

function findPersistedActiveRunId(messages: ConversationMessage[]) {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    if (message.status === "streaming") {
      return message.runId;
    }
  }

  return null;
}

export function useChat(conversationId: string) {
  const queryClient = useQueryClient();
  const queryKey = useMemo(() => conversationKeys.find(conversationId), [conversationId]);
  const [activeRun, dispatch] = useReducer(activeRunReducer, initialActiveRun);
  const eventSequences = useRef(new Map<string, number>());
  const conversationQuery = useQuery(conversationQueryOptions(conversationId));
  const persistedActiveRunId = findPersistedActiveRunId(conversationQuery.data?.messages ?? []);
  const activeRunId = isActiveStatus(activeRun.status)
    ? activeRun.runId
    : (persistedActiveRunId ?? null);
  const snapshotQuery = useQuery({
    queryKey: agentRunKeys.snapshot(activeRunId ?? "inactive"),
    queryFn: activeRunId ? () => agentRunSnapshot(activeRunId) : skipToken,
    refetchInterval: (query) => {
      const status = query.state.data?.run.status;
      return status && !isActiveStatus(status) ? false : 500;
    },
  });

  useEffect(() => {
    const snapshot = snapshotQuery.data;
    if (!snapshot) {
      return;
    }

    queryClient.setQueryData<ConversationSnapshot>(queryKey, (conversation) =>
      conversation
        ? {
            ...conversation,
            messages: conversation.messages.map((message) =>
              message.id === snapshot.assistantMessage.id ? snapshot.assistantMessage : message,
            ),
          }
        : conversation,
    );
    dispatch({
      type: "snapshot",
      runId: snapshot.run.id,
      status: snapshot.run.status,
      toolCalls: snapshot.toolCalls,
    });
  }, [queryClient, queryKey, snapshotQuery.data]);

  const finishRun = (event: AgentRunEvent) => {
    const status =
      event.event === "run_completed"
        ? "completed"
        : event.event === "run_cancelled"
          ? "cancelled"
          : "failed";
    queryClient.setQueryData<ConversationSnapshot>(queryKey, (snapshot) =>
      updateAssistantRunMessage(snapshot, event.runId, (message) => ({ ...message, status })),
    );
    void Promise.all([
      queryClient.invalidateQueries({ queryKey }),
      queryClient.invalidateQueries({ queryKey: conversationKeys.list }),
      queryClient.invalidateQueries({ queryKey: agentRunKeys.snapshot(event.runId) }),
    ]);
  };

  const sendMutation = useMutation({
    mutationFn: async ({ content, selection, skillId, attachment }: ChatInputValue) => {
      const messageAttachments = attachment
        ? [{ name: attachment.name, size: attachment.size }]
        : [];
      const handleEvent = (event: AgentRunEvent) => {
        const lastSequence = eventSequences.current.get(event.runId) ?? 0;
        if (event.sequence <= lastSequence) {
          return;
        }
        eventSequences.current.set(event.runId, event.sequence);
        dispatch({ type: "event", event });

        if (event.event === "run_started") {
          queryClient.setQueryData<ConversationSnapshot>(queryKey, (snapshot) =>
            appendRunMessages(
              snapshot,
              {
                runId: event.runId,
                userMessageId: event.data.userMessageId,
                assistantMessageId: event.data.assistantMessageId,
              },
              content,
              messageAttachments,
            ),
          );
        } else if (event.event === "output_text_delta") {
          queryClient.setQueryData<ConversationSnapshot>(queryKey, (snapshot) =>
            updateAssistantRunMessage(snapshot, event.runId, (message) => ({
              ...message,
              content: message.content + event.data.content,
            })),
          );
        } else if (
          event.event === "run_completed" ||
          event.event === "run_failed" ||
          event.event === "run_cancelled"
        ) {
          finishRun(event);
          if (event.event === "run_failed") {
            showErrorToast("消息发送失败", event.data.errorMessage);
          }
        }
      };

      const response = await agentRunStart(
        {
          conversationId,
          providerId: selection.providerId,
          modelId: selection.modelId,
          reasoningEffort: selection.reasoningEffort,
          content,
          skillIds: skillId ? [skillId] : [],
          attachments: attachment ? [attachment] : [],
        },
        handleEvent,
      );
      dispatch({ type: "started", runId: response.runId });
      queryClient.setQueryData<ConversationSnapshot>(queryKey, (snapshot) =>
        appendRunMessages(snapshot, response, content, messageAttachments),
      );
      return response;
    },
    onError: (error) => {
      showErrorToast("消息发送失败", error);
    },
  });
  const cancelMutation = useMutation({
    mutationFn: async () => {
      if (activeRunId) {
        await agentRunCancel(activeRunId);
      }
    },
    onError: (error) => {
      showErrorToast("停止生成失败", error);
    },
  });
  const approvalMutation = useMutation({
    mutationFn: async ({
      toolCall,
      decision,
    }: {
      toolCall: ActiveToolCall;
      decision: ToolCallDecision;
    }) => {
      await toolCallDecide({
        runId: toolCall.runId,
        toolCallId: toolCall.id,
        argumentsDigest: toolCall.argumentsDigest,
        decision,
      });
      return toolCall.id;
    },
    onSuccess: (_, { toolCall }) => {
      void queryClient.invalidateQueries({ queryKey: agentRunKeys.snapshot(toolCall.runId) });
    },
    onError: (error) => {
      showErrorToast("工具审批失败", error);
    },
  });
  const snapshotStatus = snapshotQuery.data?.run.status;
  const isSending =
    sendMutation.isPending || isActiveStatus(activeRun.status) || isActiveStatus(snapshotStatus);

  return {
    messages: conversationQuery.data?.messages ?? [],
    sendMessage: sendMutation.mutate,
    cancelRun: cancelMutation.mutate,
    canCancel: activeRunId !== null,
    phase: activeRun.phase,
    toolCalls: activeRun.toolCalls,
    decideToolCall: (toolCall: ActiveToolCall, decision: ToolCallDecision) =>
      approvalMutation.mutate({ toolCall, decision }),
    decidingToolCallId: approvalMutation.isPending ? approvalMutation.variables?.toolCall.id : null,
    isPending: conversationQuery.isPending || isSending,
    isSending,
    isCancelling: cancelMutation.isPending,
    isError: conversationQuery.isError,
  };
}
