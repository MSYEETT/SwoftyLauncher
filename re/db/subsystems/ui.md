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
- 2026-10-19 23:41 rnd-kb: rnd-kb: eXRef rule (format reference 5.4/5.5 should say it): an eXRef carries fileRef AND an xref u64 = md5-64 (big-endian, path without .idf) of the referenced scene's eaid, i.e. that scene's componentId. The ENGINE RESOLVES BY xref, NOT fileRef. Every stock eXRef has xref == md5-64(fileRef). Changing only fileRef keeps the old scene in game while a fileRef-following viewer shows the new one (seen on the R&D pie: chart_1.fileRef -> rnd_dev_potential5 but xref still md5-64(rnd_dev_potential) drew the stock 4 wedges). Scene edits must set xref with fileRef; validators should fail xref != md5-64(fileRef). Evidence and the pie chain: rnd_ui.md#exref, #pie.
