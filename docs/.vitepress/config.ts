import { defineConfig } from "vitepress";

const repo = "https://github.com/Swofty-Developments/F1ModManager";
const base = process.env.DOCS_BASE ?? "/F1ModManager/";

export default defineConfig({
  title: "F1 Mod Manager",
  description: "A mod launcher for F1 25 on Windows and Linux",
  base,
  lang: "en-AU",
  appearance: "dark",
  lastUpdated: true,
  srcExclude: ["**/public/**"],
  head: [
    ["link", { rel: "icon", href: `${base}logo.svg`, type: "image/svg+xml" }],
    ["meta", { name: "theme-color", content: "#0e1015" }],
  ],
  themeConfig: {
    logo: "/logo.svg",
    siteTitle: "F1 Mod Manager",
    search: {
      provider: "local",
    },
    nav: [
      { text: "Installation", link: "/guide/installation" },
      { text: "Maps", link: "/using/importing-maps" },
      { text: "Modding", link: "/modding/manifest" },
      { text: "Formats", link: "/reference/erp" },
      { text: "Contributors", link: "/design/" },
      { text: "Source", link: repo },
    ],
    sidebar: [
      {
        text: "Getting started",
        items: [
          { text: "Installation", link: "/guide/installation" },
          { text: "Windows", link: "/guide/windows" },
          { text: "Linux and Steam Deck", link: "/guide/linux" },
          { text: "Uninstalling", link: "/guide/uninstall" },
        ],
      },
      {
        text: "Using the launcher",
        items: [
          { text: "Mods and maps", link: "/using/mods-and-maps" },
          { text: "Importing maps", link: "/using/importing-maps" },
          { text: "Profiles", link: "/using/profiles" },
          { text: "Restoring the game", link: "/using/profiles-and-restoring" },
          { text: "In-game mod settings", link: "/using/swofty-mod" },
        ],
      },
      {
        text: "Making mods",
        items: [
          { text: "Manifest", link: "/modding/manifest" },
          { text: "Database operations", link: "/modding/database-ops" },
          { text: "ERP operations", link: "/modding/erp-ops" },
          { text: "UI patches", link: "/modding/ui-patches" },
          { text: "Native plugins", link: "/modding/native-plugins" },
          { text: "The mod template", link: "/modding/template" },
        ],
      },
      {
        text: "Reference",
        items: [
          { text: "ERP archives", link: "/reference/erp" },
          { text: "Game database (GVDB)", link: "/reference/gvdb" },
          { text: "!pkg resources", link: "/reference/pkg" },
          { text: "Command line", link: "/reference/cli" },
          { text: "Backups and recovery", link: "/reference/restore" },
          { text: "Launching the game", link: "/reference/launch" },
          { text: "Offline play", link: "/reference/safety" },
          { text: "Development", link: "/reference/development" },
        ],
      },
      {
        text: "Contributors",
        items: [{ text: "Design notes", link: "/design/" }],
      },
    ],
    socialLinks: [{ icon: "github", link: repo }],
    outline: [2, 3],
    editLink: {
      pattern: `${repo}/edit/main/docs/:path`,
      text: "Edit this page on GitHub",
    },
    footer: {
      message: "Released under the AGPL-3.0-or-later licence. The SDK and the mod template are MIT.",
      copyright: "Not affiliated with Codemasters, Electronic Arts or Formula 1.",
    },
    docFooter: {
      prev: "Previous page",
      next: "Next page",
    },
  },
});
