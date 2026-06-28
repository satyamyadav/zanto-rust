# zanto-site

Marketing site for [zanto](https://github.com/satyamyadav/zanto-rust),
built with [Astro](https://astro.build) + Tailwind CSS v4 and deployed to GitHub
Pages. Lives inside the main repo under `site/`; CI builds it from there.

## Develop

```bash
pnpm install
pnpm dev          # http://localhost:4321/zanto-rust
pnpm build        # static output in dist/
pnpm preview
```

## Structure

```
src/
├── layouts/Base.astro          page shell (meta/OG, header, footer)
├── components/                 Header, Footer
├── pages/index.astro           home: hero + features + privacy CTA
└── lib/site.ts                 site config + base-path href() helper
```

## Deploy

`.github/workflows/site.yml` (at the repo root) builds and publishes on push to
`main` when anything under `site/**` changes.
**One-time setup:** repo Settings → Pages → Source → **GitHub Actions**.

The site is configured for a project page at `https://<user>.github.io/zanto-rust`
(`base: "/zanto-rust"` in `astro.config.mjs`). For a custom domain: set `site` to
the domain, drop `base`, and add `public/CNAME`.

## TODO before launch

- Replace `public/favicon.svg` with the real app icon and add `public/og.png`
  (1200×630) for link previews.
- Add product screenshots / a demo GIF to the home page.
- Confirm the GitHub repo URL / handle in `src/lib/site.ts`.
