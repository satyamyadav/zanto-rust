# Spec: migrate zanto-site into this repo at `site/`

Date: 2026-06-29
Status: proposed

## Goal

Bring the entire Astro marketing site (currently the standalone `zanto-app`
repo at `/home/lazy/dev/github/zanto-site`) into this repo (`zanto-rust`) under
`site/`, unchanged in structure. Local dev = `cd site && pnpm dev`. On push to
`main`, a path-filtered GitHub Actions job builds the site and publishes it to
GitHub Pages, so the public URL becomes
`https://satyamyadav.github.io/zanto-rust` (was `.../zanto-app`).

The existing Rust `release.yml` is untouched and independent.

## Decisions (confirmed)

- CI: **GitHub Actions + Pages** (not GitLab — both repos are GitHub).
- Site path: **`site/`**.
- Deploy trigger: **path filter on push to `main`** (`paths: ['site/**', workflow file]`).
- Astro base: **`/zanto-rust`**.

## Source inventory (what moves)

23 tracked files in `zanto-site`. Move the project verbatim into `site/`:

```
site/
├── astro.config.mjs          # base path edited (see below)
├── package.json
├── pnpm-lock.yaml
├── pnpm-workspace.yaml
├── tsconfig.json
├── README.md                 # base-path mentions edited
├── .gitignore                # site-local ignores (dist/, .astro/, node_modules/)
├── public/                   # og.png, hero.webm/mp4, llms.txt, robots.txt,
│                             #   favicon.svg, shots/*.png
└── src/
    ├── styles/global.css
    ├── layouts/Base.astro
    ├── components/Header.astro, Footer.astro
    ├── pages/index.astro
    └── lib/site.ts
```

Do **not** move: `zanto-site/.git`, `node_modules/`, `dist/`, `.astro/`
(build/dep/VCS artifacts — regenerated locally and in CI).

The base path is centralized: only `astro.config.mjs` sets `base`, and code reads
it via `import.meta.env.BASE_URL` (`src/lib/site.ts`). No source `.astro`/`.ts`
file hardcodes `/zanto-app`. Only `astro.config.mjs` and `README.md` mention it
literally.

## Changes

### 1. Copy the site into `site/`

Copy all tracked files from `zanto-site` into `local-work/site/`, preserving the
tree. Exclude `.git`, `node_modules`, `dist`, `.astro`.

Method: `git -C zanto-site archive HEAD | tar -x -C local-work/site` to copy
exactly the committed tree (no artifacts, no uncommitted cruft). Then `git add
site/`.

Caveat: confirm `zanto-site` has no important **uncommitted** changes before
using `archive HEAD` (it copies the last commit, not the working tree). Check
`git -C zanto-site status --porcelain`; if dirty with wanted changes, commit
there first or copy the working tree instead.

### 2. Edit `site/astro.config.mjs`

```diff
- // Dedicated project repo → served at https://<user>.github.io/zanto-app/.
+ // Dedicated project repo → served at https://<user>.github.io/zanto-rust/.
  ...
-   base: "/zanto-app",
+   base: "/zanto-rust",
```

`site:` stays `https://satyamyadav.github.io`.

### 3. Edit `site/README.md`

Replace the three `/zanto-app` references (dev URL, project-page URL, `base:`
line) with `/zanto-rust`. Optionally note it now lives inside the main repo.

### 4. Root `.gitignore`

Site build/dep artifacts are covered by `site/.gitignore` (kept). No root change
strictly required since git applies nested `.gitignore`. Optional: add explicit
`site/node_modules/`, `site/dist/`, `site/.astro/` to root `.gitignore` for
clarity. **Recommendation:** keep `site/.gitignore` as the source of truth, no
root edit.

### 5. New workflow `.github/workflows/site.yml`

Adapted from `zanto-site/.github/workflows/deploy.yml`, with a path filter and a
working-directory so it builds the subdir. `withastro/action@v3` supports a
`path` input for the project subdirectory.

```yaml
name: site

# Build the Astro site (in site/) and publish to GitHub Pages.
# One-time setup: repo Settings → Pages → Source → "GitHub Actions".
on:
  push:
    branches: [main]
    paths:
      - 'site/**'
      - '.github/workflows/site.yml'
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: pages
  cancel-in-progress: false

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build with Astro
        uses: withastro/action@v3
        with:
          path: ./site          # project lives in the subdir
          node-version: 22      # pnpm 11 needs Node >= 22.13

  deploy:
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - name: Deploy to GitHub Pages
        id: deployment
        uses: actions/deploy-pages@v4
```

Notes:
- `withastro/action@v3` auto-detects pnpm from `site/pnpm-lock.yaml` and uploads
  the Pages artifact itself, so `deploy` needs no `upload-pages-artifact` step.
- `concurrency.group: pages` is fine as the only Pages-publishing workflow in
  this repo. `release.yml` does not touch Pages — no conflict.

## One-time manual steps (out of band, you do these)

1. In the **`zanto-rust`** GitHub repo: Settings → Pages → Source → **GitHub
   Actions**. (Pages was previously enabled on `zanto-app`.)
2. Decide the fate of the old `zanto-app` repo/site: archive it or leave it.
   Anything still linking to `.../zanto-app` will 404 after you stop publishing
   there — out of scope for this change but worth noting.
3. The local `zanto-site` working copy can stay as-is for reference; this repo
   does not import or symlink it. Per CLAUDE.md cross-project rule, no files are
   created in `zanto-site` by this work.

## Out of scope

- Custom domain / CNAME (config notes already document the path).
- Redirects from the old `/zanto-app` URL.
- Any change to `release.yml` or the Rust crates.
- Touching the `zanto-site` repo.

## Verification

1. `git -C zanto-site status --porcelain` is clean (or wanted changes committed)
   before copy.
2. After copy: `cd site && pnpm install && pnpm build` → `dist/` produced with no
   errors; built HTML references `/zanto-rust/...` asset paths (grep `dist` for
   `/zanto-rust`).
3. `cd site && pnpm dev` serves at `http://localhost:4321/zanto-rust`.
4. Workflow file is valid YAML; path filter limits it to `site/**` changes.
5. After push to `main` + enabling Pages: Actions `site` run is green; site loads
   at `https://satyamyadav.github.io/zanto-rust`.

## Risks

- **Uncommitted site changes lost** if using `archive HEAD` while the source is
  dirty. Mitigation: status check in step 1 / verification 1.
- **Pages source not switched** in repo settings → deploy step fails. Manual step
  1 covers it.
- **pnpm/Node mismatch** — pinned to Node 22 in the workflow, matching the
  original; `packageManager` field in `package.json` is carried over.
