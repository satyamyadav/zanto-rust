// Artifact catalogue cache + AJV validators. Fetched from the backend catalogue on
// startup; Block.svelte uses validateArtifact() to AJV-check data before mounting.
import Ajv, { type ValidateFunction } from "ajv";
import { ipc, type ArtifactDef } from "$lib/ipc";

const ajv = new Ajv({ allErrors: true, strict: false });
const validators = new Map<string, ValidateFunction>();

export async function loadCatalogue() {
  try {
    const defs: ArtifactDef[] = await ipc.getCatalogue();
    for (const d of defs) {
      try {
        validators.set(d.id, ajv.compile(d.data_schema));
      } catch {
        /* skip an artifact with a bad schema */
      }
    }
  } catch {
    /* catalogue unavailable — render without validation */
  }
}

/** True if data is valid for `id`, or if no schema is known (don't block rendering). */
export function validateArtifact(id: string, data: unknown): boolean {
  const v = validators.get(id);
  return v ? (v(data) as boolean) : true;
}
