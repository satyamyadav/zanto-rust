/// End-to-end CLI tests. Require a live Ollama instance at the default endpoint.
/// Marked `#[ignore]` — skipped by plain `cargo test`.
///
/// Run with: `cargo test -p zanto-cli -- --include-ignored`
use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

struct Harness {
    workspace: TempDir,
    db_dir: TempDir,
}

impl Harness {
    fn new() -> Self {
        let workspace = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();

        let zanto_dir = workspace.path().join(".zanto");
        fs::create_dir_all(&zanto_dir).unwrap();

        let settings = serde_json::json!({
            "allowed_paths": [workspace.path().to_str().unwrap()],
            "allow_read_outside": false,
            "allow_write_outside": false
        });
        fs::write(
            zanto_dir.join("settings.json"),
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();

        Self { workspace, db_dir }
    }

    fn db_path(&self) -> String {
        self.db_dir.path().join("test.db").to_str().unwrap().to_string()
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("zanto").unwrap();
        cmd.current_dir(self.workspace.path())
            .env("ZANTO_DB", self.db_path());
        cmd
    }

    fn stdout(&self, args: &[&str]) -> String {
        let out = self.cmd().args(args).output().unwrap();
        assert!(out.status.success(), "exit {}: {}", out.status, String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).to_string()
    }
}

#[test]
#[ignore]
fn one_shot_returns_output() {
    let h = Harness::new();
    let out = h.stdout(&["say the word PONG and nothing else"]);
    assert!(
        out.to_uppercase().contains("PONG"),
        "expected PONG in output, got:\n{out}"
    );
}

#[test]
#[ignore]
fn list_directory_tool_used() {
    let h = Harness::new();
    fs::write(h.workspace.path().join("probe.txt"), "sentinel").unwrap();

    let out = h.stdout(&["list all files in the current working directory"]);
    assert!(
        out.to_lowercase().contains("probe"),
        "expected 'probe' in output, got:\n{out}"
    );
}

#[test]
#[ignore]
fn write_file_creates_file() {
    let h = Harness::new();
    let target = h.workspace.path().join("result.txt");

    h.stdout(&["write the text HELLO to result.txt"]);

    assert!(target.exists(), "result.txt was not created");
    let content = fs::read_to_string(&target).unwrap();
    assert!(
        content.to_uppercase().contains("HELLO"),
        "expected HELLO in file, got: {content}"
    );
}

#[test]
#[ignore]
fn session_persists_across_runs() {
    let h = Harness::new();

    h.stdout(&["please remember the number 7741 for me"]);

    // Get the session id from the list
    let list = h.stdout(&["sessions", "list"]);
    let session_id = list
        .lines()
        .skip(2)
        .next()
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or("")
        .to_string();
    assert!(!session_id.is_empty(), "no session id in:\n{list}");

    let second = h.stdout(&["--session", &session_id, "what number did I ask you to remember?"]);
    assert!(
        second.contains("7741"),
        "expected 7741 in answer, got:\n{second}"
    );
}

#[test]
#[ignore]
fn new_flag_starts_fresh_session() {
    let h = Harness::new();

    h.stdout(&["hello"]);
    h.stdout(&["--new", "hello again"]);

    let list = h.stdout(&["sessions", "list"]);
    let session_count = list.lines().skip(2).filter(|l| !l.trim().is_empty()).count();
    assert!(
        session_count >= 2,
        "expected >= 2 sessions, got:\n{list}"
    );
}

#[test]
#[ignore]
fn edit_file_modifies_content() {
    let h = Harness::new();
    let file = h.workspace.path().join("edit_target.txt");
    fs::write(&file, "the original line\nsome other content\n").unwrap();

    h.stdout(&[
        "Use the edit_file tool to edit edit_target.txt: \
         replace the exact string 'the original line' with 'the modified line'",
    ]);

    let content = fs::read_to_string(&file).unwrap();
    assert!(
        content.contains("the modified line"),
        "expected file to contain 'the modified line', got:\n{content}"
    );
    assert!(
        !content.contains("the original line"),
        "expected 'the original line' to be removed, got:\n{content}"
    );
}

#[test]
#[ignore]
fn shell_runs_command() {
    let h = Harness::new();

    let out = h.stdout(&["run the shell command: echo zanto-shell-test"]);
    assert!(
        out.contains("zanto-shell-test"),
        "expected 'zanto-shell-test' in output, got:\n{out}"
    );
}
