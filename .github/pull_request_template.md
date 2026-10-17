## What changed

<!-- A sentence or two. What a reviewer needs to know before reading the diff,
     and why, if the title does not already say it. -->

## How it was checked

<!-- What you ran and what it said. "cargo test: 50 passed" beats "tested".
     Say whether the real-file round trips ran (F1_25_DIR set) and whether the
     change was tried in game. Say if something could not be checked, and why. -->

## Watch out for

<!-- Behaviour that changes for people already using the launcher, anything a
     diff will not show, and known follow-ups. "Nothing" is a fine answer. -->

---

- [ ] `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass
- [ ] Format changes still round-trip byte for byte against the Python tools with `F1_25_DIR` set
- [ ] Docs updated if behaviour changed
- [ ] No game data in the diff: no ERPs, `gamedatabases` copies, exe or DLL from the game, extracted assets or decompiled code
