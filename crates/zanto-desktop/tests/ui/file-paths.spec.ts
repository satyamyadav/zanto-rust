import { test, expect } from "@playwright/test";

// C-14: Backticked absolute paths in assistant output render as openable file links.
//
// The "file path test" scenario emits an assistant message containing:
//   - `/home/user/project/src/main.rs` — backticked absolute path → MUST be linkified
//   - `src/relative.rs`               — backticked relative path  → MUST NOT be linkified
//   - /bare/prose/path                 — bare prose slash-string   → MUST NOT be linkified
//
// The mock get_config returns project_dir: null, so the absolute path is shown
// as-is (no relative shortening). The link element is an <a data-file-path="…">.
// Clicking it calls ipc.openPath (open_path mock: no-op) — no navigation occurs.
// C-12 (http link panel) must remain unaffected.

test("C-14: backticked absolute path renders as a file-path link", async ({ page }) => {
  await page.goto("/");
  const urlBefore = page.url();

  const composer = page.getByRole("textbox").first();
  await composer.fill("file path test");
  await composer.press("Enter");

  // Wait for the assistant reply to appear.
  await expect(page.getByText(/See.*for details/)).toBeVisible();

  // The absolute path must be rendered as an anchor with data-file-path.
  const fileLink = page.locator('a[data-file-path="/home/user/project/src/main.rs"]');
  await expect(fileLink).toBeVisible();

  // The link text is the full path (project_dir is null in the fixture → no shortening).
  await expect(fileLink).toHaveText("/home/user/project/src/main.rs");

  // Clicking the link must NOT navigate the page (open_path is a mock no-op).
  await fileLink.click();
  expect(page.url()).toBe(urlBefore);
});

test("C-14b: backticked relative path is NOT linkified", async ({ page }) => {
  await page.goto("/");

  const composer = page.getByRole("textbox").first();
  await composer.fill("file path test");
  await composer.press("Enter");

  await expect(page.getByText(/Also.*and/)).toBeVisible();

  // The relative backtick path must NOT have data-file-path (no link element).
  const relativeLink = page.locator('a[data-file-path="src/relative.rs"]');
  await expect(relativeLink).toHaveCount(0);

  // The <code> element wrapping it must still be a plain <code>, not an <a>.
  const relativeCode = page.locator("code").filter({ hasText: "src/relative.rs" });
  await expect(relativeCode).toBeVisible();
});

test("C-14c: bare prose slash-string is NOT linkified", async ({ page }) => {
  await page.goto("/");

  const composer = page.getByRole("textbox").first();
  await composer.fill("file path test");
  await composer.press("Enter");

  await expect(page.getByText(/for details/)).toBeVisible();

  // Bare prose slash-string must not become a file-path link.
  const bareLink = page.locator('a[data-file-path="/bare/prose/path"]');
  await expect(bareLink).toHaveCount(0);
});

test("C-12 still works: http link opens the panel, not a file link", async ({ page }) => {
  await page.goto("/");
  const urlBefore = page.url();

  const composer = page.getByRole("textbox").first();
  await composer.fill("link please");
  await composer.press("Enter");

  // Wait for the reply link to appear.
  const link = page.getByRole("link", { name: /example\.com/ });
  await expect(link).toBeVisible();

  // Confirm it is NOT a file-path link.
  await expect(link).not.toHaveAttribute("data-file-path");

  // Click opens the canvas panel (C-12 unchanged).
  await link.click();
  await expect(page.getByText("example.com", { exact: true })).toBeVisible();

  // No navigation.
  expect(page.url()).toBe(urlBefore);
});
