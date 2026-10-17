# game: game:: code named from pretty-function strings

Owner: see `subsystems.tsv`. Source: the bulk naming pass of 2026-10-17.

## Map
- Every row here is L1. Its name comes from a `__PRETTY_FUNCTION__` or log string that the function references (`str:` anchor plus `xref:` sites); the full C++ name is in `notes` as `cxx=`.
- A function was named only when it references exactly one such name and no other function references that name. Names shared by several functions are kept as anchors only, since they usually mean an inlined callee.
- The string can still belong to a callee inlined into the named function. Read the decompile before raising a row to L2.
- Rows move to a feature lane (`session`, `fe_home`, ...) when someone takes them on; rename to `<Prefix>_<Verb>_<Noun>` at that point.
- neon:: names from the same pass are in `core` (prefix `Ne`, class `Ne` prefix dropped). The exe has no `ego::` strings.

## Inbox
