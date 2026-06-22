import { backend } from "./backend";

// Mock of @tauri-apps/api/core `invoke`. Dispatches by command name to the
// in-memory fake backend. Unknown command names throw so a missing handler
// surfaces loudly in tests instead of silently returning undefined.
export async function invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const handler = backend[cmd];
  if (!handler) throw new Error(`mock invoke: no handler for "${cmd}"`);
  return (await handler(args)) as T;
}
