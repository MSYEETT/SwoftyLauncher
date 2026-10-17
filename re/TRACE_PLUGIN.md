# Trace plugin (`swofty_trace.dll`)

A passive, config-driven tracer for the native loader. It installs safetyhook MidHooks at KB signature sites, logs registers and guarded memory reads to `swofty/trace.log`, and changes nothing in the game. It is the L3 instrument of `PROTOCOL.md` §8 step 2.

Status: implemented and tested under Wine against a fake `F1_25.exe` (`re/trace_plugin/test/run_wine_test.sh`, 26 + 1 checks pass). Not yet run in the real game.

| path | what |
|---|---|
| `re/trace_plugin/src/swofty_trace.cpp` | the plugin (one file, no deps beyond safetyhook/Zydis from `native/third_party`) |
| `re/trace_plugin/build.sh` | zig cross-build → `re/trace_plugin/build/swofty_trace.dll` |
| `re/trace_plugin/test/` | fake game, test signatures generator, `T900.trace.json`, log checker, Wine runner |
| `re/trace_plugin/examples/home_menu.trace.json` | authoring example against the real KB |
| `re/tools/trace_build.py` | authoring config(s) + `re/db/signatures.json` → runtime `trace.json` |

## 1. Install and load order

- `swofty/mods/swofty_trace.dll` (plain `mods/`, **not** `mods/core`). The loader (`native/src/loader/version.c`) loads `mods/core/*.dll` first and `mods/*.dll` after, sorted, so behaviour hooks in core always resolve against unpatched code and the tracer can never break them.
- Reads `swofty/trace.json`. No file: logs `H ... config=missing status=idle`, returns 1, does nothing.
- Uses the loader API v1 only: `log` (one-line status into `loader.log`), `module_base`, `scan` (exactly-once rule enforced by the loader).

## 2. Authoring → runtime

Agents never hand-write the runtime file. They write an authoring config next to the test (`re/tests/T###.trace.json`) and build:

```
python3 re/tools/trace_build.py re/tests/T001.trace.json [re/tests/T003.trace.json ...] -o <session>/trace.json
```

Several authoring files merge into one session config (hook ids must be unique; later `log`/`limits` keys override earlier ones with a warning). `trace_build.py` copies each signature's `section/pattern/anchor/resolve/expect` and `rva = target - image_base` out of the KB, copies the KB `build` block, and validates everything the plugin would reject (register names, types, expression grammar, limits, kind/args/ret combinations, `.xtls` only, signature verified on the KB build). Options: `--signatures`, `--build-from-exe <exe>` (take sha256/size/timestamp from a file), `--allow-build-mismatch`, `--no-sha256`.

Authoring hook object = runtime hook object (§3.2) with `"sig": "<signature id>"` instead of the embedded signature object. `"raw_sig": {...}` embeds an ad-hoc signature (tests only; warned).

KB signature `kind` → hook kind: `mid` → `mid`; `func_start` → `func_start`; `call` (resolve `rel32`) → `func_start` on the callee; `data` → refused.

## 3. Runtime config `swofty/trace.json` (schema 1)

### 3.1 Top level

| key | type | default | meaning |
|---|---|---|---|
| `schema` | int | required | `1` |
| `build` | object | required | `{exe, sha256, size, pe_timestamp}` of the build the signatures were verified on |
| `check_sha256` | bool | `true` | hash the running exe with BCrypt at init (~1 s for 408 MB, on the loader's worker thread) and compare against `build.sha256` (prefix compare). `false` = size + PE timestamp only |
| `allow_build_mismatch` | bool | `false` | on mismatch install nothing unless `true`; when `true` the `rva` cross-check is skipped |
| `allowed_sections` | [string] | `[".xtls"]` | sections a signature may be scanned in **and** a target may lie in (target section must also be executable) |
| `log.max_mb` | int | 64 | event byte cap for `trace.log` (hard max 1024) |
| `log.on_full` | `"stop"`/`"rotate"` | `"stop"` | stop: write one `W` line, keep counting, write no more `E` lines. rotate: move to `trace.log.1`, start a new file |
| `log.flush_ms` | int | 100 | writer-thread drain period (min 10) |
| `log.summary_every_s` | int | 60 | periodic `S` lines (only hooks whose call count changed); 0 = only at exit |
| `limits.max_events_total` | int | 200000 | global `E` line cap across hooks; after it, counters only |
| `limits.max_read` | int | 64 | default length for `cstr`/`wstr`/`nestr` reads (hard max 1024) |
| `hooks` | [hook] | `[]` | at most 64; extras ignored with a `W` line |

### 3.2 Hook

| key | type | default | meaning |
|---|---|---|---|
| `id` | string | required | stable id `[a-z0-9_.]{1,63}`, conventionally `t001.after_career`. Appears in every line for this hook |
| `test` | string | `-` | owning test `T###` |
| `kind` | `"mid"`/`"func_start"` | from sig | `func_start` targets must equal a `.pdata` function begin (non-chained) and unlock `args` and `ret` |
| `sig` | object | required | `{id, section, pattern, anchor, resolve:{kind:"addr"/"rel32"}, expect?, rva?}`; `anchor` and `rva` are ints or `"0x.."` strings |
| `regs` | [string] | `[]` | logged raw: `rax rbx rcx rdx rsi rdi rbp rsp r8..r15 rflags xmm0..xmm15` (xmm = low 64 bits) |
| `args` | int 0..8 | 0 | `func_start` only: `a0..a3` = rcx, rdx, r8, r9; `a4+` = `[rsp+0x28+8*(i-4)]` |
| `fields` | [field] | `[]` | at most 24, see 3.3 |
| `when` | [string] | `[]` | at most 4, all must hold, see 3.4 |
| `ret` | bool | false | `func_start` only: log the return address `[rsp]` |
| `bt` | int 0..32 | 0 | unwind N caller frames with `RtlLookupFunctionEntry`/`RtlVirtualUnwind` from the hook site |
| `max_per_sec` | int | 50 | events per wall-clock second for this hook (0 = unlimited); excess counted as `rate_dropped` |
| `max_events` | int | 2000 | lifetime `E` lines for this hook; excess counted as `capped` |
| `enabled` | bool | true | `false` → `R ... reason=disabled` |
| `skip_boundary_check` | bool | false | skip the Zydis instruction-boundary check (only for code the linear decoder cannot walk) |

### 3.3 Fields

Object form `{"name": "...", "type": "...", "expr": "...", "len": N}` or compact string `"name=type:expr"` (length in the type: `hex[32]`, `cstr[128]`).

**Expression grammar** (evaluates to a u64; every `[ ]` is a guarded read):

```
expr := term { ('+' | '-') number }
term := register | aN | number | '[' expr ']'          number := 0x.. | decimal
```

`[[r14+0x78]+8]` reads the qword at `r14+0x78`, adds 8, reads the qword there. Nesting depth is limited by 16 ops per expression.

**Types:**

| type | expr means | output |
|---|---|---|
| `u8 u16 u32 u64 ptr` | if the whole expr is `[X]`: read sizeof(T) bytes at X; otherwise the expr value truncated | `0x..` lowercase hex |
| `i8 i16 i32 i64` | same | signed decimal |
| `f32 f64` | same (use `xmmN` for float registers) | decimal, up to 6 fraction digits, `e` exponent outside 1e-4..1e15, `nan`, `inf` |
| `bool` | same, 1 byte | `true`/`false` |
| `hex[N]` (default 16) | expr value = address of N bytes | `h:` + 2N hex digits |
| `cstr[N]` | address of a NUL-terminated string, at most N bytes | JSON-style quoted string, `\xNN` for non-printables; suffix `~cut` if N reached, `~unreadable` if the string ran into an unreadable page |
| `wstr[N]` | address of UTF-16LE string | quoted, non-ASCII as `?` |
| `nestr[N]` | expr value = `NeStr*` (block: u32 djb2 hash at +0, u16 refcount at +4, chars from +9) | `0x<hash>:"chars"`; null pointer → `null:""` (the engine's empty string, hash 0x1505) |

A failed read anywhere in the chain prints `!unreadable@0x<addr>` for that field (the first address that failed) and counts a `read_error`; the event is still written.

### 3.4 `when`

`"[type:]expr OP number"`, OP ∈ `== != < > <= >= &` (`&` = any bit set). `type` is a scalar type (default `u64`); `i*` types compare signed. A condition whose read fails is false. Filtered calls count as `filtered`. Example: `"u8:r12 == 2"`, `"[rcx+0x10] != 0"`, `"u32:[[rdx]] == 0x2b0357a"`.

## 4. Log `swofty/trace.log`

The previous run's log is moved to `trace.prev.log` at init. Every line is

```
<kind> <ms> <token> <token> ...
```

`<kind>` ∈ `H` header/meta, `R` hook resolution, `E` event, `S` summary, `W` warning. `<ms>` = milliseconds since plugin init, QueryPerformanceCounter-based, 3 decimals (monotonic). Tokens are `key=value`; values never contain spaces (strings are quoted and escaped). Maximum line length 2048 bytes; a longer event ends in ` ~trunc`. Lines are newline-terminated (`\n`).

| kind | tokens |
|---|---|
| `H` start | `plugin=swofty_trace version=1 pid= local=<YYYY-MM-DDThh:mm:ss> image_base= preferred_base=` |
| `H` build | `build size= want_size= pe_timestamp= want_pe_timestamp= sha256=<hex/skipped/error> want_sha256= sha_ms= result=match` or `result=MISMATCH allowed=0/1` |
| `H` status | `status=active hooks= installed= rejected=`, or `status=idle reason=build_mismatch installed=0`, or `config=missing/invalid/unsupported_schema status=idle` |
| `R` | `hook= test= sig= status=installed kind= match= target= rva= fields= when= max_per_sec= max_events=` or `hook= sig= status=rejected reason=<reason> [detail]` |
| `E` | `tid=<thread id> hook=<id> n=<call #, counts every hit incl. filtered/dropped> ev=<event # for this hook>` then, in this order: `regs` as `<reg>=0x..`, args `a0=..`, fields `<name>=<value>`, `ret=<addr>`, `bt=<addr>,<addr>,...` |
| `S` per hook | `hook= installed= calls= logged= filtered= rate_dropped= capped= ring_dropped= read_errors= [final=1]` |
| `S` total | `total logged= capped= dropped_log_full= stopped= [final=1]` |
| `W` | `log_full=1 max_bytes= events_stopped=1 counters_continue=1`, `too_many_hooks=..` |
| `H` end | `end=1` (written from `DLL_PROCESS_DETACH` after the final summary) |

Code addresses (`match`, `target`, `ret`, `bt`) inside `F1_25.exe` are printed **rebased to the preferred image base** (`0x140000000`), so they paste straight into the KB and Ghidra. Code addresses outside the exe are printed `@0x<raw>`. Register and field values are always raw.

`bt` starts at the hooked function's caller (at a `func_start` hook the first frame equals `ret`).

Real lines from the Wine test:

```
H 2.163 build size=191488 want_size=191488 pe_timestamp=2738223641 want_pe_timestamp=2738223641 sha256=b5b9c8...feb8 want_sha256=b5b9c8...feb8 sha_ms=3 result=match
R 6.140 hook=t900.tick test=T900 sig=fake.tick_mid status=installed kind=mid match=0x140034000 target=0x14003400b rva=0x3400b fields=7 when=0 max_per_sec=20 max_events=50
R 6.155 hook=t900.tick_head sig=fake.tick_head status=rejected reason=already_patched match=0x140034000 target=0x14003400b
E 5043.333 tid=36 hook=t900.tick n=1 ev=1 rbx=0x7fffff0ffe80 rsi=0x1 counter=0x29 chain=0x1122334455667788 name=0xf5e94237:"career" speed=312.5 head=h:4f424a4845414421 label="first" idx=1
E 5043.346 tid=36 hook=t900.tick n=2 ev=2 rbx=0x7fffff0ffe40 rsi=0x2 counter=0x29 chain=!unreadable@0x18 name=null:"" ...
E 7244.252 tid=36 hook=t900.entry n=1 ev=1 a0=0x1 a1=0x7 a2=0x3 a3=0x4 a4=0x5 sum_in=0x5 ret=0x14000103c bt=0x14000103c,0x14000105f,0x1400013d2,0x14000182d
S 8740.188 hook=t900.tick installed=1 calls=904 logged=50 filtered=0 rate_dropped=564 capped=290 ring_dropped=0 read_errors=2 final=1
```

### Evidence citation

A run's files go to `re/traces/T###/<run>/` (`trace.json`, `trace.log`, `loader.log`, a `RUN.md` with date, build sha, plugin md5, what the user did). Cite a line as `trace:T###/<run>:<line number in trace.log>`. With `on_full=rotate`, cite the rotated file explicitly (`trace:T###/<run>/trace.log.1:<line>`).

## 5. Resolution (per hook, in config order)

1. Config validation (kind, regs, args, fields, when, bt, ret) → `bad_*`.
2. `sig.section` ∈ `allowed_sections` → `section_not_allowed`.
3. `api->scan(section, pattern)` must report exactly 1 hit → `no_match hits=0` / `not_unique hits=N`.
4. target = match + anchor (`addr`) or the rel32 destination at match + anchor (`rel32`) → other kinds `resolve_not_code`.
5. Target lies in an allowed, executable section → `target_outside_allowed_code`.
6. `expect` bytes at the target → `expect_mismatch`, or `already_patched` when the target starts with `E9`/`FF 25` (another hook owns the site).
7. Build matched and `rva` given: target − image base == `rva` → `rva_mismatch`.
8. Instruction boundary: `RtlLookupFunctionEntry` finds the `.pdata` function (`no_pdata`); `func_start` must be its non-chained begin (`not_function_start`); `mid` must be reached by a Zydis linear decode from the function begin (`not_instruction_boundary`, `decode_failed`, `function_too_large` > 1 MB).
9. `safetyhook::MidHook::create` → `install_failed safetyhook_error=N`.

Every outcome is one `R` line plus one `[trace]` line in `loader.log`.

## 6. Safety limits

| limit | value |
|---|---|
| hooks | 64 (one compiled callback per slot; no shared trampoline state) |
| fields / when / regs / args / bt | 24 / 4 / 24 / 8 / 32 |
| bytes per read | 1024 hard cap (`max_read` default 64, `hex` default 16) |
| line length | 2048 bytes, then `~trunc` |
| events | per hook `max_per_sec` + `max_events`, global `max_events_total`; past any cap the hook only increments counters |
| log size | `log.max_mb` (≤ 1024) then stop or rotate; `H`/`S`/`W` lines are always written |
| memory reads | every read goes through `safe_read`: each touched page is checked with `VirtualQuery` (committed, readable, not guard/noaccess; result cached per page for ~0.5 s in a lock-free direct-mapped table), then copied by a tiny asm routine covered by a vectored exception handler that turns an access violation **inside that routine only** into a failed read. Pages freed or re-protected between the check and the copy are therefore reported, not fatal (tested: `VirtualProtect(PAGE_NOACCESS)` after a cached read) |
| hook path | no heap allocation, no CRT calls (own number/string formatter), no locks: events go into a 4096-slot × 2 KB bounded MPMC ring (Vyukov sequence numbers); a full ring drops the event and counts `ring_dropped`. A writer thread drains every `flush_ms`. The file lock is taken only by the writer thread, init and exit |
| build | size + PE timestamp (+ sha256 by default) must match `build`, else nothing is installed |
| sections | `.xtls` only by default; `.data1` (Denuvo-relocated bodies) can never be hooked unless a config explicitly allows that section, which `trace_build.py` refuses |
| unhooking | never; hooks live until process exit (no teardown races with game threads) |

## 7. Coexistence with behaviour hooks

A trace hook and a behaviour hook cannot share a site: safetyhook rewrites the first 5+ bytes, so the second hook's pattern no longer matches (`no_match`) or its `expect` fails (`already_patched`). Because core mods load first, the tracer is the one that loses, and the behaviour hook is never disturbed. To observe a site that core hooks:

- run the trace-only phase **without** the behaviour mod (PROTOCOL §8 step 2, the normal order), or
- trace a neighbouring instruction or the callee's `func_start` (e.g. `fe_home.career_adder` instead of `fe_home.after_career_site`).

## 8. Building and testing

```
re/trace_plugin/build.sh                    # → build/swofty_trace.dll (imports bcrypt, kernel32, user32, ucrt)
re/trace_plugin/test/run_wine_test.sh       # fake F1_25.exe + native/dist/version.dll + plugin under Wine/Xvfb
```

The test builds a fake `F1_25.exe` whose `.xtls` holds two functions with `.pdata`, generates a KB-format `signatures.json` for it (`test/mksigs.py`), builds the runtime config through `trace_build.py --build-from-exe`, runs it, and checks the log (`test/check_log.py`): register output, `[reg+off]`, a 2-level pointer chain, NeStr, f32, hex, cstr, `args`/`ret`/`bt`, `when` filtering, per-second rate limiting, `max_events` capping, periodic and final summaries, a null-pointer chain and a VEH-caught fault reported as `!unreadable@..`, and six rejected hooks (not unique, no match, wrong section, expect mismatch, mid-instruction target, site already patched). Phase 2 changes `build.size` and checks that nothing is installed.

## 9. Known limitations

- Rate limiting uses one-second wall-clock windows (not a sliding window); counters are relaxed atomics, so per-second limits can overshoot by a few events under heavy multi-thread contention.
- Event lines are ordered by ring reservation; across threads, `<ms>` can be out of order by microseconds.
- `bt` at a site inside an epilogue unwinds wrongly (the unwinder's epilogue detection sees our `jmp`); keep mid sites out of epilogues.
- The Zydis boundary check walks linearly from the function begin; code with embedded data before the site needs `skip_boundary_check` plus a static argument in the test.
- `check_sha256` reads the whole 408 MB exe once at startup (~1 s from page cache).
- Not yet run inside the real game; first real use should be a test with a single low-rate hook (`max_per_sec` ≤ 5).
