import sys, os, copy
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pkg import Pkg, Obj, Arr, Val
from uinode import PROPS, CHILDREN, EVENTS, child, name, fix_ids, djb2
from swofty_settings import S, V, obj, prop, sig, event, node, var, dictionary

ONLINE_MESSAGES = (
    'online_services_generic_exception_online',
    'online_services_generic_exception_online_with_code',
    'online_services_generic_exception_offline',
    'online_services_generic_exception_offline_with_code',
)
TITLE = 'Swofty Mod v0.1.0'
BODY = ('Online services are off while mods are loaded, so multiplayer is unavailable.\n\n'
        'New in v0.1.0:\n'
        '- MOD SETTINGS on the home screen (F9 / R3)\n'
        '- Mod settings page listing installed mods\n'
        '- Launcher panel on the home screen')
BUTTON = 'lng_continue'
ONCE_ATTR = 'swofty_once'
SEEN = '#swofty.state.online_popup_seen'
UIDM = 'persistent.consume_input_wrapper.persistent_multichoice_message'


def token(name_, db, value):
    if db is None:
        return obj(('localise', V('bool', False)), ('value', S(value)), ('name', S(name_)))
    return obj(('localise', V('bool', False)), ('value', S(value)), ('name', S(name_)), ('dbEntry', S(db)))


def patch_messages(data, once, literal=False):
    p = Pkg(data)
    done = []
    for m in p.root['messages'].items:
        if m['id'].value not in ONLINE_MESSAGES:
            continue
        m['title'].value = '[swofty_title]'
        m['messageBody'].value = '[swofty_body]'
        m.set('titleTokens', obj(('swofty_title', token('swofty_title', None if literal else '#swofty.popup.title', TITLE))))
        m.set('bodyTokens', obj(('swofty_body', token('swofty_body', None if literal else '#swofty.popup.body', BODY))))
        m['options'].items = [obj(('value', S(BUTTON)), ('response', S('cancel')), ('defaultSelection', V('bool', True)))]
        if once:
            m['attributes'].items.append(obj(('localise', V('bool', False)), ('value', S('true')),
                                             ('name', S(ONCE_ATTR)), ('dbEntry', S(''))))
        done.append(m['id'].value)
    assert sorted(done) == sorted(ONLINE_MESSAGES), done
    return p.to_bytes()


def patch_ui_data_popup(data, literal=False):
    p = Pkg(data)
    sw = p.root['swofty']['Contents']
    assert sw.get('popup') is None
    if not literal:
        sw.set('popup', dictionary('popup', [
            ('title', var('title', 'uistring', TITLE)),
            ('body', var('body', 'uistring', BODY)),
        ]))
    sw['state']['Contents'].set('online_popup_seen', var('online_popup_seen', 'bool', False))
    return p.to_bytes()


def patch_persistent_gate(data):
    p = Pkg(data)
    scr = p.root['Screens'].items[0]
    assert name(scr) == 'persistent'
    m = child(child(scr, 'consume_input_wrapper'), 'persistent_multichoice_message')
    act = next(e for e in m.items[EVENTS].items if e['eventName'].value == 'activate_message')
    names = [(s['to_object_name'].value, s['to_pin_name'].value) for s in act['signals'].items]
    assert names == [('persistent.consume_input_wrapper', 'consumeStart'), (UIDM, 'show')], names
    show_sigs = copy.deepcopy(act['signals'].items)
    act['signals'].items = [sig(f'{UIDM}.swofty_once_check', 'is_valid', 'activate_message')]

    def relabel(sigs, frm):
        out = copy.deepcopy(sigs)
        for s in out:
            s['from_pin_name'].value = frm
        return out
    hidden = [('enabled', prop('bool', False)), ('visible', prop('bool', False))]
    m.items[CHILDREN].items += [
        node('swofty_once_check', 'DBVV', hidden + [('dbEntry', prop('stri', '', f'user_attributes.{ONCE_ATTR}'))],
             events=[event('yes', [sig(f'{UIDM}.swofty_seen_check', 'query', 'yes')]),
                     event('no', relabel(show_sigs, 'no'))]),
        node('swofty_seen_check', 'SVCD', hidden + [('value', prop('bool', True)), ('dbEntry', prop('stri', SEEN))],
             events=[event('no', [sig(f'{UIDM}.swofty_mark_seen', 'trigger', 'no')] + relabel(show_sigs, 'no')),
                     event('yes', [sig(UIDM, 'deactivate_message', 'yes'),
                                   sig(f'{UIDM}.disable_input', 'trigger', 'yes')])]),
        node('swofty_mark_seen', 'UIDS', hidden + [('source', prop('bool', True)), ('target', prop('bool', False, SEEN))]),
    ]
    fix_ids(scr)
    return p.to_bytes()
