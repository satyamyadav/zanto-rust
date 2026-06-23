import { test, expect } from "@playwright/test";

// R-6: monthly_summary renders inline as a component block; no tool-call card
// is shown for the summary.
//
// Flow:
//   1. Navigate to the app root (Chat is the active app by default).
//   2. Switch to the Finance app via the sidebar "Finance" button (the vertical-apps
//      section in Sidebar.svelte renders each non-chat app by a.name as its text).
//      switchTo() calls mountApp then newSession, so after the click the thread is
//      ready for a Finance-context conversation.
//   3. Send a message containing "finance summary" — the mock pickScenario routes
//      this to the finance-summary scenario, which emits:
//        chat_block  →  { kind:"component", component_id:"monthly_summary", data:{...}, target:"inline" }
//        chat_done
//   4. Assert the monthly_summary component rendered inline:
//      - The month heading "June 2026" is visible (data.month, text-sm heading).
//      - The category row "Dining" is visible (by_category[0].category).
//   5. Assert NO tool-call card for the summary: no element with the text
//      "monthly_summary" exists in the page (the component_id would appear as the
//      tool name on a tool-call card if one were rendered).
test("R-6: monthly_summary renders inline as a block, no tool-call card", async ({ page }) => {
  await page.goto("/");

  // 1. Switch to the Finance app.
  //    Sidebar renders vertical apps (id !== "chat") with their a.name as button text.
  //    The mock list_apps fixture returns { id:"finance", name:"Finance" } (Task 1).
  const financeBtn = page.getByRole("button", { name: "Finance" });
  await expect(financeBtn).toBeVisible();
  await financeBtn.click();

  // Wait for the switch to complete: "switching" state goes false and the session
  // list reloads. The sidebar disables the button while switching, so wait for it
  // to become enabled again before proceeding.
  await expect(financeBtn).toBeEnabled();

  // 2. Send the finance summary message.
  const composer = page.getByRole("textbox").first();
  await composer.fill("finance summary for this month");
  await composer.press("Enter");

  // 3. Assert the monthly_summary component rendered inline.
  //    monthly_summary.svelte renders data.month in a <div class="text-sm font-medium">.
  await expect(page.getByText("June 2026")).toBeVisible();
  //    by_category rows render each c.category in a <span>; "Dining" is our canned entry.
  await expect(page.getByText("Dining")).toBeVisible();

  // 4. Assert NO tool-call card for the summary.
  //    A tool-call card would display the tool/component name "monthly_summary".
  //    The scenario emits only chat_block (no chat_tool_call), so no card should exist.
  //    Scope the assertion to the chat area (not the sidebar / script text) by checking
  //    the count across the whole page — "monthly_summary" must not appear as visible text.
  await expect(page.getByText("monthly_summary")).toHaveCount(0);
});

// #11 + #13: canvas panel scrolls vertically; finance tablist scrolls horizontally.
//
// After switching to Finance the canvas panel hosts the Dashboard. Assert computed
// overflow styles on the two containers without needing real overflow (deterministic).
test("#11/#13: canvas vertical scroll + finance tablist horizontal scroll", async ({ page }) => {
  await page.goto("/");

  // Switch to Finance so the canvas panel renders <Dashboard />.
  const financeBtn = page.getByRole("button", { name: "Finance" });
  await expect(financeBtn).toBeVisible();
  await financeBtn.click();
  await expect(financeBtn).toBeEnabled();

  // #11: the canvas scroll container wraps <Dashboard /> inside Canvas.svelte,
  //      tagged data-testid="canvas-scroll" so this targets exactly that element.
  const canvasScroll = page.locator('[data-testid="canvas-scroll"]');
  const canvasOverflowY = await canvasScroll.evaluate(
    (el) => getComputedStyle(el).overflowY,
  );
  expect(["auto", "scroll"]).toContain(canvasOverflowY);

  // #13: the finance tab bar uses role="tablist". Assert its overflowX.
  const tablist = page.locator("[role=tablist]");
  const tabsOverflowX = await tablist.evaluate(
    (el) => getComputedStyle(el).overflowX,
  );
  expect(["auto", "scroll"]).toContain(tabsOverflowX);
});
