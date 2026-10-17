import re, sys

lines = open(sys.argv[1]).read().splitlines()
fails = 0


def expect(desc, rx, count=None, at_least=None):
    global fails
    hits = [l for l in lines if re.search(rx, l)]
    ok = (count is None or len(hits) == count) and (at_least is None or len(hits) >= at_least) and (count is not None or at_least is not None or hits)
    print(f'{"ok  " if ok else "FAIL"} {desc}: {len(hits)} line(s)' + (f'  e.g. {hits[0][:150]}' if hits else ''))
    fails += not ok
    return hits


expect('header', r'^H \d+\.\d{3} plugin=swofty_trace version=1 ')
expect('build check matched', r'^H .* build size=\d+ .* result=match', 1)
expect('dup rejected (2 hits)', r'^R .*hook=t900\.dup .*status=rejected reason=not_unique hits=2', 1)
expect('no-match rejected', r'^R .*hook=t900\.nomatch .*reason=no_match hits=0', 1)
expect('wrong section rejected', r'^R .*hook=t900\.wrong_section .*reason=section_not_allowed \.text', 1)
expect('expect mismatch rejected', r'^R .*hook=t900\.expect_wrong .*reason=expect_mismatch', 1)
expect('mid-instruction target rejected', r'^R .*hook=t900\.mid_insn .*reason=not_instruction_boundary', 1)
expect('site already hooked detected', r'^R .*hook=t900\.tick_head .*reason=already_patched', 1)
expect('tick installed', r'^R .*hook=t900\.tick .*status=installed kind=mid', 1)
expect('entry installed', r'^R .*hook=t900\.entry .*status=installed kind=func_start', 1)
expect('active summary line', r'^H .*status=active hooks=8 installed=2 rejected=6', 1)
first = expect('first tick: regs, [reg+off], chain, NeStr, f32, hex, cstr', r'^E \d+\.\d{3} tid=\d+ hook=t900\.tick n=1 ev=1 rbx=0x[0-9a-f]+ rsi=0x1 counter=0x29 chain=0x1122334455667788 name=0x[0-9a-f]{8}:"career" speed=312\.5 head=h:4f424a4845414421 label="first" idx=1$', 1)
expect('bad pointer chain reported', r'hook=t900\.tick n=2 .* chain=!unreadable@0x18 name=null:""', 1)
expect('VEH armed read ok', r'hook=t900\.tick n=3 .* chain=0xabcdef ', 1)
expect('VEH-guarded fault reported', r'hook=t900\.tick n=4 .* chain=!unreadable@0x[0-9a-f]+8 ', 1)
ticks = expect('tick events capped at max_events=50', r'^E .*hook=t900\.tick ', 50)
expect('rate limit: <=20 tick events per second window', r'^E .*hook=t900\.tick .*label="flood1"', at_least=1)
expect('entry filtered by when rdx==7, args+ret+bt', r'^E .*hook=t900\.entry n=\d+ ev=\d a0=0x1 a1=0x7 a2=0x3 a3=0x4 a4=0x5 sum_in=0x5 ret=0x14[0-9a-f]+ bt=0x14[0-9a-f]+', 3)
s = expect('final tick summary', r'^S .*hook=t900\.tick installed=1 calls=904 logged=50 .*final=1', 1)
expect('rate_dropped > 0 in final summary', r'^S .*hook=t900\.tick .*rate_dropped=[1-9]\d* .*final=1', 1)
expect('capped > 0 in final summary', r'^S .*hook=t900\.tick .*capped=[1-9]\d* .*final=1', 1)
expect('read errors counted', r'^S .*hook=t900\.tick .*read_errors=[2-9] .*final=1', 1)
expect('entry filtered=2 in final summary', r'^S .*hook=t900\.entry installed=1 calls=5 logged=3 filtered=2 .*final=1', 1)
expect('periodic summaries', r'^S .*hook=t900\.tick (?!.*final=1)', at_least=1)
expect('end marker', r'^H .*end=1$', 1)
bad = [l for l in lines if not re.match(r'^[HREWS] \d+\.\d{3}( |$)', l)]
print(f'{"ok  " if not bad else "FAIL"} every line parses as <kind> <ms> ...: {len(lines)} lines, {len(bad)} bad')
fails += bool(bad)
print('ALL PASSED' if not fails else f'FAILED ({fails})')
sys.exit(1 if fails else 0)
