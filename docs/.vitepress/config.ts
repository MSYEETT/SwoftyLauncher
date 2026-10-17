import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { defineConfigWithTheme } from "vitepress";
import gameApi from "./game-api.json";
import { flashCodeTheme } from "./theme/code-theme";
import type { DocsThemeConfig } from "./theme/types";

const repo = "https://github.com/Swofty-Developments/F1ModManager";
const base = process.env.DOCS_BASE ?? "/";
const site = "https://f1.swofty.net";

const fonts =
  "https://fonts.googleapis.com/css2?family=Open+Sans:wght@400;600;700&family=Poppins:wght@400;600;700&family=Quicksand:wght@600;700&family=Source+Code+Pro:wght@400;500;600&family=Varela+Round&display=swap";

const pickTheme = `try{var t=localStorage.getItem("f1mm-docs-theme");if(t==="dark"||t==="ocean"||t==="peach")document.documentElement.classList.add("flash-theme-"+t)}catch(e){}`;

const pickSidebar = `try{if(localStorage.getItem("f1mm-docs-sidebar")==="closed")document.documentElement.classList.add("flash-sidebar-closed")}catch(e){}`;

const moved: Record<string, string> = {
  "/tutorials/first-addon": "/reference/addons/first-addon",
  "/tutorials/addon-settings": "/reference/addons/settings",
  "/tutorials/hooking": "/reference/addons/hooking",
  "/tutorials/in-game-ui": "/reference/addons/in-game-ui",
  "/tutorials/database": "/reference/mods/database",
  "/tutorials/erp-assets": "/reference/mods/erp-assets",
  "/modding/template": "/reference/addons/template",
  "/modding/manifest": "/reference/addons/manifest",
  "/modding/native-plugins": "/reference/addons/native-plugins",
  "/modding/database-ops": "/reference/ops/database",
  "/modding/erp-ops": "/reference/ops/erp",
  "/modding/ui-patches": "/reference/ops/ui-patches",
  "/reference/std/files": "/reference/ops/files",
};

const gameApiItems = gameApi.subsystems.map((s) => ({
  text: s.name,
  link: s.link,
  linkText: `${s.name}.hpp`,
  icon: "file",
  items: s.entries.map((e) => ({ text: e.name, link: e.link, icon: e.kind })),
}));

export default defineConfigWithTheme<DocsThemeConfig>({
  title: "Swofty Launcher for F1 25",
  description: "Addons and mods for F1 25 on Windows and Linux",
  base,
  lang: "en-AU",
  appearance: "force-dark",
  srcExclude: ["**/public/**", "scripts/**"],
  sitemap: { hostname: site },
  buildEnd({ outDir }) {
    for (const [from, to] of Object.entries(moved)) {
      const url = `${base}${to.slice(1)}.html`;
      const file = join(outDir, `${from}.html`);
      mkdirSync(dirname(file), { recursive: true });
      writeFileSync(
        file,
        `<!doctype html><meta charset="utf-8"><title>Moved</title><link rel="canonical" href="${url}"><meta http-equiv="refresh" content="0; url=${url}"><script>location.replace(${JSON.stringify(url)} + location.hash)</script><a href="${url}">${url}</a>\n`,
      );
    }
  },
  head: [
    ["link", { rel: "icon", href: `${base}logo.svg`, type: "image/svg+xml" }],
    ["meta", { name: "theme-color", content: "#2a1b39" }],
    ["meta", { property: "og:site_name", content: "Swofty Launcher for F1 25" }],
    ["meta", { property: "og:image", content: `${site}/screenshots/hero-launcher.png` }],
    ["link", { rel: "preconnect", href: "https://fonts.googleapis.com" }],
    ["link", { rel: "preconnect", href: "https://fonts.gstatic.com", crossorigin: "" }],
    ["link", { rel: "stylesheet", href: fonts }],
    ["script", {}, pickTheme],
    ["script", {}, pickSidebar],
  ],
  markdown: {
    theme: flashCodeTheme,
    lineNumbers: true,
    config(md) {
      const { rules } = md.renderer;
      const code = rules.code_inline!;
      rules.table_open = (tokens, idx, options, env, self) => {
        env.inTable = true;
        return `<div class="table-wrap">${self.renderToken(tokens, idx, options)}`;
      };
      rules.table_close = (tokens, idx, options, env, self) => {
        env.inTable = false;
        return `${self.renderToken(tokens, idx, options)}</div>\n`;
      };
      const cell: typeof code = (tokens, idx, options, _env, self) => {
        const inline = tokens[idx + 1];
        if (inline?.type === "inline" && inline.content.length <= 20) tokens[idx].attrJoin("class", "short");
        return self.renderToken(tokens, idx, options);
      };
      rules.th_open = cell;
      rules.td_open = cell;
      rules.code_inline = (tokens, idx, options, env, self) => {
        if (!env.inTable) return code(tokens, idx, options, env, self);
        const t = tokens[idx];
        const body = md.utils.escapeHtml(t.content).replace(/\.(?=[A-Za-z_])/g, ".<wbr>");
        return `<code${self.renderAttrs(t)}>${body}</code>`;
      };
    },
  },
  themeConfig: {
    logo: "/logo.svg",
    siteTitle: "Swofty Launcher for F1 25",
    repo,
    socialLinks: [{ icon: "github", link: repo }],
    search: {
      provider: "local",
    },
    tabs: [
      {
        id: "tutorials",
        text: "Tutorials",
        icon: "book",
        groups: [
          {
            text: "Getting Started",
            items: [
              { text: "Introduction", link: "/guide/" },
              { text: "Addons, mods and maps", link: "/guide/addons-mods-maps" },
              { text: "1. Installation", link: "/guide/installation" },
              { text: "1.1. Windows", link: "/guide/windows" },
              { text: "1.2. Linux and Steam Deck", link: "/guide/linux" },
              { text: "2. Addons and mods", link: "/using/mods-and-maps" },
              { text: "3. Maps", link: "/using/importing-maps" },
              { text: "4. Profiles", link: "/using/profiles" },
              { text: "5. In-game addon settings", link: "/using/swofty-mod" },
              { text: "6. Restoring the game", link: "/using/profiles-and-restoring" },
              { text: "7. Uninstalling", link: "/guide/uninstall" },
            ],
          },
          {
            text: "Handbook",
            items: [
              { text: "Launching the game", link: "/reference/launch" },
              { text: "Offline play", link: "/reference/safety" },
              { text: "Backups and recovery", link: "/reference/restore" },
            ],
          },
          {
            text: "Making your own",
            items: [
              { text: "Making a mod", link: "/guide/making-a-mod" },
              { text: "Making an addon", link: "/guide/making-an-addon" },
            ],
          },
        ],
      },
      {
        id: "reference",
        text: "Reference",
        icon: "code",
        mono: true,
        groups: [
          {
            text: "Creating mods",
            items: [
              { text: "Overview", link: "/reference/mods/", icon: "file" },
              { text: "Textures and models", link: "/reference/mods/erp-assets", icon: "file" },
              { text: "Game database", link: "/reference/mods/database", icon: "file" },
            ],
          },
          {
            text: "Creating addons",
            items: [
              { text: "Overview", link: "/reference/addons/", icon: "file" },
              { text: "Your first addon", link: "/reference/addons/first-addon", icon: "file" },
              { text: "Addon template", link: "/reference/addons/template", icon: "file" },
              { text: "Addon manifest", link: "/reference/addons/manifest", icon: "file" },
              { text: "Settings", link: "/reference/addons/settings", icon: "file" },
              { text: "Mixin system", link: "/reference/addons/hooking", icon: "file" },
              { text: "In-game UI", link: "/reference/addons/in-game-ui", icon: "file" },
              { text: "Native plugins", link: "/reference/addons/native-plugins", icon: "file" },
            ],
          },
          {
            text: "Standard library",
            items: [
              { text: "Overview", link: "/reference/std/", icon: "file" },
              { text: "Mixin API", link: "/reference/std/mixin", icon: "file" },
              { text: "Settings", link: "/reference/std/settings", icon: "file" },
              { text: "UI", link: "/reference/std/ui", icon: "file" },
              { text: "Logging", link: "/reference/std/logging", icon: "file" },
            ],
          },
          {
            text: "Game API",
            items: [{ text: "Overview", link: "/reference/game/", icon: "file" }, ...gameApiItems],
          },
          {
            text: "Formats",
            items: [
              { text: "ERP archives", link: "/reference/erp", icon: "file" },
              {
                text: "Game database (GVDB)",
                link: "/reference/gamedb",
                icon: "file",
                items: [
                  { text: "File format", link: "/reference/gamedb/format", icon: "file" },
                  { text: "How the game reads it", link: "/reference/gamedb/reading", icon: "file" },
                  { text: "Modding recipes", link: "/reference/gamedb/recipes", icon: "file" },
                  { text: "Tools", link: "/reference/gamedb/tools", icon: "file" },
                  { text: "Table catalogue", link: "/reference/gamedb/catalogue", icon: "file" },
                  { text: "Key schema", link: "/reference/gamedb/schema", icon: "file" },
                ],
              },
              { text: "!pkg resources", link: "/reference/pkg", icon: "file" },
              { text: "Database operations", link: "/reference/ops/database", icon: "file" },
              { text: "ERP operations", link: "/reference/ops/erp", icon: "file" },
              { text: "UI patches", link: "/reference/ops/ui-patches", icon: "file" },
              { text: "File operations", link: "/reference/ops/files", icon: "file" },
            ],
          },
          {
            text: "Tools",
            items: [{ text: "swofty (command line)", link: "/reference/cli", icon: "file" }],
          },
          {
            text: "Development",
            items: [
              { text: "Development", link: "/reference/development", icon: "file" },
              { text: "Design notes", link: "/design/", icon: "file" },
              { text: "Addons and mods design", link: "/design/addons-and-mods", icon: "file" },
              { text: "How maps are installed", link: "/reference/maps", icon: "file" },
            ],
          },
        ],
      },
    ],
  },
});
