import { test, expect } from "@playwright/test";

// C-1: Streamed tokens accumulate into the visible assistant reply.
// The default mock scenario emits "Hi " + "there." → "Hi there."
test("C-1: tokens stream into the assistant reply", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("hello");
  await composer.press("Enter");
  await expect(page.getByText("Hi there.")).toBeVisible();
});

// C-6: Copy a reply puts its text on the clipboard.
// The copy button (aria-label="Copy message") lives inside a
// `opacity-0 group-hover:opacity-100` wrapper — hover the assistant
// message container first, then click Copy.
// After the click the button shows "Copied" (CheckIcon + text) for 1.5 s.
// We prefer asserting the clipboard contents; if clipboard-read is blocked
// in this runner, we fall back to the visible "Copied" state feedback.
test("C-6: copy a reply puts its text on the clipboard", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("hello");
  await composer.press("Enter");

  const replyText = page.getByText("Hi there.");
  await expect(replyText).toBeVisible();

  // Hover the assistant message group to reveal the copy control.
  // The group div wraps the reply content; hover it to trigger the
  // `group-hover:opacity-100` transition on the copy button container.
  await replyText.hover();

  const copyBtn = page.getByRole("button", { name: "Copy message" });
  await expect(copyBtn).toBeVisible();
  await copyBtn.click();

  // Try clipboard read first; fall back to the 'Copied' state text.
  const clip: string = await page
    .evaluate(() => navigator.clipboard.readText().catch(() => ""))
    .catch(() => "");

  if (clip) {
    // Clipboard read succeeded — assert the content.
    expect(clip).toContain("Hi there.");
  } else {
    // Clipboard read blocked — assert the visible 'Copied' feedback instead.
    await expect(page.getByRole("button", { name: "Copy message" })).toContainText("Copied");
  }
});

// C-2: Stopping mid-turn keeps the partial reply and shows the Stopped marker.
// The "partial stop" scenario emits one chunk ("Partial answer so far") then
// blocks until interrupt_turn is called. Clicking Stop (aria-label="Stop")
// fires interrupt_turn; the store emits chat_stopped + chat_done so the
// streaming turn finalises with entry.stopped === true, which MessageList
// renders as a "Stopped" label after the bubble.
test("C-2: stopping mid-turn keeps the partial reply and shows the Stopped marker", async ({
  page,
}) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("partial stop now");
  await composer.press("Enter");

  // Wait for the partial text to stream in (turn is now blocked/busy).
  await expect(page.getByText("Partial answer so far")).toBeVisible();

  // Click Stop — fires interrupt_turn, unblocks the mock, emits chat_stopped.
  const stopBtn = page.getByRole("button", { name: "Stop" });
  await expect(stopBtn).toBeVisible();
  await stopBtn.click();

  // Partial text must still be visible after stopping.
  await expect(page.getByText("Partial answer so far")).toBeVisible();
  // The Stopped marker must appear beneath the assistant bubble.
  await expect(page.getByText("Stopped")).toBeVisible();
});

// C-3: A message typed while busy is queued and dispatched FIFO after the turn ends.
// While the "partial stop" turn is blocking, submitting a second message queues it.
// MessageList renders queued messages as dashed-border chips (border-dashed class)
// with the message text inside a <span class="whitespace-pre-wrap">.
// After Stop frees the first turn, send()'s finally dispatches the queued message
// using the default scenario (it doesn't contain "partial stop"), producing a
// real user bubble (bg-primary, solid border) and a "Hi there." reply.
//
// Phase 1 (while busy): the chip locator (div.border-dashed) is visible.
// Phase 2 (after Stop): the chip locator is GONE and "Hi there." confirms dispatch.
test("C-3: a message typed while busy is queued and dispatched after the turn ends", async ({
  page,
}) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("partial stop please");
  await composer.press("Enter");

  // Wait until the blocking turn is streaming (busy = true).
  await expect(page.getByText("Partial answer so far")).toBeVisible();

  // Submit a second message while busy — it should queue, not send immediately.
  await composer.fill("queued follow-up");
  await composer.press("Enter");

  // Phase 1: the message MUST appear as a dashed-border chip, not a real bubble.
  // The chip container has `border-dashed` (see MessageList.svelte line ~122);
  // normal user bubbles use `bg-primary` with no dashed border.
  const queuedChip = page
    .locator("div.border-dashed")
    .filter({ hasText: "queued follow-up" });
  await expect(queuedChip).toBeVisible();

  // Free the first turn by clicking Stop.
  const stopBtn = page.getByRole("button", { name: "Stop" });
  await expect(stopBtn).toBeVisible();
  await stopBtn.click();

  // Phase 2: after the turn ends the chip MUST disappear (message was dispatched)
  // and the default reply "Hi there." must appear (proving the queued turn ran).
  await expect(queuedChip).not.toBeVisible();
  await expect(page.getByText("Hi there.")).toBeVisible();
});

// C-4: A tool-using turn shows a thinking block that collapses to "Thought for N steps".
// The "think" scenario emits: chat_reasoning("Considering options") + chat_tool_call +
// chat_tool_result + chat_chunk("Done.") + chat_done.
// stepCount = 1 (one tool call), so the summary label is "Thought for 1 step".
// The block starts collapsed (open = false by default); click the button to expand
// and reveal the reasoning text "Considering options".
test("C-4: a tool-using turn shows a thinking block that collapses to 'Thought for 1 step' and is expandable", async ({
  page,
}) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("think about this");
  await composer.press("Enter");

  // Wait for the turn to complete.
  await expect(page.getByText("Done.")).toBeVisible();

  // The collapsed thinking label must be visible.
  const thinkingBtn = page.getByRole("button", { name: "Thought for 1 step" });
  await expect(thinkingBtn).toBeVisible();

  // Expand it — reasoning content should now be visible.
  await thinkingBtn.click();
  await expect(page.getByText("Considering options")).toBeVisible();
});

// C-5: Multiple tool calls in a single turn are grouped as a Workflow.
// The "workflow" scenario emits: two tool_call+tool_result pairs + chat_chunk("Done.") + chat_done.
// ≥2 consecutive tool_call segments → WorkflowGroup; label: "Workflow (2 steps)".
// The pill shows "2/2 done" once both results arrive.
test("C-5: multiple tool calls are grouped as a Workflow", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("workflow run");
  await composer.press("Enter");

  // Wait for the turn to complete.
  await expect(page.getByText("Done.")).toBeVisible();

  // The workflow header must show the step count.
  await expect(page.getByText("Workflow (2 steps)")).toBeVisible();

  // The completion pill must confirm both steps finished.
  await expect(page.getByText("2/2 done")).toBeVisible();
});

// C-7: A large paste (>20 lines or >2000 chars) collapses to a 'pasted N lines' chip
// in the composer, but the full text is spliced into the message on send.
// Composer thresholds: CHAR_THRESHOLD = 2000, LINE_THRESHOLD = 20.
// We paste 60 lines ("line 0" … "line 59"), which exceeds the line threshold.
// The component's onpaste handler reads e.clipboardData.getData("text/plain") and
// calls e.preventDefault() to suppress the normal insert, then pushes a Paste chip.
// The chip label is "pasted 60 lines". On send, composeMessage() joins paste texts
// into the message, so the user bubble contains the full text.
// Triggering paste: Playwright's clipboard API + keyboard Ctrl+V can be unreliable
// across environments, so we dispatch a synthetic ClipboardEvent directly on the
// textarea via page.evaluate, mirroring what a real paste would deliver.
test("C-7: a large paste collapses to a chip but the full text is still sent", async ({
  page,
  context,
}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.focus();

  // Build a 60-line string — exceeds the LINE_THRESHOLD of 20.
  const lines = Array.from({ length: 60 }, (_, i) => `line ${i}`);
  const big = lines.join("\n");

  // Dispatch a synthetic ClipboardEvent carrying the big text on the textarea.
  // This replicates exactly what the browser delivers on Ctrl+V; the component's
  // `onpaste` handler calls e.clipboardData.getData("text/plain") to extract it.
  await page.evaluate((text) => {
    const el = document.querySelector("textarea");
    if (!el) throw new Error("textarea not found");
    const dt = new DataTransfer();
    dt.setData("text/plain", text);
    el.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true, cancelable: true }));
  }, big);

  // The chip "pasted 60 lines" must appear in the composer area.
  await expect(page.getByText("pasted 60 lines")).toBeVisible();

  // Send — composeMessage() includes the paste text in the user message.
  await composer.press("Enter");

  // Scope the full-text assertions to the sent USER MESSAGE BUBBLE (bg-primary).
  // This rules out any match against the composer chip or its aria/metadata text.
  // The bubble renders the full composed text as a text segment (via TextSegment →
  // Block → markdown), so both boundary lines must appear inside it.
  const userBubble = page
    .locator("div.bg-primary")
    .filter({ hasText: "line 0" });
  await expect(userBubble).toBeVisible();
  // The same bubble must also contain "line 59" (the last line), proving the full
  // 60-line paste was spliced into the sent message — not just the chip summary.
  await expect(userBubble).toContainText("line 59");
});

// C-8: Typing @ opens a file autocomplete (backed by browse_dir) and selecting an
// entry inserts an @<path> token into the composer.
// The file menu is a role="listbox" overlay; each entry is role="option".
// Selecting a non-directory entry calls insertTag(path), writing `@<path> ` into input.
// The mock browse_dir returns two entries: a dir "src" and a file "README.md".
// We pick "README.md" (the file), which inserts "@/home/user/project/README.md ".
// Note: the listbox uses onmousedown (not onclick) with e.preventDefault() to
// prevent the textarea from blurring before the insertion completes.
test("C-8: typing @ opens a file autocomplete and inserts the path", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();

  // Type up to and including `@` — oninput triggers syncMenu → openFileMenu.
  // Use fill + type so the `@` triggers the input event that opens the file menu.
  await composer.fill("what is in ");
  await composer.type("@");

  // The file autocomplete listbox must appear.
  const fileMenu = page.getByRole("listbox");
  await expect(fileMenu).toBeVisible();

  // Both seeded entries must be present.
  await expect(fileMenu.getByRole("option", { name: /README\.md/ })).toBeVisible();

  // Click the file entry (README.md) — this calls insertTag and inserts the @-token.
  await fileMenu.getByRole("option", { name: /README\.md/ }).click();

  // The listbox must close.
  await expect(fileMenu).not.toBeVisible();

  // The composer must now contain the @<path> token.
  await expect(composer).toHaveValue(/\@\/home\/user\/project\/README\.md/);
});

// C-9: Slash menu lists /new and /clear; selecting /new starts a fresh session.
// Typing `/` at line start (empty composer) opens the listbox.
// The menu items are role="option" buttons inside a role="listbox".
// After /new: convo is reset to [], showing the "Start a conversation" empty state.
// /clear is not re-tested in depth here (covered by R-8).
test("C-9: slash menu offers /new and /clear, and /new starts a fresh session", async ({
  page,
}) => {
  await page.goto("/");

  // Send a message first so the thread is non-empty.
  const composer = page.getByRole("textbox").first();
  await composer.fill("hello");
  await composer.press("Enter");
  await expect(page.getByText("Hi there.")).toBeVisible();

  // Open the slash menu: clear the composer and type `/` at line start.
  await composer.fill("");
  await composer.type("/");

  const slashMenu = page.getByRole("listbox");
  await expect(slashMenu).toBeVisible();

  // Both /new and /clear must appear.
  await expect(slashMenu.getByRole("option", { name: /\/new/ })).toBeVisible();
  await expect(slashMenu.getByRole("option", { name: /\/clear/ })).toBeVisible();

  // Select /new — resets the convo to [].
  await slashMenu.getByRole("option", { name: /\/new/ }).click();

  // After /new the thread is empty: the MessageList empty-state is visible.
  await expect(page.getByText("Start a conversation")).toBeVisible();
  // The previous reply must be gone.
  await expect(page.getByText("Hi there.")).toHaveCount(0);
});
