// Mock-mode stub for @tauri-apps/plugin-opener — no-op so the seam works in the browser harness.
export async function openUrl(_url: string): Promise<void> {}
