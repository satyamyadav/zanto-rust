import { test, expect } from "@playwright/test";

test("R-1: chart renders inline in one step, no base64 image or markdown-table fallback", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("show me a chart");
  await composer.press("Enter");
  // ApexCharts mounts a .apexcharts-canvas node inside the Chart component's root div.
  await expect(page.locator(".apexcharts-canvas")).toBeVisible();
  // No base64 image fallback, no markdown table standing in for the chart.
  await expect(page.locator('img[src^="data:image/png;base64"]')).toHaveCount(0);
});

test("R-2: artifact-rendering tool-call card is hidden when it renders as a block", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("chart with toolcall");
  await composer.press("Enter");
  await expect(page.locator(".apexcharts-canvas")).toBeVisible();
  // The render_artifact tool-call card is hidden (renders_as_block=true filters it
  // from the render items in Message.svelte via isHiddenToolCall). The tool name
  // must NOT appear anywhere in the page.
  await expect(page.getByText("render_artifact")).toHaveCount(0);
});
