//! Skill IPC commands — discover markdown skills, pick the active one, and
//! create/edit/rename/delete skill files (the in-app skills editor).

use super::DesktopState;
use serde::Serialize;
use std::path::Path;
use tauri::State;
use zanto_core::config::Settings;
use zanto_core::context::SkillScope;

/// Max preview length (chars) returned for a skill body, so the picker can show
/// a hint without shipping the whole file.
const PREVIEW_CHARS: usize = 120;

/// A discoverable skill: file stem, a short body preview, and which scope's dir
/// it lives in ("project" | "global").
#[derive(Serialize)]
pub struct SkillDto {
    pub name: String,
    pub preview: String,
    pub scope: &'static str,
}

/// Parse the frontend scope string into the core enum.
fn parse_scope(scope: &str) -> Result<SkillScope, String> {
    match scope {
        "project" => Ok(SkillScope::Project),
        "global" => Ok(SkillScope::Global),
        other => Err(format!("Unknown skill scope: {other}")),
    }
}

fn scope_label(scope: SkillScope) -> &'static str {
    match scope {
        SkillScope::Project => "project",
        SkillScope::Global => "global",
    }
}

fn preview_of(body: &str) -> String {
    body.trim().chars().take(PREVIEW_CHARS).collect()
}

/// List markdown skills across the project + global skills dirs, each tagged with
/// its scope. Project and global skills of the same name are listed separately
/// (the editor manages each dir independently); the composer picker ignores
/// `scope`. Within a scope, names are unique.
#[tauri::command]
pub fn list_skills() -> Vec<SkillDto> {
    let settings = Settings::load();
    let project_dir = settings.project_dir.as_deref().map(Path::new);
    let mut out = Vec::new();
    for scope in [SkillScope::Project, SkillScope::Global] {
        for s in zanto_core::context::list_skills_in(scope, project_dir) {
            out.push(SkillDto {
                name: s.name,
                preview: preview_of(&s.body),
                scope: scope_label(scope),
            });
        }
    }
    out
}

/// Read a single skill's full body from a SPECIFIC scope (not shadowed), for the
/// editor.
#[tauri::command]
pub fn read_skill(name: String, scope: String) -> Result<String, String> {
    let scope = parse_scope(&scope)?;
    let settings = Settings::load();
    let project_dir = settings.project_dir.as_deref().map(Path::new);
    zanto_core::context::read_skill_in(scope, project_dir, &name)
        .map(|s| s.body)
        .ok_or_else(|| format!("Skill '{name}' not found in {} scope", scope_label(scope)))
}

/// Create or overwrite a skill in the given scope; returns the fresh DTO so the
/// list can refresh immediately.
#[tauri::command]
pub fn save_skill(name: String, scope: String, body: String) -> Result<SkillDto, String> {
    let scope_enum = parse_scope(&scope)?;
    let settings = Settings::load();
    let project_dir = settings.project_dir.as_deref().map(Path::new);
    zanto_core::context::write_skill(scope_enum, project_dir, &name, &body)?;
    Ok(SkillDto {
        name: name.trim().to_string(),
        preview: preview_of(&body),
        scope: scope_label(scope_enum),
    })
}

/// Delete a skill in the given scope. If it was the active skill, clear the
/// selection (live + persisted) so the next turn doesn't reference a gone file.
#[tauri::command]
pub fn delete_skill(
    state: State<'_, DesktopState>,
    name: String,
    scope: String,
) -> Result<(), String> {
    let scope = parse_scope(&scope)?;
    let mut settings = Settings::load();
    let project_dir = settings.project_dir.as_deref().map(Path::new);
    zanto_core::context::delete_skill(scope, project_dir, &name)?;
    // Clear the active selection if it pointed at this skill.
    let trimmed = name.trim();
    if settings.selected_skill.as_deref() == Some(trimmed) {
        *state.selected_skill.lock().unwrap() = None;
        settings.selected_skill = None;
        settings.save().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Rename a skill within a scope. If it was the active skill, follow the rename
/// so the selection stays valid.
#[tauri::command]
pub fn rename_skill(
    state: State<'_, DesktopState>,
    old: String,
    new: String,
    scope: String,
) -> Result<(), String> {
    let scope = parse_scope(&scope)?;
    let mut settings = Settings::load();
    let project_dir = settings.project_dir.as_deref().map(Path::new);
    zanto_core::context::rename_skill(scope, project_dir, &old, &new)?;
    if settings.selected_skill.as_deref() == Some(old.trim()) {
        let new_name = new.trim().to_string();
        *state.selected_skill.lock().unwrap() = Some(new_name.clone());
        settings.selected_skill = Some(new_name);
        settings.save().map_err(|e| e.to_string())?;
    }
    Ok(())
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
