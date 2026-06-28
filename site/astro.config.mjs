// @ts-check
import { defineConfig } from "astro/config";
import tailwindcss from "@tailwindcss/vite";
import sitemap from "@astrojs/sitemap";

// Dedicated project repo → served at https://<user>.github.io/zanto-rust/.
// When you move to a custom domain (e.g. https://zanto.app):
//   - set `site` to the domain
//   - remove `base` (or set to "/")
//   - add `public/CNAME` containing the domain
export default defineConfig({
  site: "https://satyamyadav.github.io",
  base: "/zanto-rust",
  integrations: [sitemap()],
  vite: {
    plugins: [tailwindcss()],
  },
});
