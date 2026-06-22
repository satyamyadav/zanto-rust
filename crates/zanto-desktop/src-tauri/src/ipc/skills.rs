//! Skill IPC commands — discover markdown skills and pick the active one.

use super::DesktopState;
use serde::Serialize;
use std::path::Path;
use tauri::State;
use zanto_core::config::Settings;

/// Max preview length (chars) returned for a skill body, so the picker can show
/// a hint without shipping the whole file.
const PREVIEW_CHARS: usize = 120;

/// A discoverable skill: file stem plus a short body preview.
#[derive(Serialize)]
pub struct SkillDto {
    pub name: String,
    pub preview: String,
}

/// List markdown skills under the project + global skills dirs.
#[tauri::command]
pub fn list_skills() -> Vec<SkillDto> {
    let settings = Settings::load();
    zanto_core::context::list_skills(settings.project_dir.as_deref().map(Path::new))
        .into_iter()
        .map(|s| {
            let preview: String = s.body.trim().chars().take(PREVIEW_CHARS).collect();
            SkillDto {
                name: s.name,
                preview,
            }
        })
        .collect()
}

/// Set (or clear, with `None`) the user-selected skill appended on each turn.
#[tauri::command]
pub fn set_active_skill(
    state: State<'_, DesktopState>,
    name: Option<String>,
) -> Result<(), String> {
    *state.selected_skill.lock().unwrap() = name.clone();
    // Persist the choice so it survives a restart (the runtime Mutex above is the
    // live value used per turn; this writes it through to Settings).
    let mut settings = Settings::load();
    settings.selected_skill = name;
    settings.save().map_err(|e| e.to_string())?;
    Ok(())
}
