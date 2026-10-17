import { useData, withBase } from "vitepress";

export function isExternal(link: string) {
  return /^[a-z][a-z0-9+.-]*:/i.test(link);
}

export function useLinks() {
  const { site } = useData();

  function to(link: string) {
    if (!link || isExternal(link)) return link;
    const [path, hash] = link.split("#");
    let out = path;
    if (out && !site.value.cleanUrls && !out.endsWith("/") && !out.endsWith(".html")) out += ".html";
    return (out ? withBase(out) : "") + (hash ? `#${hash}` : "");
  }

  function asset(src: string) {
    return isExternal(src) ? src : withBase(src);
  }

  function rel(link: string) {
    return isExternal(link) ? "noopener" : undefined;
  }

  function samePage(link: string, routePath: string) {
    if (!link || isExternal(link)) return false;
    const norm = (p: string) =>
      p
        .replace(/#.*$/, "")
        .replace(/\.html$/, "")
        .replace(/\/index$/, "/")
        .replace(/^\/+/, "/");
    const base = site.value.base;
    const current = routePath.startsWith(base) ? "/" + routePath.slice(base.length) : routePath;
    return norm(link) === norm(current);
  }

  return { to, asset, rel, samePage };
}
