// In-app confirmation dialog, replacing `window.confirm` — which renders as a
// raw OS prompt (or is suppressed) inside the Tauri webview (B4-5). Call
// `confirm({...})` and await a boolean; a single `<ConfirmDialog/>` mounted at
// the app root renders the request and resolves the promise.

export type ConfirmOptions = {
  title?: string;
  body?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  /** Style the confirm button as destructive (red). */
  destructive?: boolean;
};

type PendingConfirm = ConfirmOptions & { resolve: (ok: boolean) => void };

let pending = $state<PendingConfirm | null>(null);

/** Reactive accessor for the mounted dialog component. */
export const confirmStore = {
  get pending(): PendingConfirm | null {
    return pending;
  },
};

/** Ask the user to confirm; resolves true on confirm, false on cancel/dismiss. */
export function confirm(opts: ConfirmOptions = {}): Promise<boolean> {
  // If one is already open, resolve it as cancelled before replacing it.
  pending?.resolve(false);
  return new Promise<boolean>((resolve) => {
    pending = { ...opts, resolve };
  });
}

/** Resolve the open request (called by the dialog component). */
export function resolveConfirm(ok: boolean): void {
  pending?.resolve(ok);
  pending = null;
}
