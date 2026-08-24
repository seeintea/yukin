import { invoke } from "@tauri-apps/api/core";

import type { DeleteRequest, ImportedSkill, SetEnabledRequest } from "#/protocol/imported-skill";

export function importedSkillImportDirectory(): Promise<ImportedSkill | null> {
  return invoke("imported_skill_import_directory");
}

export function importedSkillImportArchive(): Promise<ImportedSkill | null> {
  return invoke("imported_skill_import_archive");
}

export function importedSkillList(): Promise<ImportedSkill[]> {
  return invoke("imported_skill_list");
}

export function importedSkillSetEnabled(request: SetEnabledRequest): Promise<ImportedSkill> {
  return invoke("imported_skill_set_enabled", { request });
}

export async function importedSkillDelete(request: DeleteRequest): Promise<void> {
  await invoke("imported_skill_delete", { request });
}
