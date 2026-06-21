import { test, expect } from "@playwright/test";

test("dev:mock server serves the app shell", async ({ page }) => {
  await page.goto("/");
  // The root mounts a full-screen container; assert it exists.
  await expect(page.locator("div.h-screen.w-screen")).toBeVisible();
});
