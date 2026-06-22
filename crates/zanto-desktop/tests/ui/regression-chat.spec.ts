import { test, expect } from "@playwright/test";

// R-3: Empty-stop 'Stopped' marker shows live AND survives reopening the session.
//
// Flow:
//   1. Send a message containing "silent stop" — the mock emits NO chat_chunk events
//      (events: []), so the turn streams with no text. The session stays "busy"
//      until interrupt_turn flips the interrupted flag and send_message fires
//      chat_stopped + chat_done.
//   2. While the turn is running, click the Stop button (aria-label="Stop").
//   3. Assert the 'Stopped' marker is visible in the chat.
//   4. Create a new session (so we can navigate away), then click "Test session"
//      in the sidebar — the mock load_session fixture always returns an assistant
//      turn with stopped: true, text: "" — to simulate reopening the session.
//   5. Assert the 'Stopped' marker is still visible.
test("R-3: empty-stop 'Stopped' marker shows live and survives reopen", async ({ page }) => {
  await page.goto("/");

  // 1. Send a message that triggers the silent-stop scenario.
  const composer = page.getByRole("textbox").first();
  await composer.fill("silent stop please");
  await composer.press("Enter");

  // 2. The Stop button appears while sessionStore.busy is true (SquareIcon button).
  //    With no events to stream, the turn stays open; click Stop immediately.
  const stopBtn = page.getByRole("button", { name: "Stop" });
  await expect(stopBtn).toBeVisible();
  await stopBtn.click();

  // 3. After interrupt_turn, the mock fires chat_stopped + chat_done, setting
  //    the last convo entry's stopped flag → the Stopped marker appears.
  await expect(page.getByText("Stopped")).toBeVisible();

  // 4. Open a new session so we can navigate away from the current one.
  //    Click the "New chat" (PlusIcon) button in the sidebar.
  await page.getByRole("button", { name: "New chat" }).click();
  // Confirm the new-chat view is clean — Stopped marker must be gone before we
  // reopen, so the subsequent assertion reflects the reloaded session, not lingering DOM.
  await expect(page.getByText("Stopped")).toHaveCount(0);
  // The sidebar lists "Test session" from the list_sessions fixture.

  // 5. Reopen the session via the sidebar. The mock load_session always returns
  //    the fixture with the stopped assistant turn.
  await page.getByText("Test session").click();

  // 6. Assert the Stopped marker persists after reload.
  await expect(page.getByText("Stopped")).toBeVisible();
});

// R-8: /clear is deterministic — clears with content, no-op when empty, never deadlocks.
//
// How the slash menu works (from Composer.svelte):
//   - Typing `/` at the start of a line (or on an empty composer) opens a listbox.
//   - The menu shows /new and /clear as button[role="option"] items.
//   - Selecting /clear strips the `/<query>` fragment and calls clearInput().
//   - /clear is always listed regardless of whether the composer is empty (the
//     comment in Composer.svelte explains that gating it caused a deadlock).
//   - Selection: click the `/clear` button in the menu, OR navigate with arrow
//     keys then press Enter.
//
// Test steps:
//   A. Fill the composer with some text, then type `/` to trigger the slash menu,
//      navigate to /clear with ArrowDown, confirm with Enter → composer is empty.
//   B. On an already-empty composer, type `/` + navigate to /clear + Enter →
//      composer stays empty but remains responsive (can still type afterward).
test("R-8: /clear is deterministic — clears with content, no-op when empty, never deadlocks", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();

  // ── Part A: /clear with content ──────────────────────────────────────────

  // Fill the composer then move to the end and type `/` to open the slash menu.
  // Simpler: clear and just start with `/` on the first (and only) line.
  await composer.fill("some text");

  // The slash menu triggers on `/` at the start of a line. To keep "some text"
  // in the composer we use a multi-line approach: place the caret at the end,
  // press Shift+Enter for a newline, then type `/` to trigger the menu on line 2.
  // After /clear runs it strips the `/<query>` fragment from line 2 AND calls
  // clearInput() which empties the whole value (input = "", pastes = [], attachments = []).
  await composer.press("Shift+Enter");
  await composer.type("/");

  // Slash menu should be visible now.
  const slashMenu = page.getByRole("listbox");
  await expect(slashMenu).toBeVisible();

  // Click the /clear option directly (most robust — avoids arrow-key ordering concerns).
  await slashMenu.getByRole("option", { name: /\/clear/ }).click();

  // clearInput() sets input = "" → composer value becomes empty.
  await expect(composer).toHaveValue("");

  // ── Part B: /clear on an empty composer (no-op, no deadlock) ─────────────

  // Composer is already empty; type `/` at the start of the line to open the menu.
  await composer.fill("");
  await composer.type("/");
  await expect(slashMenu).toBeVisible();

  // The /clear option must still appear (it is always listed — no deadlock).
  const clearOption = slashMenu.getByRole("option", { name: /\/clear/ });
  await expect(clearOption).toBeVisible();
  await clearOption.click();

  // Composer remains empty and fully responsive.
  await expect(composer).toHaveValue("");

  // Verify responsiveness: typing still works after a /clear on an empty composer.
  await composer.type("still works");
  await expect(composer).toHaveValue("still works");
});
