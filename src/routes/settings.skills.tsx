import { createFileRoute } from "@tanstack/react-router";

import { ImportedSkillSettings } from "#/features/imported-skill";

export const Route = createFileRoute("/settings/skills")({
  component: ImportedSkillSettings,
});
