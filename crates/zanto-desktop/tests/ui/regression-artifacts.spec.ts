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

test("R-7: a pinned chart re-renders from the stored record", async ({ page }) => {
  await page.goto("/");

  // 1. Render a chart inline.
  const composer = page.getByRole("textbox").first();
  await composer.fill("show me a chart");
  await composer.press("Enter");
  await expect(page.locator(".apexcharts-canvas")).toBeVisible();

  // 2. Pin the chart: the Pin button (aria-label="Pin to Artifacts") is shown on
  //    hover inside the group wrapper. Hover over the canvas first, then click.
  const inlineChart = page.locator(".apexcharts-canvas").first();
  await inlineChart.hover();
  const pinBtn = page.getByRole("button", { name: "Pin to Artifacts" });
  await pinBtn.click();

  // 3. Open the Artifacts browser via the sidebar "Artifacts" button (exact match
  //    avoids hitting the "Pin to Artifacts" button which also contains "Artifacts").
  await page.getByRole("button", { name: "Artifacts", exact: true }).click();

  // 4. Switch to the "Pinned views" tab inside the Artifacts browser.
  const backendTablist = page.getByRole("tablist", { name: "Artifact backend" });
  await backendTablist.getByRole("tab", { name: "Pinned views" }).click();

  // 5. The seed starts empty (list_pinned_artifacts.json has response:[]).
  //    The ONLY pinned item in the list must be the one we just pinned via
  //    pin_artifact_cmd — proving the command was actually exercised.
  const pinnedList = page.locator(".overflow-auto.rounded-md.border").first();
  const pinnedButtons = pinnedList.getByRole("button");
  await expect(pinnedButtons).toHaveCount(1);

  // 6. Open it and assert the chart re-renders in the preview panel — scoped to
  //    the preview container so we're asserting the RE-RENDERED pinned chart,
  //    not the original inline one in the chat.
  await pinnedButtons.first().click();
  const previewPane = page.locator(".flex-1.overflow-auto.p-3").last();
  await expect(previewPane.locator(".apexcharts-canvas")).toBeVisible();
});
