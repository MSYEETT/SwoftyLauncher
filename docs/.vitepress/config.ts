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

const gameApiItems = gameApi.subsystems.map((s) => ({
  text: s.name,
  link: s.link,
  linkText: `${s.name}.hpp`,
  icon: "file",
  items: s.entries.map((e) => ({ text: e.name, link: e.link, icon: e.kind })),
}));

export default defineConfigWithTheme<DocsThemeConfig>({
  title: "F1 Mod Manager",
  description: "A mod manager and launcher for F1 25 on Windows and Linux",
  base,
  lang: "en-AU",
  appearance: "force-dark",
  srcExclude: ["**/public/**", "scripts/**"],
  sitemap: { hostname: site },
  head: [
    ["link", { rel: "icon", href: `${base}logo.svg`, type: "image/svg+xml" }],
    ["meta", { name: "theme-color", content: "#2a1b39" }],
    ["meta", { property: "og:site_name", content: "F1 Mod Manager" }],
    ["meta", { property: "og:image", content: `${site}/screenshots/hero-launcher.png` }],
    ["link", { rel: "preconnect", href: "https://fonts.googleapis.com" }],
    ["link", { rel: "preconnect", href: "https://fonts.gstatic.com", crossorigin: "" }],
    ["link", { rel: "stylesheet", href: fonts }],
    ["script", {}, pickTheme],
  ],
  markdown: {
    theme: flashCodeTheme,
    lineNumbers: true,
  },
  themeConfig: {
    logo: "/logo.svg",
    siteTitle: "F1 Mod Manager",
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
              { text: "1. Installation", link: "/guide/installation" },
              { text: "1.1. Windows", link: "/guide/windows" },
              { text: "1.2. Linux and Steam Deck", link: "/guide/linux" },
              { text: "2. Mods and maps", link: "/using/mods-and-maps" },
              { text: "3. Profiles", link: "/using/profiles" },
              { text: "4. In-game mod settings", link: "/using/swofty-mod" },
              { text: "5. Restoring the game", link: "/using/profiles-and-restoring" },
              { text: "6. Uninstalling", link: "/guide/uninstall" },
            ],
          },
          {
            text: "Modding Tutorials",
            items: [
              { text: "Making a mod", link: "/modding/template" },
              { text: "Configuring mods (swofty.json)", link: "/modding/manifest" },
              { text: "Hooking game functions", link: "/modding/native-plugins" },
              { text: "Database edits", link: "/modding/database-ops" },
              { text: "Archive (ERP) edits", link: "/modding/erp-ops" },
              { text: "UI patches", link: "/modding/ui-patches" },
              { text: "Maps", link: "/using/importing-maps" },
            ],
          },
          {
            text: "Handbook",
            items: [
              { text: "Design notes", link: "/design/" },
              { text: "Launching the game", link: "/reference/launch" },
              { text: "Offline play", link: "/reference/safety" },
              { text: "Backups and recovery", link: "/reference/restore" },
              { text: "Development", link: "/reference/development" },
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
            text: "Game API",
            items: [{ text: "Overview", link: "/reference/game/", icon: "file" }, ...gameApiItems],
          },
          {
            text: "Formats",
            items: [
              { text: "ERP archives", link: "/reference/erp", icon: "file" },
              { text: "Game database (GVDB)", link: "/reference/gvdb", icon: "file" },
              { text: "!pkg resources", link: "/reference/pkg", icon: "file" },
            ],
          },
          {
            text: "Tools",
            items: [{ text: "swofty (command line)", link: "/reference/cli", icon: "file" }],
          },
        ],
      },
    ],
  },
});
