type DragPayload = { type: "enter" | "over" | "leave" | "drop"; paths: string[] };
type DropHandler = (e: { payload: DragPayload }) => void;
let dropHandler: DropHandler | null = null;

export function getCurrentWebviewWindow() {
  return {
    onDragDropEvent(cb: DropHandler) {
      dropHandler = cb;
      return Promise.resolve(() => { dropHandler = null; });
    },
  };
}

// Test-facing: simulate a native file drop.
export function emitDrop(payload: DragPayload): void {
  dropHandler?.({ payload });
}
