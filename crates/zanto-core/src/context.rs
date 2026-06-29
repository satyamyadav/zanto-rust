//! Context sources + markdown skills/preprompt loader.
//!
//! Two capabilities live here:
//! - `load_context`: read the user's configured **context sources** (files/dirs)
//!   into one prompt block, injected into every turn's system prompt.
//! - `list_skills` / `get_skill`: discover markdown **skills/preprompts** under
//!   `<project_dir>/.zanto/skills/*.md` (and a global skills dir) that can be
//!   selected to steer a session.
//!
//! No filesystem watching: callers re-load explicitly. Missing paths are skipped
//! (warn-logged, never an error).

use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::config::ContextSource;

/// Max bytes read from any single context file. Larger files are truncated.
pub const PER_FILE_CAP: usize = 16 * 1024;
/// Max total bytes across all context sources. Once reached, no more is appended.
pub const TOTAL_CAP: usize = 32 * 1024;

/// A markdown skill/preprompt. `name` is the file stem; `body` is the file text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    pub body: String,
}

/// Concatenate the user's context sources into one prompt block.
///
/// - Plain files are read directly.
/// - Directories contribute their **top-level** `*.md`/`*.txt` files only
///   (sorted by path, no recursion).
/// - Each file is capped at [`PER_FILE_CAP`]; the whole block is capped at
///   [`TOTAL_CAP`]. Truncation appends a `… [truncated]` note.
/// - Missing/unreadable paths are skipped (warn-logged), never an error.
/// - **Disabled** sources (`enabled == false`) are skipped entirely.
///
/// Each contributing file is wrapped with a `--- context: <path> ---` header.
pub fn load_context(sources: &[ContextSource]) -> String {
    let mut out = String::new();

    for source in sources.iter().filter(|s| s.enabled) {
        let path = Path::new(&source.path);
        if path.is_dir() {
            for file in dir_text_files(path) {
                if !append_file(&mut out, &file) {
                    return out;
                }
            }
        } else if path.is_file() {
            if !append_file(&mut out, path) {
                return out;
            }
        } else {
            eprintln!(
                "[zanto] warn: context source not found, skipping: {}",
                source.path
            );
        }
    }

    out
}

/// Top-level `*.md`/`*.txt` files in `dir`, sorted by path. Non-recursive.
fn dir_text_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file() && is_text_ext(p))
            .collect(),
        Err(e) => {
            eprintln!(
                "[zanto] warn: cannot read context dir {}: {e}",
                dir.display()
            );
            Vec::new()
        }
    };
    files.sort();
    files
}

fn is_text_ext(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("md") | Some("txt")
    )
}

/// Append one file's content (header + capped body) to `out`. Returns `false`
/// when the total cap has been reached and no more sources should be processed.
fn append_file(out: &mut String, path: &Path) -> bool {
    if out.len() >= TOTAL_CAP {
        return false;
    }

    let body = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "[zanto] warn: cannot read context file {}: {e}",
                path.display()
            );
            return true;
        }
    };

    let header = format!("--- context: {} ---\n", path.display());
    out.push_str(&header);

    // Per-file cap.
    let (mut chunk, truncated_file) = truncate_on_char_boundary(&body, PER_FILE_CAP);

    // Total cap: never let `out` exceed TOTAL_CAP.
    let remaining = TOTAL_CAP.saturating_sub(out.len());
    let truncated_total = chunk.len() > remaining;
    if truncated_total {
        chunk = truncate_on_char_boundary(chunk, remaining).0;
    }

    out.push_str(chunk);
    if truncated_file || truncated_total {
        out.push_str("\n… [truncated]");
    }
    out.push_str("\n\n");
    true
}

/// Borrow the prefix of `s` no longer than `max` bytes, ending on a UTF-8 char
/// boundary. Returns the slice and whether truncation occurred.
fn truncate_on_char_boundary(s: &str, max: usize) -> (&str, bool) {
    if s.len() <= max {
        return (s, false);
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    (&s[..end], true)
}

/// Directories searched for skills, in priority order: project first, then global.
/// `<project_dir>/.zanto/skills` and the global `<data_dir>/skills`.
fn skill_dirs(project_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(proj) = project_dir {
        dirs.push(proj.join(".zanto").join("skills"));
    }
    if let Some(global) = global_skills_dir() {
        dirs.push(global);
    }
    dirs
}

/// Global skills directory: `<user data dir>/zanto/skills`.
fn global_skills_dir() -> Option<PathBuf> {
    ProjectDirs::from("", "", "zanto").map(|d| d.data_dir().join("skills"))
}

/// Which on-disk skills directory a CRUD operation targets. `list_skills`/
/// `get_skill` dedupe across both (project shadows global); the editor pins a
/// scope so it reads/writes the exact file the user picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillScope {
    Project,
    Global,
}

/// The skills directory for a given scope, or `None` when it can't be resolved
/// (project scope with no project dir, or no platform data dir for global).
pub fn skill_dir(scope: SkillScope, project_dir: Option<&Path>) -> Option<PathBuf> {
    match scope {
        SkillScope::Project => project_dir.map(|p| p.join(".zanto").join("skills")),
        SkillScope::Global => global_skills_dir(),
    }
}

/// Validate a skill name so it is always a single safe filename — never a path
/// that could escape the skills dir. Allows letters/digits/`-`/`_`/space; rejects
/// empties, path separators, `..`, and leading dots.
pub fn validate_skill_name(name: &str) -> Result<(), String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("Skill name cannot be empty".into());
    }
    if n.len() > 100 {
        return Err("Skill name is too long".into());
    }
    if n.starts_with('.') {
        return Err("Skill name cannot start with a dot".into());
    }
    if n.contains('/') || n.contains('\\') || n.contains("..") {
        return Err("Skill name cannot contain path separators".into());
    }
    if !n
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == ' ')
    {
        return Err("Skill name may only contain letters, digits, spaces, - and _".into());
    }
    Ok(())
}

/// The `<dir>/<name>.md` path for a scope, after validating the name. Errors when
/// the scope's dir can't be resolved or the name is unsafe.
fn skill_path(
    scope: SkillScope,
    project_dir: Option<&Path>,
    name: &str,
) -> Result<PathBuf, String> {
    validate_skill_name(name)?;
    let dir = skill_dir(scope, project_dir).ok_or_else(|| match scope {
        SkillScope::Project => "No project is set".to_string(),
        SkillScope::Global => "Cannot resolve the global skills directory".to_string(),
    })?;
    Ok(dir.join(format!("{}.md", name.trim())))
}

/// List skills in ONE scope's directory only (no cross-scope dedupe), sorted by
/// name. The editor uses this so each scope's files are managed independently.
pub fn list_skills_in(scope: SkillScope, project_dir: Option<&Path>) -> Vec<Skill> {
    let dir = match skill_dir(scope, project_dir) {
        Some(d) => d,
        None => return Vec::new(),
    };
    let mut by_name: std::collections::BTreeMap<String, Skill> = std::collections::BTreeMap::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(), // missing dir → no skills
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let name = match path.file_stem().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        match std::fs::read_to_string(&path) {
            Ok(body) => {
                by_name.insert(name.clone(), Skill { name, body });
            }
            Err(e) => eprintln!("[zanto] warn: cannot read skill {}: {e}", path.display()),
        }
    }
    by_name.into_values().collect()
}

/// Read a single skill's body from a SPECIFIC scope (not shadowed). Returns `None`
/// when the file doesn't exist in that scope's dir.
pub fn read_skill_in(scope: SkillScope, project_dir: Option<&Path>, name: &str) -> Option<Skill> {
    let path = skill_path(scope, project_dir, name).ok()?;
    if !path.is_file() {
        return None;
    }
    match std::fs::read_to_string(&path) {
        Ok(body) => Some(Skill {
            name: name.trim().to_string(),
            body,
        }),
        Err(e) => {
            eprintln!("[zanto] warn: cannot read skill {}: {e}", path.display());
            None
        }
    }
}

/// Write a skill file in the given scope. Creates the skills dir if missing and
/// validates the name (no traversal). When `overwrite` is false, refuses if a
/// skill of that name already exists — so creating a NEW skill can't silently
/// clobber an existing one (editing an existing skill passes `overwrite = true`).
pub fn write_skill(
    scope: SkillScope,
    project_dir: Option<&Path>,
    name: &str,
    body: &str,
    overwrite: bool,
) -> Result<(), String> {
    let path = skill_path(scope, project_dir, name)?;
    if !overwrite && path.is_file() {
        return Err(format!("A skill named '{}' already exists", name.trim()));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Cannot create skills dir: {e}"))?;
    }
    std::fs::write(&path, body).map_err(|e| format!("Cannot write skill: {e}"))
}

/// Delete a skill file in the given scope. Errors if it doesn't exist.
pub fn delete_skill(
    scope: SkillScope,
    project_dir: Option<&Path>,
    name: &str,
) -> Result<(), String> {
    let path = skill_path(scope, project_dir, name)?;
    if !path.is_file() {
        return Err(format!("Skill '{}' does not exist", name.trim()));
    }
    std::fs::remove_file(&path).map_err(|e| format!("Cannot delete skill: {e}"))
}

/// Rename a skill file within the same scope. Validates both names; errors if the
/// source is missing or the target already exists.
pub fn rename_skill(
    scope: SkillScope,
    project_dir: Option<&Path>,
    old: &str,
    new: &str,
) -> Result<(), String> {
    let from = skill_path(scope, project_dir, old)?;
    let to = skill_path(scope, project_dir, new)?;
    if !from.is_file() {
        return Err(format!("Skill '{}' does not exist", old.trim()));
    }
    if to.is_file() {
        return Err(format!("A skill named '{}' already exists", new.trim()));
    }
    std::fs::rename(&from, &to).map_err(|e| format!("Cannot rename skill: {e}"))
}

/// Discover skills in `<project_dir>/.zanto/skills/*.md` and the global skills
/// dir. Project skills take precedence over global ones with the same name.
/// Result is sorted by name.
pub fn list_skills(project_dir: Option<&Path>) -> Vec<Skill> {
    let mut by_name: std::collections::BTreeMap<String, Skill> = std::collections::BTreeMap::new();

    for dir in skill_dirs(project_dir) {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue, // missing dir → no skills there
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let name = match path.file_stem().and_then(|s| s.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            // Project dir is processed first; don't let global override it.
            if by_name.contains_key(&name) {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(body) => {
                    by_name.insert(name.clone(), Skill { name, body });
                }
                Err(e) => {
                    eprintln!("[zanto] warn: cannot read skill {}: {e}", path.display());
                }
            }
        }
    }

    by_name.into_values().collect()
}

/// Fetch a single skill by name (file stem). Project dir wins over global.
pub fn get_skill(project_dir: Option<&Path>, name: &str) -> Option<Skill> {
    for dir in skill_dirs(project_dir) {
        let path = dir.join(format!("{name}.md"));
        if path.is_file() {
            match std::fs::read_to_string(&path) {
                Ok(body) => {
                    return Some(Skill {
                        name: name.to_string(),
                        body,
                    });
                }
                Err(e) => {
                    eprintln!("[zanto] warn: cannot read skill {}: {e}", path.display());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Build an enabled `ContextSource` from a path-like value.
    fn enabled(path: impl ToString) -> ContextSource {
        ContextSource {
            path: path.to_string(),
            enabled: true,
        }
    }

    #[test]
    fn load_context_reads_file_dir_and_skips_missing() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // A standalone file source.
        let file = root.join("notes.md");
        fs::write(&file, "hello notes").unwrap();

        // A directory source with mixed extensions.
        let dir = root.join("docs");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("a.md"), "alpha").unwrap();
        fs::write(dir.join("b.txt"), "bravo").unwrap();
        fs::write(dir.join("ignore.rs"), "fn main() {}").unwrap();

        let sources = vec![
            enabled(file.display()),
            enabled(dir.display()),
            enabled(root.join("does-not-exist").display()),
        ];
        let out = load_context(&sources);

        assert!(out.contains("--- context:"));
        assert!(out.contains("hello notes"));
        assert!(out.contains("alpha"));
        assert!(out.contains("bravo"));
        // Non-.md/.txt files are excluded.
        assert!(!out.contains("fn main"));
        // Missing path produced no content/error.
    }

    #[test]
    fn load_context_respects_total_cap() {
        let tmp = TempDir::new().unwrap();
        let big = tmp.path().join("big.txt");
        // Larger than TOTAL_CAP so truncation must kick in.
        fs::write(&big, "x".repeat(TOTAL_CAP * 2)).unwrap();

        let out = load_context(&[enabled(big.display())]);
        assert!(out.len() <= TOTAL_CAP + 64); // header + truncation note slack
        assert!(out.contains("[truncated]"));
    }

    #[test]
    fn load_context_skips_disabled_sources() {
        let tmp = TempDir::new().unwrap();
        let on = tmp.path().join("on.md");
        let off = tmp.path().join("off.md");
        fs::write(&on, "enabled body").unwrap();
        fs::write(&off, "disabled body").unwrap();

        let sources = vec![
            ContextSource {
                path: on.display().to_string(),
                enabled: true,
            },
            ContextSource {
                path: off.display().to_string(),
                enabled: false,
            },
        ];
        let out = load_context(&sources);

        assert!(out.contains("enabled body"));
        assert!(!out.contains("disabled body"));
    }

    #[test]
    fn list_and_get_skills_over_temp_dir() {
        let tmp = TempDir::new().unwrap();
        let skills = tmp.path().join(".zanto").join("skills");
        fs::create_dir_all(&skills).unwrap();
        fs::write(skills.join("planner.md"), "be a planner").unwrap();
        fs::write(skills.join("coder.md"), "be a coder").unwrap();
        fs::write(skills.join("notes.txt"), "not a skill").unwrap();

        let found = list_skills(Some(tmp.path()));
        let names: Vec<&str> = found.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["coder", "planner"]); // sorted, .txt excluded

        let one = get_skill(Some(tmp.path()), "planner").unwrap();
        assert_eq!(one.name, "planner");
        assert_eq!(one.body, "be a planner");

        assert!(get_skill(Some(tmp.path()), "missing").is_none());
    }

    #[test]
    fn list_skills_no_project_dir_is_ok() {
        // Should not panic and should not error when no project dir is given.
        let _ = list_skills(None);
    }

    #[test]
    fn skill_crud_round_trip_project_scope() {
        let tmp = TempDir::new().unwrap();
        let proj = Some(tmp.path());

        // write → list_in → read_in round-trip
        write_skill(SkillScope::Project, proj, "tester", "do the thing", false).unwrap();
        let listed = list_skills_in(SkillScope::Project, proj);
        assert_eq!(listed.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["tester"]);
        let got = read_skill_in(SkillScope::Project, proj, "tester").unwrap();
        assert_eq!(got.body, "do the thing");

        // create (overwrite=false) refuses to clobber an existing skill…
        assert!(write_skill(SkillScope::Project, proj, "tester", "clobber", false).is_err());
        assert_eq!(read_skill_in(SkillScope::Project, proj, "tester").unwrap().body, "do the thing");
        // …but an explicit edit (overwrite=true) replaces it.
        write_skill(SkillScope::Project, proj, "tester", "v2", true).unwrap();
        assert_eq!(read_skill_in(SkillScope::Project, proj, "tester").unwrap().body, "v2");

        // rename
        rename_skill(SkillScope::Project, proj, "tester", "qa").unwrap();
        assert!(read_skill_in(SkillScope::Project, proj, "tester").is_none());
        assert_eq!(read_skill_in(SkillScope::Project, proj, "qa").unwrap().body, "v2");

        // delete
        delete_skill(SkillScope::Project, proj, "qa").unwrap();
        assert!(read_skill_in(SkillScope::Project, proj, "qa").is_none());
        assert!(delete_skill(SkillScope::Project, proj, "qa").is_err()); // already gone
    }

    #[test]
    fn skill_name_validation_rejects_traversal() {
        assert!(validate_skill_name("good_name-1").is_ok());
        assert!(validate_skill_name("with space").is_ok());
        for bad in ["", "  ", "../evil", "a/b", "a\\b", ".hidden", "x..y"] {
            assert!(validate_skill_name(bad).is_err(), "should reject {bad:?}");
        }
        // The write path must refuse an unsafe name (no file escapes the dir).
        let tmp = TempDir::new().unwrap();
        assert!(write_skill(SkillScope::Project, Some(tmp.path()), "../evil", "x", false).is_err());
        assert!(!tmp.path().join("evil.md").exists());
    }

    #[test]
    fn editor_reads_are_scope_pinned_not_shadowed() {
        // A project `reviewer` shadows a global one in list_skills/get_skill, but
        // the editor must read the file in the scope it was asked for.
        let proj_root = TempDir::new().unwrap();
        let global_root = TempDir::new().unwrap();
        // Stand in a fake global dir by writing both via the project path helper:
        // here we exercise the project scope directly, and a separate dir for the
        // "global" case using project scope rooted at a different temp dir.
        write_skill(SkillScope::Project, Some(proj_root.path()), "reviewer", "PROJECT body", false).unwrap();
        write_skill(SkillScope::Project, Some(global_root.path()), "reviewer", "GLOBAL body", false).unwrap();

        // Reading project scope at proj_root yields the project body; reading the
        // other root yields its own — proving reads are pinned to the given dir,
        // never deduped/shadowed.
        assert_eq!(
            read_skill_in(SkillScope::Project, Some(proj_root.path()), "reviewer").unwrap().body,
            "PROJECT body"
        );
        assert_eq!(
            read_skill_in(SkillScope::Project, Some(global_root.path()), "reviewer").unwrap().body,
            "GLOBAL body"
        );
    }

    #[test]
    fn project_scope_with_no_project_dir_errors_cleanly() {
        // No project dir → project-scope ops error (not panic), global unaffected.
        assert!(write_skill(SkillScope::Project, None, "x", "y", false).is_err());
        assert!(read_skill_in(SkillScope::Project, None, "x").is_none());
        assert!(list_skills_in(SkillScope::Project, None).is_empty());
    }
}
