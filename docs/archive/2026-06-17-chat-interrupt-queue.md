# Chat interruption + message queuing

- **Date:** 2026-06-17
- **Decisions:** send-while-busy = **queue + Stop button**; on interrupt = **keep partial, mark Stopped**.

## Summary
Let the user (a) **stop** a running turn at any point and (b) **queue** follow-up
messages while a turn is in flight, dispatched FIFO as the thread frees up. Interruption
is backend-led (a per-turn cancel flag the core loop honors); queuing is frontend-led
(the backend already serializes one turn per session).

## Mechanism — interruption

### Cancel flag (no new core dep)
- `ChatConfig` gains `pub cancel: Option<Arc<AtomicBool>>` (`std::sync::atomic`). `::new` sets `None`.
- Core `chat()` checks it at safe points and, when set, stops and returns the **partial** turn.
- `DesktopState` gains `active_cancel: std::sync::Mutex<Option<Arc<AtomicBool>>>`.
- `send_message`: make a fresh `Arc::new(AtomicBool::new(false))`, store it in `active_cancel`,
  pass it as `config.cancel`. On completion, clear `active_cancel` (set to `None`).
- New command `interrupt_turn(state)`: `if let Some(f) = &*active_cancel { f.store(true, SeqCst) }`
  **and** `state.interactor.cancel_all()` (below). No session lock needed — callable mid-turn.

### Core check points (`chat.rs`)
Add `fn cancelled(c: &ChatConfig) -> bool` (`cancel.as_ref().map_or(false, |f| f.load(SeqCst))`). Check:
1. **Loop top** — before building each request → break.
2. **Inside the stream `while let Some(ev) = stream.next()`** — check each iteration; on cancel
   `break` (dropping `stream` aborts the genai/reqwest request), then break the outer loop.
3. **`route_tool_calls`** — before each dispatch (and before `flush_parallel`); on cancel stop dispatching.
On cancel: persist the partial assistant text (`push_msg(assistant(answer))` as today, even if
truncated/empty-with-blocks), and return `ChatTurn { blocks }` collected so far. Add a returned
signal that the turn was stopped — simplest: `ChatTurn` gains `pub stopped: bool` (default false;
update the few constructors + the CLI `.text()` is unaffected).

### HITL coupling (`interaction.rs`)
- `TauriInteractor::cancel_all(&self)`: drain `pending` and send `Value::Null` to each oneshot.
- A pending approval resolves to `Null` → `confirm` maps to `Deny`; a pending `ask` resolves to
  `Null` → tool returns empty. Either way the loop reaches a cancel check and bails. This unparks a
  turn that was waiting on the user.

### Stopped signal to the UI
- `send_message`, when `turn.stopped`, emits a `chat_stopped` event **before** `sink.finish()`'s
  `chat_done`. (Reuse the app handle; no sink change needed, or add `TauriSink::stopped()`.)
- Optionally persist `{stopped:true}` into the assistant message metadata (D1 path) so a reopened
  thread still shows the marker — **nice-to-have, not required for v1**.

## Mechanism — queuing (frontend)

### `session.svelte.ts`
- `sessionStore.queue: string[]` (+ `streaming`/`busy` unchanged).
- `send(text)`: if `busy`, `queue.push(text)` and return (do **not** invoke). Else run the turn as
  today; in the `finally` (after `busy=false`), if `queue.length`, `send(queue.shift())` to dispatch
  the next (FIFO; recursion is fine).
- `interrupt()`: `await ipc.interruptTurn()`. The running turn's promise then resolves (core returns
  the partial), `finally` runs, and the next queued message auto-dispatches.
- `removeQueued(i)`: splice it out.
- On `chat_stopped`: mark the live assistant entry as stopped (a `stopped` flag on the entry or a
  trailing `{kind:"stopped"}` segment) so `MessageList` shows the "Stopped" marker.
- `newSession`/app-switch while busy: call `interrupt()` + clear `queue` first.

### `Composer.svelte`
- While `busy`: the Send button becomes a **Stop** button (square icon) → `interrupt()`. The textarea
  stays **enabled** so Enter queues. Placeholder hints "Message queued — sent when the turn finishes"
  when there are queued items.
- Preserve the C2 paste-chip + C7 @-tag/slash behavior.

### `MessageList.svelte`
- Render `sessionStore.queue` as muted "pending" bubbles after the thread (each with a ✕ →
  `removeQueued`). Show the "Stopped" marker on a stopped assistant entry (small `⏹ Stopped` row,
  `text-muted-foreground`, using the Workbench tokens).

## Affected files
- `crates/zanto-core/src/chat.rs` — `ChatConfig.cancel`, `ChatTurn.stopped`, `cancelled()` + check points, partial return.
- `crates/zanto-desktop/src-tauri/src/ipc/mod.rs` — `DesktopState.active_cancel`.
- `crates/zanto-desktop/src-tauri/src/ipc/chat.rs` — manage cancel flag, pass to config, emit `chat_stopped`, `interrupt_turn` command.
- `crates/zanto-desktop/src-tauri/src/interaction.rs` — `cancel_all()` (+ optional `TauriSink::stopped`).
- `crates/zanto-desktop/src-tauri/src/lib.rs` — init `active_cancel`, register `interrupt_turn`.
- `crates/zanto-desktop/src/lib/ipc.ts` — `interruptTurn()` wrapper, `onChatStopped` listener.
- `crates/zanto-desktop/src/lib/stores/session.svelte.ts` — queue + interrupt + stopped handling.
- `crates/zanto-desktop/src/lib/components/Composer.svelte` — Stop button, keep input enabled.
- `crates/zanto-desktop/src/lib/components/MessageList.svelte` — pending bubbles + Stopped marker.

## Edge cases
- Interrupt with no active turn → no-op (active_cancel is None).
- Fresh `Arc` per turn so a prior cancel never kills the next turn; clear on finish.
- Interrupt during HITL → `cancel_all` resolves the oneshot → loop bails.
- chat_stopped then chat_done ordering preserved (same ordered event channel).
- Queue survives interruption (interrupt stops only the current turn); cleared on new session/app switch.
- Tool already mid-execution when cancel fires: it finishes (we check between calls, not mid-syscall) — acceptable; we bail before the next one.

## Acceptance (build-check only — headless)
- `cargo build` + `cargo test -p zanto-core` + `pnpm check` (0 errors) + `pnpm build:web`.
- Core test: `chat()` with a pre-set cancelled flag returns a `stopped` turn promptly **without**
  hitting the network (cancel seen at loop top) — deterministic, no model call.
- Manual (`pnpm dev`): Stop mid-stream truncates + marks Stopped; queueing two messages while busy
  dispatches them in order; Stop during an approval prompt unwinds cleanly.

## Out of scope (v1)
- Editing a queued message in place (remove + retype). 
- Per-message "stop & send now" interrupt (we chose queue + separate Stop).
- Persisting the Stopped marker across reload (optional metadata flag noted above).
- Backend-side queue / multi-session concurrency.
