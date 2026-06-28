# A3 — Artifact store + markdown-as-artifact

- **Date:** 2026-06-17
- **Wave:** A (core foundations), batch 2 (after A1)
- **Owner of:** new `crates/zanto-core/src/artifacts/`, edits `tools/mod.rs`, `chat.rs`

## Summary
A filesystem-backed **artifact store** for durable docs/assets (markdown, images,
json, text) the agent can produce and the user can browse (E4). Two scopes: project
(`.zanto/artifacts/`, when `project_dir` is set) and global
(`~/.local/share/zanto/artifacts/`). A JSON **manifest** indexes each root — no
SQLite migration (avoids colliding with A2). The engine is **ungated** (operates only
within managed roots, like `DataStore`); LLM tools wrap it.

> Naming: these are distinct from the desktop *catalogue* artifacts
> (`render_artifact` UI components). Store tools are named `store_artifact` /
> `list_stored_artifacts` / `read_stored_artifact` to avoid confusion.

## Affected files
- `crates/zanto-core/src/lib.rs` — `pub mod artifacts;`
- `crates/zanto-core/src/artifacts/mod.rs` — `ArtifactStore`, `ArtifactRef`, `ArtifactKind`, `Scope`.
- `crates/zanto-core/src/tools/artifacts/` — `store_artifact`, `list_stored_artifacts`, `read_stored_artifact`.
- `crates/zanto-core/src/tools/mod.rs` — register the `artifacts` tool category in
  `all_tools`/`dispatch`/`owns`/`is_readonly`.
- `crates/zanto-core/src/chat.rs` — construct `ArtifactStore` (from `project_dir`) and
  hand it to `ToolService`.

## Design

### Types
```rust
#[serde(rename_all="snake_case")] pub enum ArtifactKind { Markdown, Image, Json, Text }
#[serde(rename_all="snake_case")] pub enum Scope { Project, Global }

pub struct ArtifactRef {
    pub id: String,            // short uuid
    pub kind: ArtifactKind,
    pub title: String,
    pub rel_path: String,      // within the scope root
    pub scope: Scope,
    pub created_at: u64,
}
```

### Store
```rust
pub struct ArtifactStore { project_root: Option<PathBuf>, global_root: PathBuf }
impl ArtifactStore {
    pub fn new(project_dir: Option<&Path>) -> Self;   // global_root via ProjectDirs; honor $ZANTO_ARTIFACTS for tests
    pub fn save(&self, kind: ArtifactKind, title: &str, bytes: &[u8], scope: Scope) -> Result<ArtifactRef>;
    pub fn list(&self, scope: Option<Scope>) -> Result<Vec<ArtifactRef>>;
    pub fn read(&self, id: &str) -> Result<(ArtifactRef, Vec<u8>)>;
    pub fn path(&self, id: &str) -> Result<PathBuf>;
}
```
- Layout per root: `index.json` (`Vec<ArtifactRef>`) + files under `files/<id>.<ext>`
  (ext from kind: md/png/json/txt — image ext inferred from title or defaults png).
- `save` writes the file then upserts the manifest atomically (write tmp + rename).
- Project scope requires `project_root` set; else `Err`.

### Tools (ungated wrappers — operate only inside managed roots)
- `store_artifact { kind, title, content, scope? }` → `ArtifactRef` (markdown-as-artifact
  = `kind:"markdown"`). `content` is text for md/json/text; base64 for image.
- `list_stored_artifacts { scope? }` → `Vec<ArtifactRef>` (read-only).
- `read_stored_artifact { id }` → `{ ref, content }` (read-only; text decoded, image base64).
- Register: `owns` true for the three; `is_readonly` true for list/read.
  These do **not** call `permissions.check` (managed roots, like `DataStore`).

## Acceptance checks
- `cargo build` clean; existing tests pass.
- New tests (with `$ZANTO_ARTIFACTS` → tempdir, no `project_dir`):
  save markdown → `list(Global)` returns it → `read(id)` returns the bytes;
  manifest survives reopen; project scope without root → `Err`.

## Notes / handoff
- E4 (global artifact browser) lists/reads via desktop IPC over this store.
- AE/AF (agent uses md files as artifacts; browse/preview global) are satisfied by
  these tools + E4.
