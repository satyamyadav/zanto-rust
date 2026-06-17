// Artifact catalogue cache + AJV validators. Fetched from the backend catalogue on
// startup; Block.svelte uses validateArtifact() to AJV-check data before mounting.
import Ajv, { type ValidateFunction } from "ajv";
import { ipc, type ArtifactDef } from "$lib/ipc";

const ajv = new Ajv({ allErrors: true, strict: false });
const validators = new Map<string, ValidateFunction>();

// Ids of "view"-class artifacts (ephemeral, pinnable). Drives the A-5 user Pin
// button — shown only on rendered views, never on file/document artifacts.
// Reactive so Block.svelte updates once the catalogue resolves at startup.
export const viewArtifacts = $state(new Set<string>());

export async function loadCatalogue() {
  try {
    const defs: ArtifactDef[] = await ipc.getCatalogue();
    viewArtifacts.clear();
    for (const d of defs) {
      if (d.storage === "view") viewArtifacts.add(d.id);
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
