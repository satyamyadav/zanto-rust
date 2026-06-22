import { test, expect } from "@playwright/test";

test("sending a message renders a streamed assistant reply", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("hello");
  await composer.press("Enter");
  await expect(page.getByText("Hi there.")).toBeVisible();
});
