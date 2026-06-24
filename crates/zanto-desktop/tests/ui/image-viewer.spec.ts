import { test, expect } from "@playwright/test";

// D7-viewer: Opening a session whose message has an image attachment renders an
// image chip (thumbnail button). Clicking it opens the ImageViewer modal with an
// <img> whose src is the data-URL returned by read_image_data_url. Pressing Esc
// closes the viewer.
test("D7-viewer: clicking an image chip opens the viewer and Esc closes it", async ({ page }) => {
  await page.goto("/");

  // Load the image session from the sidebar.
  await page.getByText("Image session").click();

  // Wait for the user message bubble to appear.
  const userBubble = page.locator('[data-role="user"]');
  await expect(userBubble).toBeVisible();

  // The image chip must be rendered inside the bubble.
  const chip = userBubble.locator("[data-image-chip]");
  await expect(chip).toBeVisible();
  await expect(chip).toContainText("screenshot.png");

  // Click the chip to open the viewer.
  await chip.click();

  // The viewer must open.
  const viewer = page.locator("[data-image-viewer]");
  await expect(viewer).toBeVisible();

  // The viewer must show an <img> with a data:image/png src.
  const img = viewer.locator("[data-viewer-img]");
  await expect(img).toBeVisible();
  const src = await img.getAttribute("src");
  expect(src).toMatch(/^data:image\/png/);

  // Esc closes the viewer.
  await page.keyboard.press("Escape");
  await expect(viewer).not.toBeVisible();
});

// D7-viewer-reopen: clicking the image chip again after closing re-opens the viewer.
test("D7-viewer-reopen: viewer can be reopened after closing", async ({ page }) => {
  await page.goto("/");
  await page.getByText("Image session").click();

  const userBubble = page.locator('[data-role="user"]');
  await expect(userBubble).toBeVisible();

  const chip = userBubble.locator("[data-image-chip]");

  // Open → close via close button → reopen.
  await chip.click();
  const viewer = page.locator("[data-image-viewer]");
  await expect(viewer).toBeVisible();

  await page.getByRole("button", { name: "Close image viewer" }).click();
  await expect(viewer).not.toBeVisible();

  await chip.click();
  await expect(viewer).toBeVisible();
});
