import { test, expect } from "@playwright/test";

// D6-live: Attaching a file via the paperclip button and sending shows an
// attachment chip on the user message bubble.
// The mock `plugin:dialog|open` handler returns ["/home/user/docs/notes.txt"],
// so clicking Attach files (aria-label) calls ipc.pickFiles() → one attachment
// chip in the composer ("notes.txt"). On send, the user entry carries the
// attachment metadata and Message.svelte renders a chip with [data-attachment-chip]
// inside the user bubble (data-role="user"). The chip shows the file name "notes.txt".
test("D6-live: sending with an attachment shows a chip on the user bubble", async ({ page }) => {
  await page.goto("/");

  // Click the attach button — triggers ipc.pickFiles() → mock returns notes.txt.
  const attachBtn = page.getByRole("button", { name: "Attach files" });
  await attachBtn.click();

  // Composer chip must appear showing the file name.
  await expect(page.getByText("notes.txt")).toBeVisible();

  // Type a message and send.
  const composer = page.getByRole("textbox").first();
  await composer.fill("here is the file");
  await composer.press("Enter");

  // Wait for the user bubble (bg-primary) to appear.
  const userBubble = page.locator('[data-role="user"]');
  await expect(userBubble).toBeVisible();

  // The attachment chip must be inside the user bubble.
  const chip = userBubble.locator("[data-attachment-chip]");
  await expect(chip).toBeVisible();
  await expect(chip).toContainText("notes.txt");
});

// D6-reopen: Opening a session whose persisted messages include attachment metadata
// renders attachment chips on the user bubble (reopen path via toEntries mapping).
// The mock "Attachment session" (id "sess-attachments") contains a user message with
// attachments: [{ path: "/home/user/docs/report.pdf", name: "report.pdf", is_image: false }].
// toEntries() maps is_image → isImage; Message.svelte renders the chip.
test("D6-reopen: reopening a session with persisted attachments shows chips on the user bubble", async ({
  page,
}) => {
  await page.goto("/");

  // Click "Attachment session" in the sidebar to load the session.
  await page.getByText("Attachment session").click();

  // The user message from that session must appear.
  const userBubble = page.locator('[data-role="user"]');
  await expect(userBubble).toBeVisible();

  // The attachment chip for "report.pdf" must be rendered inside the user bubble.
  const chip = userBubble.locator("[data-attachment-chip]");
  await expect(chip).toBeVisible();
  await expect(chip).toContainText("report.pdf");
});
