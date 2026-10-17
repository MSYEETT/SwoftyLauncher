import type { DefaultTheme } from "vitepress";

export interface NavItem {
  text: string;
  link: string;
  icon?: string;
  linkText?: string;
  items?: NavItem[];
}

export interface NavGroup {
  text: string;
  items: NavItem[];
}

export interface NavTab {
  id: string;
  text: string;
  icon: string;
  mono?: boolean;
  groups: NavGroup[];
}

export interface DocsThemeConfig extends DefaultTheme.Config {
  repo: string;
  tabs: NavTab[];
}
