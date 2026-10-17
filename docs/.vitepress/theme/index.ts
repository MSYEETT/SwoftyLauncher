import type { Theme } from "vitepress";
import DefaultTheme from "vitepress/theme-without-fonts";
import Layout from "./Layout.vue";
import Landing from "./components/Landing.vue";
import "./styles/flash.css";
import "./styles/docs.css";
import "./styles/landing.css";

export default {
  extends: DefaultTheme,
  Layout,
  enhanceApp({ app }) {
    app.component("Landing", Landing);
  },
} satisfies Theme;
