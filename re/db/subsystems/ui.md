# ui: authored UI formats + packaging + binding runtime (menu edit/add)

Owner: see `subsystems.tsv` (prefix `Ui`, owner `ui-format`).

This lane owns the **format reference**, not a slice of the exe. The full write-up is
[`ui_system.md`](ui_system.md): the `!pkg` binary container, the `.ui`/UIDF node grammar,
`.uidb`/`.uir`, the `.idf` scene-layout resource, the ERP UI archive layout, the view-model
binding runtime, and the recipes to edit or add a widget/box/tile/screen.

Related exe lanes (functions/globals live there, not here): `vm` (node-class registry, binding,
routes, screen loader), `fe_flow` (flow state machine, container tab builders), `fe_home`
(home tile menu), `fe_tiles` (generic tile builder), `fe_popup`, `fe_input`, `hud`, `assets`
(eaid:// resolution), `career` (the R&D box-grid / pie / upgrade-tile surfaces this reference
cross-references).

Primary sources this reference is distilled from: the launcher's own UI generator
(`/mnt/work/VSC/F1ModManager/packages/core/src/ui/{node,pages,patch,check,mod}.rs`), the binary
format crates (`packages/formats/src/{pkg,erp}.rs`), and the native runtime loader/binder
(`native/src/core/{ui,ui_vm}.cpp`).

## Inbox
