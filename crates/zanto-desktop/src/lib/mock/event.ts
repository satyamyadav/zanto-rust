// Mock of @tauri-apps/api/event. Aliased in --mode mock. Defines its own types
// so it never re-imports the (aliased) real module.
export type UnlistenFn = () => void;
export type EventCallback<T> = (event: { payload: T }) => void;

type Handler = (payload: unknown) => void;
const handlers = new Map<string, Set<Handler>>();

export async function listen<T>(event: string, cb: EventCallback<T>): Promise<UnlistenFn> {
  const h: Handler = (p) => cb({ payload: p as T });
  let set = handlers.get(event);
  if (!set) handlers.set(event, (set = new Set()));
  set.add(h);
  return () => { set!.delete(h); };
}

// Backend/test-facing: deliver an event payload to all current listeners.
export function emit(event: string, payload: unknown): void {
  handlers.get(event)?.forEach((h) => h(payload));
}

export function resetBus(): void {
  handlers.clear();
}
