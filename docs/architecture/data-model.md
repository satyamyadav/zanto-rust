# Architecture — Data Model

Everything persisted: the SQLite schema, the settings layering, and the on-disk
layout. Current state.

## On-disk layout

```
~/.config/zanto/settings.json           — user-level settings (XDG_CONFIG_HOME)
~/.local/share/zanto/zanto.db           — sessions DB (XDG_DATA_HOME, via directories crate)

<workspace>/.zanto/settings.json        — project-level settings (auto-created)
<workspace>/                            — the directory zanto operates in
```

Per OS, the sessions DB resolves via `directories::ProjectDirs::from("", "", "zanto")`:

| OS | sessions DB |
|---|---|
| Linux | `~/.local/share/zanto/zanto.db` |
| macOS | `~/Library/Application Support/zanto/zanto.db` |
| Windows | `%APPDATA%\zanto\zanto.db` |

Override with the `ZANTO_DB` env var (used by e2e tests for isolation) — see
`Store::open()` in [session.rs](../../crates/zanto-core/src/session.rs).

## Settings — two layers, merged

In [config.rs](../../crates/zanto-core/src/config.rs), `Settings::load()`:

1. `ensure_project_config()` — if `.zanto/settings.json` is absent, create it with
   defaults.
2. Load user config (`~/.config/zanto/settings.json`) — may be absent.
3. Load project config (`.zanto/settings.json`).
4. `user.merge(project)` — **project overrides user**. For `allowed_paths` the
   lists are concatenated; bool flags OR together; `model`/`endpoint`/
   `max_context_turns` take the project value if set.
5. `resolve_paths()` — canonicalize every `allowed_paths` entry to absolute.

```jsonc
{
  "allowed_paths": ["/home/lazy/dev"],   // canonicalized at load
  "allow_read_outside": false,            // bypass read gate entirely if true
  "allow_write_outside": false,           // bypass write gate entirely if true
  "model": "gemini-flash-latest",         // optional; CLI -m overrides
  "endpoint": "http://192.168.1.66:11434/", // optional; CLI -e overrides; ignored for gemini
  "max_context_turns": 20                 // optional; → ContextPolicy::LastNTurns
}
```

`.zanto/` is gitignored — settings are machine-specific and never committed.
`AllowForever` writes back here via `Settings::persist_allowed_path()`.

## Sessions DB schema

Managed by `rusqlite_migration` (`migrations()` in
[session.rs](../../crates/zanto-core/src/session.rs)), applied with
`to_latest()` on open. WAL + foreign keys enabled via `pragma_update`.

```sql
-- migration 1
CREATE TABLE IF NOT EXISTS sessions (
    id          TEXT PRIMARY KEY,         -- "20260613T000000-a1b2c3d4"
    title       TEXT NOT NULL DEFAULT '',
    workspace   TEXT NOT NULL DEFAULT '', -- canonicalized CWD at creation
    created_at  INTEGER NOT NULL,         -- unix seconds
    updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id  TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,         -- 0-based order within the session
    role        TEXT NOT NULL,            -- user|assistant|system|tool
    content     TEXT NOT NULL,            -- serde_json of genai ChatMessage
    UNIQUE(session_id, position)
);
```

Notes:
- **`content` is a full JSON-serialized `genai::chat::ChatMessage`** — not plain
  text. This preserves tool-call and tool-response structure across reload.
- **`position`** is the message index; `UNIQUE(session_id, position)` + the
  `INSERT OR IGNORE` in `append_message` make appends idempotent (re-appending the
  same position is a no-op), which is what makes mid-loop persistence crash-safe.
- **`ON DELETE CASCADE`** — deleting a session drops its messages (FK enforced by
  the `foreign_keys=ON` pragma).
- The **system prompt is never stored** — it's injected fresh each turn in
  `chat()`, so changing the prompt doesn't require migrating history.

## Session id format

`new_id()`: `YYYYMMDDThhmmss` (from `unix_now`, pure stdlib date math) + `-` +
first 8 hex chars of a `uuid::Uuid::new_v4()`. Sortable lexicographically by
time; the uuid suffix avoids collisions within the same second. `find_by_prefix`
resolves a partial id (exact match wins; multiple non-exact matches →
`AmbiguousPrefix`).

## Context window — `effective_messages`

`Session::effective_messages(policy)` decides what actually goes to the model:

- `ContextPolicy::All` — every stored message (minus system).
- `ContextPolicy::LastNTurns { max_turns }` — `trim_to_turns`: filter out system
  messages, split into turns at each `User`-role boundary (a turn = one user
  message + the assistant/tool messages that follow), keep the last `max_turns`,
  flatten. Tool-call/tool-response pairs stay together because they're in the same
  turn. Default policy is 20 turns.

The full history stays in the DB; only what's *sent* is trimmed.

## What is NOT in the data model (current state)

- No tool-created data tables. Tools read/write the filesystem only; nothing
  writes app data rows. (The "data store tools" idea is parked, not built.)
- No memory/distillation tables.
- No per-message token counts or timestamps (only session-level `created_at` /
  `updated_at`).
