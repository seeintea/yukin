use std::collections::HashSet;

use crate::protocol::{agent_run::RunSkill, skill::Metadata};

use super::RuntimeError;

const BASE_INSTRUCTIONS: &str = "You are Yukin, a concise and helpful desktop assistant. Follow the user's request, use only the tools made available for this run, and never claim that a tool action succeeded unless its result confirms success.";

struct SkillDefinition {
    id: &'static str,
    version: &'static str,
    title: &'static str,
    description: &'static str,
    instructions: &'static str,
    required_tools: &'static [&'static str],
}

const SKILLS: &[SkillDefinition] = &[
    SkillDefinition {
        id: "time_assistant",
        version: "1",
        title: "时间助手",
        description: "查询当前日期与时间，并按用户指定的时区回答。",
        instructions: "When the user asks for the current date or time, call current_time with the relevant UTC offset instead of relying on memory. State the timezone or UTC offset in the answer.",
        required_tools: &["current_time"],
    },
    SkillDefinition {
        id: "note_writer",
        version: "1",
        title: "文本笔记",
        description: "整理文本内容，并在明确要求时保存为本地笔记。",
        instructions: "Help the user prepare concise plain-text notes. Call save_text_note only when the user explicitly asks to save a note, and explain that saving requires approval.",
        required_tools: &["save_text_note"],
    },
];

pub(crate) struct ResolvedSkills {
    pub instructions: String,
    pub allowed_tools: HashSet<String>,
    pub selected: Vec<RunSkill>,
}

pub(crate) struct SkillRegistry;

impl SkillRegistry {
    pub(crate) fn list() -> Vec<Metadata> {
        SKILLS
            .iter()
            .map(|skill| Metadata {
                id: skill.id.into(),
                version: skill.version.into(),
                title: skill.title.into(),
                description: skill.description.into(),
                required_tools: skill
                    .required_tools
                    .iter()
                    .map(|tool| (*tool).into())
                    .collect(),
            })
            .collect()
    }

    pub(crate) fn resolve(
        skill_ids: &[String],
        available_tools: &HashSet<String>,
    ) -> Result<ResolvedSkills, RuntimeError> {
        if skill_ids.is_empty() {
            return Ok(ResolvedSkills {
                instructions: BASE_INSTRUCTIONS.into(),
                allowed_tools: available_tools.clone(),
                selected: Vec::new(),
            });
        }

        let mut instructions = String::from(BASE_INSTRUCTIONS);
        let mut allowed_tools = HashSet::new();
        let mut selected = Vec::new();
        let mut seen = HashSet::new();

        for skill_id in skill_ids {
            if !seen.insert(skill_id.as_str()) {
                continue;
            }
            let skill = SKILLS
                .iter()
                .find(|skill| skill.id == skill_id)
                .ok_or_else(|| RuntimeError::SkillNotFound(skill_id.clone()))?;

            instructions.push_str("\n\nSelected skill: ");
            instructions.push_str(skill.title);
            instructions.push('\n');
            instructions.push_str(skill.instructions);
            for tool in skill.required_tools {
                if !available_tools.contains(*tool) {
                    return Err(RuntimeError::SkillToolUnavailable {
                        skill: skill.id.into(),
                        tool: (*tool).into(),
                    });
                }
                allowed_tools.insert((*tool).into());
            }
            selected.push(RunSkill {
                id: skill.id.into(),
                version: skill.version.into(),
            });
        }

        Ok(ResolvedSkills {
            instructions,
            allowed_tools,
            selected,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::SkillRegistry;
    use crate::agent::RuntimeError;

    fn tools() -> HashSet<String> {
        ["current_time", "save_text_note"]
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn no_selection_keeps_all_available_tools() {
        let resolved = SkillRegistry::resolve(&[], &tools()).expect("general agent");

        assert_eq!(resolved.allowed_tools, tools());
        assert!(resolved.selected.is_empty());
    }

    #[test]
    fn selected_skill_adds_instructions_and_restricts_tools() {
        let resolved =
            SkillRegistry::resolve(&["time_assistant".into()], &tools()).expect("time skill");

        assert_eq!(
            resolved.allowed_tools,
            HashSet::from(["current_time".into()])
        );
        assert!(resolved.instructions.contains("current_time"));
        assert_eq!(resolved.selected[0].version, "1");
    }

    #[test]
    fn rejects_unknown_skill() {
        let error = SkillRegistry::resolve(&["missing".into()], &tools())
            .err()
            .expect("unknown skill rejected");

        assert_eq!(error, RuntimeError::SkillNotFound("missing".into()));
    }

    #[test]
    fn rejects_skill_with_unavailable_tool() {
        let error = SkillRegistry::resolve(
            &["note_writer".into()],
            &HashSet::from(["current_time".into()]),
        )
        .err()
        .expect("unavailable tool rejected");

        assert_eq!(
            error,
            RuntimeError::SkillToolUnavailable {
                skill: "note_writer".into(),
                tool: "save_text_note".into(),
            }
        );
    }
}
