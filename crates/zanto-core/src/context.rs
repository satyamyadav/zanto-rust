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
///
/// Each contributing file is wrapped with a `--- context: <path> ---` header.
pub fn load_context(sources: &[String]) -> String {
    let mut out = String::new();

    for source in sources {
        let path = Path::new(source);
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
            eprintln!("[zanto] warn: context source not found, skipping: {source}");
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
            eprintln!("[zanto] warn: cannot read context dir {}: {e}", dir.display());
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
            eprintln!("[zanto] warn: cannot read context file {}: {e}", path.display());
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
                    })
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
            file.display().to_string(),
            dir.display().to_string(),
            root.join("does-not-exist").display().to_string(),
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

        let out = load_context(&[big.display().to_string()]);
        assert!(out.len() <= TOTAL_CAP + 64); // header + truncation note slack
        assert!(out.contains("[truncated]"));
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
}
