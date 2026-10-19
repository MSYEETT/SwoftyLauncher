import html
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ART = os.path.dirname(HERE)
STOCK = {
    'tyre_thermal': ['tyre_analytics_data', 'lightweight_wheel_rims', 'tyre_blankets'],
    'brake_cooling': ['redesigned_brake_ducts', 'brake_disc_cooling_structure', 'calipers', 'brake_duct_winglets'],
    'tyre_life': ['tyre_analytics_data', 'suspension_geometry', 'streamlined_suspension_arms'],
    'pu_cooling': ['ers_cooling', 'intercooler_power', 'side_pod_profile_drag'],
}
STOCK_ICON = {'tyre_thermal': 'chassis_tyre_wear', 'brake_cooling': 'chassis_brakes', 'tyre_life': 'chassis_tyre_wear',
              'pu_cooling': 'maintenance_energy_store'}
STATES = [('', 'Available'), ('_progress', 'In progress'), ('_complete', 'Complete')]


def frames(prefix, name, label):
    cells = []
    for suffix, state in STATES:
        cells.append(f'<figure><img class="part" src="{prefix}/om_xm_{name}{suffix}.png" alt="">'
                     f'<figcaption>{state}</figcaption></figure>')
    cells.append(f'<figure><img class="part mask" src="{prefix}/om_xm_{name}_m.png" alt="">'
                 f'<figcaption>Mask</figcaption></figure>')
    return f'<div class="row"><div class="who">{label}</div><div class="cells">{"".join(cells)}</div></div>'


def card(name, display, size, branch, key, state):
    suffix = {'available': '', 'in_progress': '_progress', 'complete': '_complete'}[state]
    tag = {'available': 'Available', 'in_progress': 'In progress', 'complete': 'Fitted'}[state]
    return f'''<div class="card {state}">
  <div class="card-bg" style="background-image:url(stock/{'part_' + ('in_progress' if state == 'in_progress' else 'complete' if state == 'complete' else 'available')}.png)"></div>
  <div class="strip"></div>
  <img class="card-art" src="out/om_xm_{name}{suffix}.png" alt="">
  <div class="state"><span></span>{tag}</div>
  <div class="card-text">
    <div class="card-title">{html.escape(display)}</div>
    <div class="card-sub"><i style="-webkit-mask-image:url(out/{key}.png);mask-image:url(out/{key}.png)"></i>{size} &middot; {html.escape(branch)}</div>
  </div>
</div>'''


def main():
    tree = json.load(open(os.path.join(HERE, 'upgrades.json')))
    br = {b['key']: b for b in tree['branches']}
    bc = br['brake_cooling']['upgrades']
    mocks = ''.join([
        card(bc[1][0], bc[1][1], bc[1][2], 'Brake Cooling', 'brake_cooling', 'available'),
        card(bc[2][0], bc[2][1], bc[2][2], 'Brake Cooling', 'brake_cooling', 'in_progress'),
        card(bc[3][0], bc[3][1], bc[3][2], 'Brake Cooling', 'brake_cooling', 'complete'),
        card(br['tyre_life']['upgrades'][3][0], br['tyre_life']['upgrades'][3][1], 'Ultimate', 'Tyre Life', 'tyre_life',
             'complete'),
    ])
    sections = []
    for b in tree['branches']:
        rows = [f'<h3>Stock reference</h3>']
        rows += [frames('stock', s, s.replace('_', ' ')) for s in STOCK[b['key']]]
        rows.append('<h3>Ours</h3>')
        for name, display, size, desc in b['upgrades']:
            label = (f'<b>{html.escape(display)}</b><span>{size} &middot; <code>om_xm_{name}</code></span>'
                     f'<em>{html.escape(desc)}</em>')
            rows.append(frames('out', name, label))
        cond = ' <small>conditional</small>' if b.get('conditional') else ''
        sections.append(f'''<section>
  <h2><img class="icon" src="out/{b['key']}.png" alt=""><img class="icon stock" src="stock/{STOCK_ICON[b['key']]}.png" alt="">{html.escape(b['type'])}{cond}</h2>
  {''.join(rows)}
</section>''')
    page = TEMPLATE.replace('{{MOCKS}}', mocks).replace('{{SECTIONS}}', '\n'.join(sections))
    with open(os.path.join(ART, 'preview.html'), 'w') as f:
        f.write(page)


TEMPLATE = '''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Tyres Card Art</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link href="https://fonts.googleapis.com/css2?family=Titillium+Web:wght@400;600;700&display=swap" rel="stylesheet">
<style>
:root {
  --page: #0e1015;
  --panel: #151820;
  --line: #262a35;
  --text: #e9ebf0;
  --muted: #8b91a0;
  --tyres: #ff7800;
  --scale: 1;
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--page); color: var(--text); font: 14px/1.45 "Titillium Web", system-ui, sans-serif; }
header { position: sticky; top: 0; z-index: 2; display: flex; flex-wrap: wrap; gap: 12px 24px; align-items: center;
  justify-content: space-between; padding: 14px 24px; background: rgba(14, 16, 21, .94); border-bottom: 1px solid var(--line); }
header h1 { margin: 0; font-size: 18px; font-weight: 700; letter-spacing: .02em; }
header h1 span { color: var(--tyres); }
.seg { display: inline-flex; border: 1px solid var(--line); border-radius: 6px; overflow: hidden; }
.seg button { font: inherit; color: var(--muted); background: transparent; border: 0; padding: 4px 14px; cursor: pointer; }
.seg button[aria-pressed="true"] { background: var(--tyres); color: #111; font-weight: 700; }
main { padding: 8px 24px 48px; max-width: 2200px; }
p.lede { color: var(--muted); max-width: 900px; }
h2 { display: flex; align-items: center; gap: 10px; margin: 36px 0 8px; font-size: 22px; text-transform: uppercase;
  letter-spacing: .04em; border-bottom: 2px solid var(--tyres); padding-bottom: 6px; }
h2 small { font-size: 12px; color: var(--muted); border: 1px solid var(--line); padding: 1px 8px; border-radius: 10px; }
h2 .icon { width: 32px; height: 32px; image-rendering: pixelated; }
h2 .icon.stock { opacity: .45; margin-right: 6px; }
h3 { margin: 18px 0 6px; font-size: 12px; text-transform: uppercase; letter-spacing: .12em; color: var(--muted); }
.row { display: flex; flex-wrap: wrap; gap: 8px 20px; align-items: center; padding: 6px 0; border-top: 1px solid var(--line); }
.who { width: 260px; display: flex; flex-direction: column; gap: 2px; text-transform: capitalize; color: var(--muted); }
.who b { color: var(--text); font-size: 16px; text-transform: none; }
.who span, .who em { text-transform: none; font-size: 12px; color: var(--muted); }
.who code { color: #c9cdd6; }
.cells { display: flex; flex-wrap: wrap; gap: 6px; }
figure { margin: 0; }
figcaption { font-size: 11px; color: var(--muted); text-align: center; text-transform: uppercase; letter-spacing: .08em; }
img.part { display: block; width: calc(240px * var(--scale)); height: calc(240px * var(--scale)); background: var(--panel); }
img.mask { background: #000; }
.cards { display: flex; flex-wrap: wrap; gap: 20px; margin-top: 12px; }
.card { position: relative; width: calc(260px * var(--scale)); height: calc(348px * var(--scale)); overflow: hidden;
  background: #1b1e26; box-shadow: 0 6px 22px rgba(0,0,0,.5); font-size: calc(1px * var(--scale)); }
.card-bg { position: absolute; inset: 0; background-size: 100% 100%; filter: brightness(.42) contrast(1.05); }
.card.complete .card-bg { filter: brightness(.55); }
.card.in_progress .card-bg::after { content: ""; position: absolute; inset: 0; background: linear-gradient(0deg, rgba(255,120,0,.28), transparent 60%); }
.strip { position: absolute; left: 0; right: 0; top: 0; height: calc(4px * var(--scale)); background: var(--tyres); }
.card-art { position: absolute; left: calc(10px * var(--scale)); top: calc(44px * var(--scale)); width: calc(240px * var(--scale)); height: calc(240px * var(--scale)); }
.state { position: absolute; right: calc(11px * var(--scale)); top: calc(12px * var(--scale)); display: flex; align-items: center;
  gap: calc(6px * var(--scale)); font-size: 12em; text-transform: uppercase; letter-spacing: .06em; color: #d7dae0; }
.state span { width: calc(14px * var(--scale)); height: calc(14px * var(--scale)); border-radius: 50%; border: calc(2px * var(--scale)) solid #d7dae0; }
.card.in_progress .state span { border-color: var(--tyres); border-right-color: transparent; }
.card.complete .state span { background: var(--tyres); border-color: var(--tyres); }
.card-text { position: absolute; left: calc(11px * var(--scale)); right: calc(14px * var(--scale)); bottom: calc(11px * var(--scale)); }
.card-title { font-size: 25em; line-height: 1.02; font-weight: 700; text-transform: uppercase; text-shadow: 0 2px 4px rgba(0,0,0,.6); }
.card-sub { display: flex; align-items: center; gap: calc(6px * var(--scale)); margin-top: calc(6px * var(--scale)); font-size: 14em; text-transform: uppercase; color: #c9ccd3; }
.card-sub i { display: inline-block; width: calc(24px * var(--scale)); height: calc(24px * var(--scale)); background: var(--tyres);
  -webkit-mask-size: 100% 100%; mask-size: 100% 100%; }
.note { color: var(--muted); font-size: 12px; margin-top: 10px; max-width: 900px; }
@media (max-width: 700px) { header, main { padding-left: 16px; padding-right: 16px; } .who { width: 100%; } }
</style>
</head>
<body>
<header>
  <h1><span>Tyres &amp; Thermal Management</span> &middot; R&amp;D card art</h1>
  <div class="seg" role="group" aria-label="Scale">
    <button data-scale="1" aria-pressed="true">1&times; in-game</button><button data-scale="2" aria-pressed="false">2&times;</button>
  </div>
</header>
<main>
<p class="lede">Every upgrade has the four stock frames: the grey wireframe shown while the part is available, the white wireframe while it is in progress, the full render once it is fitted, and the soft silhouette mask. 1&times; is the texture's native 240&times;240, the size the card draws it at. The stock frames for comparison come from <code>src/extract_stock.py</code>.</p>
<h2>In context <small>mock card</small></h2>
<div class="cards">{{MOCKS}}</div>
<p class="note">Mock built from the stock <code>part_card.idf</code> layout: 260&times;348 card, the part image centred 10 px above the middle, title (26 bold, upper case) and category row (24 px icon, 14 regular) bottom left, state top right. The card backgrounds are the stock <code>rnd_card_bg</code> textures darkened by eye; the game tints them at runtime, so their exact tone is approximate, as is the page colour.</p>
{{SECTIONS}}
</main>
<script>
for (const b of document.querySelectorAll('[data-scale]')) {
  b.addEventListener('click', () => {
    document.documentElement.style.setProperty('--scale', b.dataset.scale);
    for (const o of document.querySelectorAll('[data-scale]')) o.setAttribute('aria-pressed', o === b ? 'true' : 'false');
  });
}
</script>
</body>
</html>
'''

if __name__ == '__main__':
    main()
