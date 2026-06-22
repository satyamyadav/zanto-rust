import { test, expect } from "@playwright/test";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const ROOT = new URL("../../src", import.meta.url).pathname;
const ALLOWED = ["lib/ipc.ts", "lib/mock/"]; // relative to src/

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    return statSync(p).isDirectory() ? walk(p) : [p];
  });
}

test("tauri api is imported only from the ipc seam and mocks", () => {
  const offenders: string[] = [];
  for (const file of walk(ROOT)) {
    if (!/\.(ts|svelte)$/.test(file)) continue;
    const rel = file.slice(ROOT.length + 1);
    if (ALLOWED.some((a) => rel === a || rel.startsWith(a))) continue;
    const src = readFileSync(file, "utf8");
    if (/from\s+["']@tauri-apps\/(api|plugin-os)/.test(src)) offenders.push(rel);
  }
  expect(offenders, `stray @tauri-apps imports: ${offenders.join(", ")}`).toEqual([]);
});
