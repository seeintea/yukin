import { Channel, invoke } from "@tauri-apps/api/core";

import type {
  AgentRunEvent,
  AgentRunSnapshot,
  AgentRunStartRequest,
  AgentRunStartResponse,
} from "#/protocol/agent-run";

export function agentRunStart(
  request: AgentRunStartRequest,
  onEvent: (event: AgentRunEvent) => void,
): Promise<AgentRunStartResponse> {
  const events = new Channel<AgentRunEvent>(onEvent);

  return invoke("agent_run_start", { request, events });
}

export function agentRunSnapshot(runId: string): Promise<AgentRunSnapshot> {
  return invoke("agent_run_snapshot", { request: { runId } });
}

export async function agentRunCancel(runId: string): Promise<void> {
  await invoke("agent_run_cancel", { request: { runId } });
}
