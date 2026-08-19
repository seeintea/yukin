import { invoke } from "@tauri-apps/api/core";

import type { AgentSkill } from "#/protocol/skill";

export function agentSkillList(): Promise<AgentSkill[]> {
  return invoke("agent_skill_list");
}
