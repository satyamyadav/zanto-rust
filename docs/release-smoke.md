# v1.0.0 Release Smoke Runbook

This is the manual gate before tagging v1.0.0. It covers only what automated suites
cannot: live model calls, OS-level window behaviour, keychain/env auth, and
end-to-end flows.

**What this doc does NOT cover:**
- `cargo test` / `pnpm test:ui` / `pnpm check` — run these in CI; they must be green.
- Building the release artifacts — that is the maintainer's job via the release
  workflow.
- CI tagging — only tag after this checklist is clean.

**How to read this:** each item gives **Precondition · Steps · Expected**.
IDs in parentheses cross-reference `docs/zanto-test-checklist.csv`.

---

## 1. Setup

### S2 — Configure provider and API key

**Precondition:** Fresh install or Settings cleared. App launched.

**Steps:**
1. Open **Settings → Provider**.
2. Select a cloud provider (e.g. Anthropic/Claude) and a model.
3. Enter the API key → Save (it stores in the OS keychain, or reads the env var if
   set before launch).
4. In Settings the key field shows the saved value (or "Saved" if from env).

**Expected:** Settings reflects the chosen provider; the next turn uses that model.
No "missing key" error on send.

---

## 2. Core live-model

### C-1 — Streaming turn against a real provider

**Precondition:** S2 complete.

**Steps:**
1. Open Chat, start a new session.
2. Send: `"Briefly explain what zanto is."`
3. Watch the reply area.

**Expected:** Tokens stream in progressively (no single-shot flash). Reply is
coherent. Turn completes without error.

### C-10 — Error card + Retry recovery

**Precondition:** S2 complete.

**Steps:**
1. Open **Settings → Provider**, change the API base URL to
   `https://invalid.example.com` → Save.
2. Send any message.
3. Observe the error state. Click **Retry**.
4. Restore the correct base URL in Settings → Save.
5. Click **Retry** again (or send a new message).

**Expected:**
- Step 2: an inline error card appears (not a blank or frozen chat).
- Step 5: the retry succeeds; tokens stream normally.

---

## 3. Fixed-retests (live model)

### F-2 — Finance: add transaction via chat + "this month summary"

**Precondition:** Finance app open. At least one account exists (see FS-2).
Real provider active (S2).

**Steps:**
1. Switch to **Finance**.
2. In the Finance chat, send: `"Add an expense: $12.50 at a cafe, category dining, today."`
3. After the turn completes, send: `"this month summary"`.

**Expected:**
- The transaction is stored as a number (not 0). Checklist note: amounts phrased
  as `"$12.50"` are coerced correctly since the fix on 2026-06-18.
- `this month summary` renders inline (no tool-call card visible). KPIs and chart
  include the new transaction.
- Close and reopen the Finance session; the transaction persists.

### CO-1 — Skill persists across restart

**Precondition:** A skill file exists at `<project>/.zanto/skills/<name>.md`
(create one if needed; any content that steers replies, e.g. "Always respond in
bullet points"). Real provider active.

**Steps:**
1. Open **Settings → Skills**, select the skill.
2. Send a test message in Chat; confirm the reply follows the skill's style.
3. Fully quit the app (not just close the window).
4. Relaunch.
5. Open Settings → Skills — the skill is still selected.
6. Send the same test message again.

**Expected:**
- Step 2: reply is steered by the skill.
- Step 5: skill selection survived the restart (stored in `selected_skill`,
  seeded on startup since fix 2026-06-18).
- Step 6: reply is still skill-steered without re-selecting.

---

## 4. OS / Window

### W-1 — Window state persistence

**Precondition:** App running.

**Steps:**
1. Resize the window to a non-default size (e.g. make it noticeably narrower).
2. Move it to a different corner of the screen.
3. Quit the app.
4. Relaunch.

**Expected:** Window reopens at the same size and position.

### W-2 — Single instance

**Precondition:** App already running.

**Steps:**
1. Launch a second instance of the app (double-click the icon or run the binary again).

**Expected:** The existing window is focused. No second window opens.

### W-3 — Turn-done notification (unfocused)

**Precondition:** S2 complete. OS notifications allowed for zanto.

**Steps:**
1. Start a message that will take a few seconds (e.g. ask for a detailed list).
2. While the turn is in progress, switch focus to another app (e.g. your browser).
3. Wait for the turn to finish.

**Expected:** A native OS notification "Reply ready" (or equivalent) appears.
No notification fires when the window is already focused.

### W-4 — Approval notification (unfocused)

**Precondition:** S2 complete. OS notifications allowed. A folder access grant
is NOT set for the path you will use.

**Steps:**
1. Switch focus away from zanto.
2. From Chat, send a prompt that triggers a file-read outside granted folders
   (e.g. `"Read /tmp/test-approval.txt"` after creating that file).

**Expected:** A native OS notification "zanto needs your input" fires while the
app is unfocused.

---

## 5. Vision / Web / Docs

### DOC-4 / FLOW-3 — Vision turn (multimodal provider)

**Precondition:** Provider is a multimodal model (Anthropic Claude / Gemini).
An image file is available locally.

**Steps:**
1. In Chat, attach an image (drag-drop or paperclip button).
2. Send: `"What is in this image?"`

**Expected:** The model describes the image content accurately.
If using Ollama (non-multimodal), expect a graceful degradation note — do NOT
mark this as a failure for Ollama.

### CO-3 — Web fetch of a public URL

**Precondition:** S2 complete. Internet connected.

**Steps:**
1. Send: `"Fetch https://example.com and tell me what it says."`

**Expected:** The agent calls `fetch_url`, returns extracted text from the page,
and summarizes it. No crash or empty response.

### DOC-2 — Attach a document and summarize it

**Precondition:** A PDF, DOCX, or text file is available locally.

**Steps:**
1. Attach the file (drag-drop or paperclip button). A chip appears in the composer.
2. Send: `"Summarize it."`

**Expected:** The agent calls `read_document` and answers from the file's content.
The summary is relevant and not a generic refusal.

---

## 6. Human-in-the-Loop (HITL)

### H-1 — Permission approval overlay

**Precondition:** No folder access grant for `/tmp` (or the target path).

**Steps:**
1. Send: `"Read the file /tmp/hitl-test.txt"` (create it first: `echo hello > /tmp/hitl-test.txt`).
2. The approval overlay appears above the composer.
3. Test each response mode in separate turns:
   - **Allow once** — approves this request only; next read asks again.
   - **Allow for session** — approves all reads in this session.
   - **Allow forever** — persists in settings.
   - **Deny** — the agent gets a denial; it stops or asks differently.

**Expected:** Overlay appears above the composer (not a modal). Each response
mode behaves as described. After "deny", the agent does not retry silently.

### H-2 — Agent ask-form round-trip

**Precondition:** S2 complete. A skill or scenario that triggers a clarifying
question (or use a prompt like `"Use the ask tool to get my name, then greet me"`
if the model supports it).

**Steps:**
1. Trigger an agent clarifying question.
2. The multi-field form appears above the composer.
3. Fill in the fields and submit.

**Expected:** The form renders correctly. Submitted answers return to the agent
and it continues the turn using them.

---

## 7. Finance — Advanced smoke (best-effort, render/no-crash)

> Exhaustive value-checking is NOT required here — logic is covered by Rust unit
> tests. This section is a render/no-crash smoke.

### FS-1 — Import sample statement

**Precondition:** `docs/sample-statement.csv` present in the repo root.

**Steps:**
1. Finance → **Import** tab.
2. Enter (or Browse to) `docs/sample-statement.csv`.
3. Map columns: `Date → Date`, `Description → Merchant`, `Debit → Debit`,
   `Credit → Credit`, `Category → Category`.
4. Set account = **Checking**.
5. Click **Import** (approve the file-read permission if prompted).

**Expected:** 33 rows imported (4 income credits, 29 expense debits). Seeds
March–June 2026 data including recurring entries (Netflix, dining, etc.).

### FS-2 — Create accounts

**Precondition:** Finance app open.

**Steps:**
1. Finance → **Edit → Accounts**.
2. Add:
   - **Checking** (opening balance 0)
   - **Savings** (opening balance 2500)
   - **Card** (opening balance −800)
3. Save.

**Expected:** Accounts tab populates with per-account cards. Net worth headline
is visible.

### FS-3 — Set budgets and goals

**Precondition:** FS-1 and FS-2 complete.

**Steps:**
1. Finance → **Edit → Monthly budgets**: set Dining = 200, Groceries = 250 → Save.
2. Finance → **Edit → Goals**:
   - "Emergency" — type: savings, account: Savings, target: 10000 → Save.
   - "Pay off card" — type: debt, account: Card, target: 1000 → Save.

**Expected:** Budget bars appear on the dashboard. Goals tab shows both goals
with progress indicators.

### Finance surface smoke

With FS-1, FS-2, FS-3 complete, verify each surface renders without a JS error
or blank panel:

| Surface | How to open | Pass criterion |
|---|---|---|
| **Dashboard KPIs** | Finance → Dashboard | Balance / This month / Income / Net KPI cards visible |
| **Budgets bars** | Dashboard → budget section | Dining and Groceries bars rendered (one may be red if over) |
| **Subscriptions** | Finance → Subscriptions tab | Tab loads; at least one recurring entry visible (Netflix from seed data) |
| **Trends** | Finance → Trends tab | 6-month per-category line chart renders |
| **Accounts / net worth** | Finance → Accounts tab | Per-account cards and net worth figure visible |
| **Goals** | Finance → Goals tab | Both goals show with progress |
| **Forecast** | Dashboard → Forecast card | Projected end-of-month net worth and expected in/out visible |

---

## 8. FLOW smoke (happy-path walkthroughs)

Run each flow; mark **pass** or note a defect. A defect in FLOW-1…8 is not
automatically a release blocker — see Triage rule below.

### FLOW-1 — Doc Q&A → visualize → keep

**Steps:**
1. Drop a spreadsheet (CSV or XLSX) onto the Chat window.
2. Ask: `"Summarize the data."`
3. Ask: `"Chart the totals as a bar chart."`
4. Pin the chart.
5. Close and reopen the session.

**Expected:** Each step works end-to-end: `read_document` called, chart renders
inline, pin affordance visible and clickable, chart re-renders on reopen from
Artifacts → Pinned views.

### FLOW-2 — Generate and download a report

**Steps:**
1. Send: `"Write a short markdown report on the state of AI assistants in 2026 and save it."`
2. In Artifacts → the new document → click **Save a copy**.
3. Click **Reveal** in the file manager.

**Expected:** Markdown document artifact created; Save dialog writes to disk;
Reveal opens the file manager at the saved file location.

### FLOW-3 — Vision (multimodal)

**Steps:**
1. Attach a screenshot to the chat.
2. Send: `"Describe this image and list any visible problems."`

**Expected:** Model gives a useful description. (Requires multimodal provider;
skip or mark N/A for Ollama.)

### FLOW-4 — Filesystem assistant

**Steps:**
1. Send: `"List ~/Downloads as a table, then delete any .tmp files you find."`

**Expected:** `list_directory` → table artifact. Delete triggers the approval
overlay (H-1); approving deletes the files; denying stops with confirmation.

### FLOW-5 — Interrupt and queue

**Steps:**
1. Send a long, expensive prompt.
2. While streaming, click **Stop**.
3. Immediately send two follow-up messages.

**Expected:** Stop works; partial reply kept with "Stopped" marker. Both queued
messages run in FIFO order after stop.

### FLOW-6 — Workspace context

**Steps:**
1. Sidebar → **Workspace → Set project** to any local directory.
2. Add a notes file as a context source; toggle enable on.
3. Ask something answerable only from that file.
4. Toggle the source off; ask the same question.

**Expected:** Answer uses the source when on; ignores it (generic answer) when
off.

### FLOW-7 — Finance zero-to-dashboard

**Steps:**
1. Switch to Finance (start fresh or use existing FS-1/FS-2/FS-3 data).
2. Add a transaction via chat.
3. Ask: `"this month summary"`.
4. Dashboard → Edit → add a widget.

**Expected:** Full loop works: chat adds transaction, summary renders inline,
widget appears on dashboard.

### FLOW-8 — Skill-steered code review

**Steps:**
1. Create `<project>/.zanto/skills/reviewer.md` with content:
   ```
   You are a code reviewer. Always respond with: a one-line summary, then
   a numbered list of issues, then a verdict (approve / request-changes).
   ```
2. Settings → Skills → select `reviewer`.
3. Paste a short code snippet and ask it to review it.

**Expected:** Reply follows the skill's exact format: one-liner, numbered issues,
verdict. Skill does not need to be re-selected after restart (CO-1 already
verified this).

---

## Triage rule

**Core path break (C-*, F-*, CO-*, H-*, W-*, DOC-*, S2) → release blocker.**
Do not tag until fixed.

**Advanced Finance path break (FS-*, FV-*, FB-*, FI-*, FA-*, FG-*) → NOT a
blocker.** Add an entry to `known_issues.md` describing the surface, the symptom,
and any workaround, then continue the release.

**FLOW smoke break → judgement call.** If the underlying capability (chat,
tools, artifacts) is fine and only the end-to-end convenience is broken, add a
`known_issues.md` entry. If a FLOW failure points to a regression in a core
primitive, treat it as a core break.
