// Central site config + helpers.

export const SITE = {
  name: "zanto",
  tagline: "A private, local-first AI workspace for your desktop.",
  description:
    "Private, local-first AI for your desktop. Bring your own model — or run fully offline with Ollama. " +
    "It works your files, runs tools and renders charts, with focused apps like personal finance. Nothing leaves your machine.",
  blog: "https://blog.zanto.xyz/",
  repo: "https://github.com/satyamyadav/zanto-rust",
  releasesLatest: "https://github.com/satyamyadav/zanto-rust/releases/latest",
  releases: "https://github.com/satyamyadav/zanto-rust/releases",
  linkedin: "https://www.linkedin.com/in/satyamyadav",
} as const;

// Base path (handles the GitHub Pages project subpath). Trailing slash stripped.
const base = import.meta.env.BASE_URL.replace(/\/$/, "");

/** Build an internal href that respects the configured `base`. */
export function href(path: string): string {
  const p = path.startsWith("/") ? path : `/${path}`;
  return `${base}${p}`;
}
