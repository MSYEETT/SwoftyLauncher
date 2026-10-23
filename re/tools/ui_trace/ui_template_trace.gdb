# Live trace of F1 25 UI template loading and instantiation (Steam build, base 0x140000000).
# Run through ui_template_trace.sh, never by hand without the signal handling below.
# Every breakpoint prints and continues, so the game never stays stopped.
# Addresses: docs/reference/ui/engine.md and re/db/globals.tsv (site_Vm_Template*, site_Vm_UiTabs_*).

set pagination off
set confirm off
set width 0
set print thread-events off
handle all nostop noprint pass
handle SIGTRAP stop noprint nopass
handle SIGINT stop print nopass

set $seq = 0
set $inst = 0
set $inst_max = 600

define ui_cc
  printf "%c%c%c%c", *(unsigned char*)($arg0), *(unsigned char*)($arg0+1), *(unsigned char*)($arg0+2), *(unsigned char*)($arg0+3)
end

define ui_cstr
  if $arg0 != 0
    printf "%s", (char*)$arg0
  else
    printf "?"
  end
end

define ui_nestr
  if $arg0 != 0
    printf "%s", (char*)($arg0 + 9)
  else
    printf "?"
  end
end

define ui_vt
  set $vt = *(unsigned long*)$arg0
  if $vt == 0x145a75ae0
    printf "UIGP"
  else
    if $vt == 0x145884850
      printf "F1TS"
    else
      if $vt == 0x14587bb30
        printf "TILE"
      else
        if $vt == 0x145a6cb50
          printf "SVCD"
        else
          if $vt == 0x145a63300
            printf "UIAM"
          else
            if $vt == 0x145a6af10
              printf "UITC"
            else
              printf "vt=%#lx", $vt
            end
          end
        end
      end
    end
  end
end

define ui_beh
  set $bv = *(unsigned long*)$arg0
  if $bv == 0x145a7eed0
    printf "NBVN(visual nav, needs prop visualNav)"
  else
    if $bv == 0x145a6d770
      printf "NBMO"
    else
      if $bv == 0x145a7ecf0
        printf "NBAN"
      else
        if $bv == 0x145a7f0b0
          printf "NBAC"
        else
          if $bv == 0x145a73fa0
            printf "NBEL"
          else
            printf "behaviour vt=%#lx", $bv
          end
        end
      end
    end
  end
end

# 1. Template store load: duplicate template name (Vm_Load_UidfTemplates returns 0x58 and the
#    UIDF handler drops every later template of that resource). sil == 1 marks the duplicate path.
break *0x144177cfd if ($rsi & 0xff) == 1
commands
silent
set $seq = $seq + 1
printf "[ui %d] TEMPLATE LOAD duplicate name '", $seq
ui_nestr *(void**)($rsp+0x40)
printf "': Vm_Load_UidfTemplates returns 0x58, the rest of this .ui resource's templates are NOT registered\n"
continue
end

# 2. UIDF handler: the syst (templates) fragment failed to load; the handler destroys the tracker
#    and stops walking that resource's fragments.
break *0x14449e292 if $eax != 0
commands
silent
set $seq = $seq + 1
printf "[ui %d] UIDF RESOURCE templates fragment failed rc=%d (%#x), resource id %016lx%016lx\n", $seq, $eax, $eax, *(unsigned long*)($rdi+8), *(unsigned long*)($rdi+0x10)
continue
end

# 3. Template node read failed (Vm_Read_UidfNode inside Vm_Build_TemplateNode).
break *0x14418160a if $eax != 0
commands
silent
set $seq = $seq + 1
printf "[ui %d] TEMPLATE NODE read failed rc=%d: node record unreadable (bad !pkg node array)\n", $seq, $eax
continue
end

# 4. Class FourCC has no factory, or the factory returned null.
break *0x144181765
commands
silent
set $seq = $seq + 1
printf "[ui %d] TEMPLATE NODE '", $seq
ui_cstr *(void**)($rdi+0x68)
printf "' class "
ui_cc ($rdi+0x70)
printf " has no registered factory (or it returned null): rc 5\n"
continue
end

# 5. Class init (vt+0x48 from-data) rejected a non-root node (roots are reported by tracepoint 12).
break *0x144181657 if $eax != 0 && *(long*)($rsp+0x1b0) == 0
commands
silent
set $seq = $seq + 1
printf "[ui %d] TEMPLATE NODE '", $seq
ui_cstr *(void**)($rdi+0x68)
printf "' class "
ui_cc ($rdi+0x70)
printf " init rejected it: rc=%d (%#x) [3 = a required key is missing (layout step nodeId/componentId, component name/componentId, behaviour type, list data dbentry); 5 = wrong value shape or FourCC mismatch; 0x22 = empty layout path; 0x57 = bad enum value]\n", $eax, $eax
continue
end

# 6. Failure chain: every exit of Vm_Build_TemplateNode with rc != 0 (failing node first, then each ancestor).
break *0x14418176a if $eax != 0
commands
silent
set $seq = $seq + 1
printf "[ui %d]   <- node '", $seq
ui_cstr *(void**)($rdi+0x68)
printf "' ("
ui_cc ($rdi+0x70)
printf ") returns rc=%d\n", $eax
continue
end

# 7. Instance link (root vt+0x50) failed. Hardware breakpoint: .data1 code is never patched.
python
try:
    gdb.execute('hbreak *0x153215dbf if $eax != 0')
except gdb.error as e:
    print('[ui] hardware breakpoint at 0x153215dbf refused (%s); .data1 code is never patched, so this tracepoint is off' % e)
    gdb.execute('break *0x140001000')
    gdb.execute('disable $bpnum')
end
commands
silent
set $seq = $seq + 1
printf "[ui %d] TEMPLATE INSTANCE '", $seq
ui_cstr *(void**)($rsp+0x1d8)
printf "' link (root vt+0x50) failed rc=%d\n", $eax
continue
end

# 8. Every template instantiation result (instance name, rc, root). rc != 0 leaves the partial tree attached to its parent.
python
try:
    gdb.execute('hbreak *0x153215e71')
except gdb.error as e:
    print('[ui] hardware breakpoint at 0x153215e71 refused (%s); .data1 code is never patched, so this tracepoint is off' % e)
    gdb.execute('break *0x140001000')
    gdb.execute('disable $bpnum')
end
commands
silent
set $inst = $inst + 1
if $inst <= $inst_max || $ebp != 0
  set $seq = $seq + 1
  printf "[ui %d] TEMPLATE INSTANCE #%d '", $seq, $inst
  ui_cstr *(void**)($rsp+0x1d8)
  if $ebp == 0
    printf "' ok root=%#lx ", $rdi
    ui_vt $rdi
    printf "\n"
  else
    printf "' FAILED rc=%d (%#x), partial root=%#lx left attached to its parent\n", $ebp, $ebp, $rdi
  end
end
continue
end

# 9. Tab controller (UITC): tab template name not in the template store (silently skipped by the engine).
break *0x144267110
commands
silent
set $seq = $seq + 1
printf "[ui %d] UITC TAB template '", $seq
ui_nestr *(void**)($rsp+0x58)
printf "' NOT FOUND in the template store (instance '"
ui_nestr *(void**)($rsp+0x50)
printf "'); tab left empty\n"
continue
end

# 10. Tab controller (UITC): tab instance failed (engine returns NULL, no log).
break *0x14426708d if *(int*)($rsp+0x78) != 0
commands
silent
set $seq = $seq + 1
if *(long*)($rsp+0x198) == 0
  printf "[ui %d] UITC TAB content instance '", $seq
else
  printf "[ui %d] UITC TAB header #%ld instance '", $seq, *(long*)($rsp+0x198)
end
ui_nestr *(void**)($rsp+0x50)
printf "' of template '"
ui_nestr *(void**)($rsp+0x58)
printf "' FAILED rc=%d; partial tree stays under the tab controller\n", *(int*)($rsp+0x78)
continue
end

# 11. Ordering marker: the era tile builder (core's site_FeTile_EraTilesBuilt sits inside it).
break *0x141a6e980
commands
silent
set $seq = $seq + 1
printf "[ui %d] ERA TILES build starts (FeTile_Build_EraTiles); core creates addon_tiles / addon_tiles_visible after this\n", $seq
continue
end

# 12. Template ROOT init result, success or failure (param_11 = instance name is set only for the root).
#     Prints which pkg buffer and node the root came from, and how many behaviours slot 6 lists.
break *0x144181657 if *(long*)($rsp+0x1b0) != 0
commands
silent
set $seq = $seq + 1
printf "[ui %d] ROOT INIT '", $seq
ui_cstr *(void**)($rdi+0x68)
printf "' ("
ui_cc ($rdi+0x70)
printf ") as '"
ui_cstr *(void**)($rsp+0x1b0)
printf "' init rc=%d pkg=%#lx props=%#lx slot6=%#lx", $eax, *(unsigned long*)($rdi+0x48), *(unsigned long*)($rdi+0x50), *(unsigned long*)($rdi+0x38)
if *(long*)($rdi+0x38) != 0
  printf " (%u behaviour(s))", *(unsigned int*)(*(unsigned long*)($rdi+0x38)+4) & 0xffffff
end
printf "\n"
continue
end

# 13. Element init: a behaviour's from-data init returned non-zero (slot-6 behaviour loop in Vm_ElementBase_Init).
break *0x144202550 if $eax != 0
commands
silent
set $seq = $seq + 1
printf "[ui %d] BEHAVIOUR INIT failed rc=%d on node '", $seq, $eax
ui_cstr *(void**)($r13+0x68)
printf "': "
ui_beh *(void**)($rsi+$rdi*8)
printf "\n"
continue
end

# 14. Element init: the NBAN re-init (event-driven) and the NBMO init returned non-zero.
break *0x1442028c8 if $eax != 0
commands
silent
set $seq = $seq + 1
printf "[ui %d] BEHAVIOUR INIT (NBAN re-init) failed rc=%d on node '", $seq, $eax
ui_cstr *(void**)($r13+0x68)
printf "'\n"
continue
end
break *0x144202978 if $eax != 0
commands
silent
set $seq = $seq + 1
printf "[ui %d] BEHAVIOUR INIT (NBMO) failed rc=%d on node '", $seq, $eax
ui_cstr *(void**)($r13+0x68)
printf "'\n"
continue
end

# 15. NBVN (visual navigation) init: remember the record at entry, report the visualNav read.
set $nbvn_rec = 0
set $nbvn_n = 0
break *0x144289830
commands
silent
set $nbvn_rec = $r8
continue
end
break *0x144289879
commands
silent
set $nbvn_n = $nbvn_n + 1
if $eax != 0 || $nbvn_n <= 200
  set $seq = $seq + 1
  printf "[ui %d] NBVN init on node '", $seq
  if $nbvn_rec != 0
    ui_cstr *(void**)($nbvn_rec+0x68)
  end
  if $eax == 0
    printf "': visualNav found\n"
  else
    printf "': visualNav MISSING, rc=%d (the node fails with this code)\n", $eax
  end
end
continue
end

# 16. UITC tab add: content or header, the instance name (always built from content_template) and the
#     template actually instantiated (the content_template or header_template value), with its record.
break *0x144266fa1
commands
silent
set $seq = $seq + 1
if *(long*)($rsp+0x198) == 0
  printf "[ui %d] UITC TAB content instance '", $seq
else
  printf "[ui %d] UITC TAB header #%ld instance '", $seq, *(long*)($rsp+0x198)
end
ui_nestr *(void**)($rsp+0x50)
printf "' builds template '"
ui_nestr *(void**)$r8
printf "' (record %#lx pkg=%#lx node_off=%#x)\n", $r8, *(unsigned long*)($r8+0x10), *(unsigned int*)($r8+0x18)
continue
end

# 17. UITC destroys an old tab instance (call into the instance destroyer).
break *0x144265036
commands
silent
set $seq = $seq + 1
printf "[ui %d] UITC destroys instance %#lx\n", $seq, $rdx
continue
end

printf "[ui] armed: 20 tracepoints; open F1 World -> Play now, press Ctrl+C in this terminal when done\n"
continue
printf "[ui] tracing stopped (%d instance(s) seen); breakpoint hit counts:\n", $inst
info breakpoints
info program
delete
detach
quit
