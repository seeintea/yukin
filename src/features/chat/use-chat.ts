import { skipToken, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useReducer, useRef } from "react";

import { agentRunCancel, agentRunSnapshot, agentRunStart } from "#/api/agent-run";
import type { ChatInputValue } from "#/components/chat-input";
import type { AgentRunEvent, AgentRunStartResponse, RunStatus } from "#/protocol/agent-run";
import type { ConversationMessage, ConversationSnapshot } from "#/protocol/conversation";
import { toast } from "#/shadcn/toast";

import { agentRunKeys, conversationKeys, conversationQueryOptions } from "./queries";

type ActiveStatus = RunStatus | "idle";

interface ActiveRunState {
  runId: string | null;
  status: ActiveStatus;
  phase: "thinking" | "responding" | null;
  lastSequence: number;
}

type ActiveRunAction =
  | { type: "started"; runId: string }
  | { type: "event"; event: AgentRunEvent }
  | { type: "snapshot"; runId: string; status: RunStatus };

const initialActiveRun: ActiveRunState = {
  runId: null,
  status: "idle",
  phase: null,
  lastSequence: 0,
};

function activeRunReducer(state: ActiveRunState, action: ActiveRunAction): ActiveRunState {
  if (action.type === "started") {
    if (state.runId === action.runId && state.lastSequence > 0) {
      return state;
    }
    return { runId: action.runId, status: "pending", phase: null, lastSequence: 0 };
  }
  if (action.type === "snapshot") {
    return { ...state, runId: action.runId, status: action.status };
  }
  if (state.runId === action.event.runId && action.event.sequence <= state.lastSequence) {
    return state;
  }

  let status = state.status;
  let phase = state.phase;
  switch (action.event.event) {
    case "run_started":
      status = "running";
      break;
    case "phase_changed":
      status = "running";
      phase = action.event.data.phase;
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
  };
}

function getErrorMessage(error: unknown) {
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }

  return "请稍后重试";
}

function isActiveStatus(status: ActiveStatus | undefined) {
  return status === "pending" || status === "running" || status === "waiting_approval";
}

function appendRunMessages(
  snapshot: ConversationSnapshot | undefined,
  response: AgentRunStartResponse,
  content: string,
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
    dispatch({ type: "snapshot", runId: snapshot.run.id, status: snapshot.run.status });
  }, [queryClient, queryKey, snapshotQuery.data]);

  const finishRun = (event: AgentRunEvent) => {
    const status =
      event.event === "run_completed"
        ? "completed"
        : event.event === "run_cancelled"
          ? "cancelled"
          : "failed";
    queryClient.setQueryData<ConversationSnapshot>(queryKey, (snapshot) =>
      snapshot
        ? {
            ...snapshot,
            messages: snapshot.messages.map((message) =>
              message.runId === event.runId && message.role === "assistant"
                ? { ...message, status }
                : message,
            ),
          }
        : snapshot,
    );
    void Promise.all([
      queryClient.invalidateQueries({ queryKey }),
      queryClient.invalidateQueries({ queryKey: conversationKeys.list }),
      queryClient.invalidateQueries({ queryKey: agentRunKeys.snapshot(event.runId) }),
    ]);
  };

  const sendMutation = useMutation({
    mutationFn: async ({ content, selection }: ChatInputValue) => {
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
            ),
          );
        } else if (event.event === "output_text_delta") {
          queryClient.setQueryData<ConversationSnapshot>(queryKey, (snapshot) =>
            snapshot
              ? {
                  ...snapshot,
                  messages: snapshot.messages.map((message) =>
                    message.runId === event.runId && message.role === "assistant"
                      ? { ...message, content: message.content + event.data.content }
                      : message,
                  ),
                }
              : snapshot,
          );
        } else if (
          event.event === "run_completed" ||
          event.event === "run_failed" ||
          event.event === "run_cancelled"
        ) {
          finishRun(event);
          if (event.event === "run_failed") {
            toast.add({
              title: "消息发送失败",
              description: event.data.errorMessage,
              type: "error",
              priority: "high",
            });
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
        },
        handleEvent,
      );
      dispatch({ type: "started", runId: response.runId });
      queryClient.setQueryData<ConversationSnapshot>(queryKey, (snapshot) =>
        appendRunMessages(snapshot, response, content),
      );
      return response;
    },
    onError: (error) => {
      toast.add({
        title: "消息发送失败",
        description: getErrorMessage(error),
        type: "error",
        priority: "high",
      });
    },
  });
  const cancelMutation = useMutation({
    mutationFn: async () => {
      if (activeRunId) {
        await agentRunCancel(activeRunId);
      }
    },
    onError: (error) => {
      toast.add({
        title: "停止生成失败",
        description: getErrorMessage(error),
        type: "error",
        priority: "high",
      });
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
    isPending: conversationQuery.isPending || isSending,
    isSending,
    isCancelling: cancelMutation.isPending,
    isError: conversationQuery.isError,
  };
}
