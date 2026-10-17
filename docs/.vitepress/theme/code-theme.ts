const c = (name: string) => `var(--flash-${name})`;

export const flashCodeTheme = {
  name: "flash",
  type: "dark" as const,
  colors: {
    "editor.foreground": c("white"),
    "editor.background": c("gray-darkest"),
  },
  tokenColors: [
    { settings: { foreground: c("white") } },
    { scope: ["comment", "punctuation.definition.comment"], settings: { foreground: c("less-dark"), fontStyle: "" } },
    {
      scope: ["keyword", "storage", "storage.type", "storage.modifier", "keyword.control", "keyword.other"],
      settings: { foreground: c("purple") },
    },
    { scope: ["string", "string.quoted", "punctuation.definition.string"], settings: { foreground: c("green") } },
    { scope: ["constant.numeric"], settings: { foreground: c("red") } },
    { scope: ["constant.language", "constant.language.boolean", "constant.language.null"], settings: { foreground: c("skin") } },
    {
      scope: ["entity.name.function", "support.function", "meta.function-call.generic", "entity.name.command"],
      settings: { foreground: c("blue") },
    },
    {
      scope: ["entity.name.type", "entity.name.class", "entity.name.struct", "support.class", "support.type", "storage.type.primitive"],
      settings: { foreground: c("yellow") },
    },
    {
      scope: ["support.type.property-name", "meta.object-literal.key", "variable.other.property", "entity.name.tag.yaml"],
      settings: { foreground: c("orange") },
    },
    {
      scope: ["meta.preprocessor", "keyword.control.directive", "entity.name.function.preprocessor", "punctuation.definition.directive"],
      settings: { foreground: c("cyan-light") },
    },
    { scope: ["punctuation", "keyword.operator", "meta.brace"], settings: { foreground: c("light") } },
    { scope: ["variable.parameter", "variable.other.readwrite", "variable"], settings: { foreground: c("white") } },
  ],
};
