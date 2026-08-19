use crate::{agent::skills::SkillRegistry, protocol::skill::Metadata};

#[tauri::command]
pub fn agent_skill_list() -> Vec<Metadata> {
    SkillRegistry::list()
}
