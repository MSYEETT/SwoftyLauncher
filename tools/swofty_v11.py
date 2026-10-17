import sys, os, copy
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pkg import Pkg, Obj, Arr, Val
from uinode import PROPS, CHILDREN, LAYOUT, BEHAVIOURS, child, name, fix_ids, rename_refs, set_prop_value, drop_key
from swofty_settings import SCREEN, S, prop, node, fourcc
import swofty_v10 as s10

HIDE_BEHAVIOURS = ('NBEL', 'NBDC', 'NBMG', 'NBVM', 'NBFC', 'NBAN', 'NBAC', 'NBOS', 'NBXA', 'NBMO')


def strip_vm(scr):
    scr.items[PROPS].items = [(k, v) for k, v in scr.items[PROPS].items if k.value != 'allowVMBinding']
    assert scr.items[PROPS].get('allowVMBinding') is None
    return scr


def ui_bytes(scr):
    p = Pkg()
    p.root = Obj([(S('Screens'), Arr([scr]))])
    return p.to_bytes()


def build_v11a(fe_home_ui, syst):
    fe_home = Pkg(fe_home_ui).root['Screens'].items[0]
    scr = s10.build_screen(fe_home, Pkg(syst).root['Templates'].items)
    return ui_bytes(strip_vm(scr))


def _clone(n, new_name=None):
    c = copy.deepcopy(n)
    rename_refs(c, 'fe_career', SCREEN)
    if new_name:
        c.items[PROPS]['name'].value = new_name
    return c


def _literal(label, text):
    t = label.items[PROPS]['text']
    t['value'].value = text
    drop_key(t, 'dbentry')
    drop_key(t, 'isloctag')
    set_prop_value(label, 'localise', False, 'bool')


def build_v11b(fe_home_ui, syst, fe_career_ui):
    fe_home = Pkg(fe_home_ui).root['Screens'].items[0]
    fe_career = Pkg(fe_career_ui).root['Screens'].items[0]
    assert name(fe_career) == 'fe_career'
    scr = s10.build_screen(fe_home, Pkg(syst).root['Templates'].items)

    props = copy.deepcopy(fe_career.items[PROPS])
    props['name'].value = SCREEN
    scr.items[PROPS] = props
    scr.items[LAYOUT] = copy.deepcopy(fe_career.items[LAYOUT])
    scr.items[BEHAVIOURS] = Arr([b for b in copy.deepcopy(fe_career.items[BEHAVIOURS]).items
                                 if b['type'].value != fourcc('F1FS')])
    strip_vm(scr)

    drop = {'title', 'description', 'tooltips', 'hide_details_offline', 'hide_details_news',
            'hide_details_leagues', 'hide_beta_side_panel'}
    kids = [c for c in scr.items[CHILDREN].items if name(c) not in drop]

    title = _clone(child(fe_career, 'screen_title'))
    _literal(child(title, 'label_title'), 'ADDON SETTINGS')
    desc = _clone(child(fe_career, 'description'))
    _literal(desc, 'SwoftyMod settings and the mods loaded this session.')
    tips = _clone(child(fe_career, 'tooltips'))
    tips.items[CHILDREN].items = [c for c in tips.items[CHILDREN].items if name(c) in ('select', 'back')]
    divider = _clone(child(fe_career, 'branding_divider'))
    hidden = [node(f'hide_{h}', 'UIGP', [('visible', prop('bool', False))], layout=h, behav=HIDE_BEHAVIOURS)
              for h in ('details_offline', 'details_career_mp', 'details_career_solo', 'details_career_cc', 'cc_timer')]
    action = kids[0]
    assert name(action) == 'Action'
    scr.items[CHILDREN].items = [action, title, tips, desc, divider] + hidden + kids[1:]
    fix_ids(scr)
    return ui_bytes(scr)
