'use strict';

const $ = (sel, root = document) => root.querySelector(sel);

function h(tag, attrs, ...kids) {
  const n = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v === undefined || v === null || v === false) continue;
    if (k === 'class') n.className = v;
    else if (k === 'style') n.style.cssText = v;
    else if (k.startsWith('on')) n.addEventListener(k.slice(2), v);
    else if (k === 'text') n.textContent = v;
    else n.setAttribute(k, v === true ? '' : v);
  }
  for (const c of kids.flat()) {
    if (c === undefined || c === null || c === false) continue;
    n.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
  return n;
}

const TYPE_COLORS = {
  eScene: '#9aa4b2', eNull: '#8fa3bf', eXRef: '#4cc9f0', eText: '#ffd166',
  eStacker: '#c77dff', eScaler: '#80ed99', eCentre: '#72efdd', eWindow: '#ff8fab',
  eMask: '#ff6b6b', eMaskTexture: '#ff6b6b', ePoint: '#f8961e', eAnchor: '#f8961e',
  eBoundless: '#adb5bd', eScroller: '#e9c46a', eCanvas: '#a0c4ff', eCamera: '#f1f5f9',
};

function typeColor(t) {
  if (TYPE_COLORS[t]) return TYPE_COLORS[t];
  let x = 0;
  for (const c of String(t)) x = (x * 31 + c.charCodeAt(0)) >>> 0;
  return `hsl(${x % 360} 70% 70%)`;
}

const SCREEN = { w: 19.2, h: 10.8 };
const VALIDATION_SCENES = [
  ['art/screens/career/rnd_overview.idf', 'R&D overview: 2x2 tiles + secondary panels'],
  ['art/components/career/laptop/overview_panel.idf', 'card_stacker with card_0..card_4'],
  ['art/components/career/rnd_tree/rnd_dev_potential.idf', 'R&D pie: sector_1..4 at 0/90/180/270'],
];

const S = {
  catalog: null,
  doc: null,
  origin: null,
  model: null,
  els: {},
  ops: [],
  redo: [],
  sel: null,
  hov: null,
  view: { cx: 0, cy: 0, s: 40 },
  opts: { textures: true, outlines: false, text: true, reveal: false, xrefs: true, frame: true, grid: true, pivots: true, labels: 'auto' },
  hiddenTypes: new Set(),
  collapsed: new Set(),
  ghosts: new Map(),
  world: new Map(),
  drawOrder: [],
  paints: [],
  vis: new Map(),
  tex: new Map(),
  tints: new Map(),
  applySeq: 0,
  clonedPaths: new Set(),
  lastExport: null,
  drag: null,
  space: false,
};

/* ---------- math ---------- */

const ID4 = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];

function quatUnit(q) {
  const [x = 0, y = 0, z = 0, w = 0] = q || [];
  const n = Math.hypot(x, y, z, w);
  return n < 1e-9 ? [0, 0, 0, 1] : [x / n, y / n, z / n, w / n];
}

function trs(p, q, s) {
  const [px = 0, py = 0, pz = 0] = p || [];
  const [sx = 1, sy = 1, sz = 1] = s || [];
  const [x, y, z, w] = quatUnit(q);
  return [
    (1 - 2 * (y * y + z * z)) * sx, 2 * (x * y - z * w) * sy, 2 * (x * z + y * w) * sz, px,
    2 * (x * y + z * w) * sx, (1 - 2 * (x * x + z * z)) * sy, 2 * (y * z - x * w) * sz, py,
    2 * (x * z - y * w) * sx, 2 * (y * z + x * w) * sy, (1 - 2 * (x * x + y * y)) * sz, pz,
    0, 0, 0, 1,
  ];
}

function mul(a, b) {
  const o = new Array(16);
  for (let r = 0; r < 4; r++) {
    for (let c = 0; c < 4; c++) {
      o[r * 4 + c] = a[r * 4] * b[c] + a[r * 4 + 1] * b[4 + c] + a[r * 4 + 2] * b[8 + c] + a[r * 4 + 3] * b[12 + c];
    }
  }
  return o;
}

function apply(m, x, y, z) {
  return [m[0] * x + m[1] * y + m[2] * z + m[3], m[4] * x + m[5] * y + m[6] * z + m[7], m[8] * x + m[9] * y + m[10] * z + m[11]];
}

function inv3(m) {
  const a = m[0], b = m[1], c = m[2], d = m[4], e = m[5], f = m[6], g = m[8], hh = m[9], i = m[10];
  const A = e * i - f * hh, B = -(d * i - f * g), C = d * hh - e * g;
  const det = a * A + b * B + c * C;
  if (Math.abs(det) < 1e-12) return null;
  const k = 1 / det;
  return [
    A * k, -(b * i - c * hh) * k, (b * f - c * e) * k,
    B * k, (a * i - c * g) * k, -(a * f - c * d) * k,
    C * k, -(a * hh - b * g) * k, (a * e - b * d) * k,
  ];
}

function worldToLocalDelta(parentM, dx, dy) {
  const inv = inv3(parentM);
  if (inv) return [inv[0] * dx + inv[1] * dy, inv[3] * dx + inv[4] * dy, inv[6] * dx + inv[7] * dy];
  const a = parentM[0], b = parentM[1], c = parentM[4], d = parentM[5];
  const det = a * d - b * c;
  if (Math.abs(det) < 1e-12) return null;
  return [(d * dx - b * dy) / det, (-c * dx + a * dy) / det, 0];
}

function quatMul(a, b) {
  const [ax, ay, az, aw] = a, [bx, by, bz, bw] = b;
  return [
    aw * bx + ax * bw + ay * bz - az * by,
    aw * by - ax * bz + ay * bw + az * bx,
    aw * bz + ax * by - ay * bx + az * bw,
    aw * bw - ax * bx - ay * by - az * bz,
  ];
}

function quatAngleZ(q) {
  const [x, y, z, w] = quatUnit(q);
  const cx = 1 - 2 * (y * y + z * z), cy = 2 * (x * y + z * w);
  if (Math.hypot(cx, cy) < 1e-6) return null;
  let a = Math.atan2(cy, cx) * 180 / Math.PI;
  a = ((a % 360) + 360) % 360;
  if (a > 359.995) a = 0;
  return a;
}

function corners(mn, mx) {
  const out = [];
  for (const x of [mn[0], mx[0]]) for (const y of [mn[1], mx[1]]) for (const z of [mn[2], mx[2]]) out.push([x, y, z]);
  return out;
}

function hull(pts) {
  const p = pts.map((a) => [a[0], a[1]]).sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const u = [];
  for (const q of p) {
    const l = u[u.length - 1];
    if (!l || Math.abs(l[0] - q[0]) > 1e-9 || Math.abs(l[1] - q[1]) > 1e-9) u.push(q);
  }
  if (u.length <= 2) return u;
  const cross = (o, a, b) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
  const lo = [], up = [];
  for (const q of u) {
    while (lo.length >= 2 && cross(lo[lo.length - 2], lo[lo.length - 1], q) <= 0) lo.pop();
    lo.push(q);
  }
  for (let i = u.length - 1; i >= 0; i--) {
    const q = u[i];
    while (up.length >= 2 && cross(up[up.length - 2], up[up.length - 1], q) <= 0) up.pop();
    up.push(q);
  }
  lo.pop();
  up.pop();
  const r = lo.concat(up);
  return r.length >= 2 ? r : [u[0], u[u.length - 1]];
}

function polyArea(p) {
  if (p.length < 3) return 0;
  let a = 0;
  for (let i = 0; i < p.length; i++) {
    const [x1, y1] = p[i], [x2, y2] = p[(i + 1) % p.length];
    a += x1 * y2 - x2 * y1;
  }
  return Math.abs(a) / 2;
}

function aabbOf(p) {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const [x, y] of p) {
    if (x < x0) x0 = x;
    if (y < y0) y0 = y;
    if (x > x1) x1 = x;
    if (y > y1) y1 = y;
  }
  return [x0, y0, x1, y1];
}

function unionBox(a, b) {
  if (!a) return b ? [...b] : null;
  if (!b) return [...a];
  return [Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.max(a[2], b[2]), Math.max(a[3], b[3])];
}

function inPoly(x, y, p) {
  let inside = false;
  for (let i = 0, j = p.length - 1; i < p.length; j = i++) {
    const [xi, yi] = p[i], [xj, yj] = p[j];
    if ((yi > y) !== (yj > y) && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside;
  }
  return inside;
}

function distSeg(x, y, a, b) {
  const dx = b[0] - a[0], dy = b[1] - a[1];
  const L = dx * dx + dy * dy;
  const t = L ? Math.max(0, Math.min(1, ((x - a[0]) * dx + (y - a[1]) * dy) / L)) : 0;
  return Math.hypot(x - (a[0] + t * dx), y - (a[1] + t * dy));
}

function inBox(x, y, b) {
  return b && x >= b[0] && x <= b[2] && y >= b[1] && y <= b[3];
}

/* ---------- formatting ---------- */

function fmt(n, dp = 5) {
  if (typeof n !== 'number' || !Number.isFinite(n)) return String(n);
  const r = parseFloat(n.toFixed(dp));
  return Object.is(r, -0) ? '0' : String(r);
}

const fmtVec = (v, dp) => '(' + (v || []).map((x) => fmt(x, dp)).join(', ') + ')';
const shortPath = (p) => (p || '').replace(/^eaid:\/\/ui_package\//, '');
const baseName = (p) => (p || '').split(/[\\/]/).pop();

/* ---------- api ---------- */

async function api(path, body) {
  const opt = body === undefined ? {} : body instanceof ArrayBuffer
    ? { method: 'POST', body }
    : { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) };
  const r = await fetch(path, opt);
  const ct = r.headers.get('Content-Type') || '';
  const data = ct.includes('json') ? await r.json() : await r.arrayBuffer();
  if (!r.ok) {
    const err = new Error((data && data.error) || `HTTP ${r.status}`);
    err.status = r.status;
    if (data && data.opIndex !== undefined) err.opIndex = data.opIndex;
    throw err;
  }
  return data;
}

let toastTimer = null;
function toast(msg, kind) {
  const t = $('#toast');
  t.textContent = msg;
  t.className = 'toast' + (kind === 'err' ? ' err' : '');
  t.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { t.hidden = true; }, kind === 'err' ? 6000 : 2600);
}

/* ---------- scene structure helpers ---------- */

const E = (k) => S.els[k];
const isRoot = (k) => E(k) && E(k).parent === null;

function isSelfOrDesc(k, anc) {
  for (let c = k; c != null; c = E(c) && E(c).parent) if (c === anc) return true;
  return false;
}

function hiddenEl(e) {
  return S.hiddenTypes.has(e.type);
}

function param(e, name) {
  const p = e.params.find((x) => x.n === name);
  return p ? p.v : undefined;
}

/* ---------- compose ---------- */

function compose() {
  S.world.clear();
  S.paints = [];
  if (!S.model) return;
  const order = [];
  let dfs = 0;
  const visit = (k, pm, pl) => {
    const e = E(k);
    const m = mul(pm, trs(e.pos, e.rot, S.opts.reveal ? e.scale.map((v) => (Math.abs(v) < 1e-3 ? 1 : v)) : e.scale));
    const L = effectiveLayer(e, pl);
    const o = dfs++;
    const w = { k, m, pm, poly: null, aabb: null, area: 0, pivot: [m[3], m[7]], gaabb: null, ghosts: null, L };
    if (e.geoms.length || e.type === 'eText') {
      S.paints.push({ k, L, o, m, t: e.type, n: e.name, g: e.geoms.map((g) => visKey(g.resourceid, g.materialid)),
        box: e.box, a: param(e, 'halign'), c: param(e, 'textcase') });
    }
    if (e.box) {
      const pts = corners(e.box.min, e.box.max).map((c) => apply(m, c[0], c[1], c[2]));
      w.poly = hull(pts);
      w.aabb = aabbOf(w.poly);
      w.area = polyArea(w.poly);
      order.push(k);
    }
    S.world.set(k, w);
    for (const c of e.ch) visit(c, m, L);
    let g = w.aabb;
    for (const c of e.ch) g = unionBox(g, S.world.get(c).gaabb);
    w.gaabb = g;
    if (e.fileRef) {
      const gh = S.ghosts.get(ghostKey(e.fileRef));
      if (gh && gh.items) {
        const n = gh.items.length;
        w.ghosts = gh.items.map((it, i) => {
          const mm = mul(m, it.m);
          const poly = hull(corners(it.min, it.max).map((c) => apply(mm, c[0], c[1], c[2])));
          if ((it.g && it.g.length) || it.t === 'eText') {
            S.paints.push({ k, ghost: true, L: it.L == null ? L : it.L, o: o + (i + 1) / (n + 1), m: mm, t: it.t, n: it.n,
              g: (it.g || []).map(([r, mt]) => visKey(r, mt)), erp: it.e, box: { min: it.min, max: it.max }, a: it.a, c: it.c });
          }
          return { t: it.t, n: it.n, poly };
        });
      }
    }
  };
  for (const sc of S.model.scenes) visit(sc.root, ID4, null);
  order.sort((a, b) => S.world.get(b).area - S.world.get(a).area);
  S.drawOrder = order;
  S.paints.sort((a, b) => (a.L ?? 0) - (b.L ?? 0) || a.o - b.o);
}

function effectiveLayer(e, parent) {
  const v = param(e, 'eLayer.layer');
  if (typeof v !== 'number') return parent;
  return param(e, 'eLayer.layerIsOffset') && parent != null ? parent + v : v;
}

const visKey = (res, mat) => `${res}|${mat}`;

/* ---------- canvas ---------- */

const cv = $('#cv');
const ctx = cv.getContext('2d');
let W = 0, H = 0, DPR = 1, renderQueued = false;

function resize() {
  const r = cv.getBoundingClientRect();
  DPR = window.devicePixelRatio || 1;
  W = r.width;
  H = r.height;
  cv.width = Math.max(1, Math.round(W * DPR));
  cv.height = Math.max(1, Math.round(H * DPR));
  render();
}

const sx = (x) => (x - S.view.cx) * S.view.s + W / 2;
const sy = (y) => H / 2 - (y - S.view.cy) * S.view.s;
const wx = (px) => (px - W / 2) / S.view.s + S.view.cx;
const wy = (py) => S.view.cy - (py - H / 2) / S.view.s;

function queueRender() {
  if (renderQueued) return;
  renderQueued = true;
  requestAnimationFrame(() => { renderQueued = false; render(); });
}

function pathPoly(p) {
  ctx.beginPath();
  ctx.moveTo(sx(p[0][0]), sy(p[0][1]));
  for (let i = 1; i < p.length; i++) ctx.lineTo(sx(p[i][0]), sy(p[i][1]));
  if (p.length > 2) ctx.closePath();
}

function withAlpha(color, a) {
  ctx.globalAlpha = a;
  return color;
}

function drawGrid() {
  const target = 70 / S.view.s;
  const p10 = Math.pow(10, Math.floor(Math.log10(target)));
  const step = [1, 2, 5, 10].map((k) => k * p10).find((v) => v >= target);
  const x0 = Math.floor(wx(0) / step) * step, x1 = wx(W);
  const y0 = Math.floor(wy(H) / step) * step, y1 = wy(0);
  ctx.lineWidth = 1;
  ctx.strokeStyle = '#161c23';
  ctx.beginPath();
  for (let x = x0; x <= x1; x += step) { const X = Math.round(sx(x)) + 0.5; ctx.moveTo(X, 0); ctx.lineTo(X, H); }
  for (let y = y0; y <= y1; y += step) { const Y = Math.round(sy(y)) + 0.5; ctx.moveTo(0, Y); ctx.lineTo(W, Y); }
  ctx.stroke();
  ctx.strokeStyle = '#26303b';
  ctx.beginPath();
  const X = Math.round(sx(0)) + 0.5, Y = Math.round(sy(0)) + 0.5;
  ctx.moveTo(X, 0); ctx.lineTo(X, H); ctx.moveTo(0, Y); ctx.lineTo(W, Y);
  ctx.stroke();
}

function drawFrame() {
  const x0 = sx(-SCREEN.w / 2), y0 = sy(SCREEN.h / 2), x1 = sx(SCREEN.w / 2), y1 = sy(-SCREEN.h / 2);
  ctx.fillStyle = 'rgba(255,255,255,0.018)';
  ctx.fillRect(x0, y0, x1 - x0, y1 - y0);
  ctx.setLineDash([6, 5]);
  ctx.strokeStyle = '#3a4554';
  ctx.lineWidth = 1;
  ctx.strokeRect(x0, y0, x1 - x0, y1 - y0);
  ctx.setLineDash([]);
  ctx.fillStyle = '#4b5868';
  ctx.font = '11px ui-monospace, monospace';
  ctx.textBaseline = 'bottom';
  ctx.fillText('1920x1080 screen (19.2 x 10.8 units)', x0, y0 - 4);
}

const VISUALS = {
  box(e, w, state) {
    const col = typeColor(e.type);
    const sel = state === 'sel', hov = state === 'hov';
    if (!state && S.opts.textures) {
      ctx.globalAlpha = 0.38;
      ctx.strokeStyle = col;
      ctx.lineWidth = 1;
      if (e.box.src === 'text') ctx.setLineDash([4, 3]);
      pathPoly(w.poly);
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.globalAlpha = 1;
      return;
    }
    if (w.poly.length >= 3 && !(S.opts.textures && state)) {
      pathPoly(w.poly);
      ctx.fillStyle = withAlpha(col, sel ? 0.22 : hov ? 0.16 : e.geoms.length ? 0.09 : 0.05);
      ctx.fill();
    }
    ctx.globalAlpha = sel ? 1 : hov ? 0.95 : 0.75;
    ctx.strokeStyle = sel ? '#ffffff' : col;
    ctx.lineWidth = sel ? 2 : hov ? 1.6 : e.type === 'eXRef' ? 1.3 : 1;
    if (e.box.src === 'text') ctx.setLineDash([4, 3]);
    pathPoly(w.poly);
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.globalAlpha = 1;
  },
};

function visualFor() {
  return VISUALS.box;
}

function drawPivot(e, w, state) {
  const X = sx(w.pivot[0]), Y = sy(w.pivot[1]);
  const col = state === 'sel' ? '#ffffff' : typeColor(e.type);
  ctx.strokeStyle = col;
  ctx.fillStyle = col;
  ctx.lineWidth = state ? 1.6 : 1;
  ctx.globalAlpha = state ? 1 : 0.7;
  ctx.beginPath();
  if (e.type === 'ePoint' || e.type === 'eAnchor') {
    ctx.moveTo(X, Y - 5); ctx.lineTo(X + 5, Y); ctx.lineTo(X, Y + 5); ctx.lineTo(X - 5, Y); ctx.closePath();
  } else {
    ctx.arc(X, Y, 3.5, 0, Math.PI * 2);
  }
  ctx.stroke();
  ctx.globalAlpha = 1;
}

function drawGizmo(w) {
  const X = sx(w.pivot[0]), Y = sy(w.pivot[1]);
  const axis = (ix, iy, col) => {
    const len = Math.hypot(w.m[ix], w.m[iy]);
    if (len < 1e-9) return;
    const dx = (w.m[ix] / len) * 38, dy = -(w.m[iy] / len) * 38;
    ctx.strokeStyle = col;
    ctx.lineWidth = 2;
    ctx.beginPath(); ctx.moveTo(X, Y); ctx.lineTo(X + dx, Y + dy); ctx.stroke();
    const a = Math.atan2(dy, dx);
    ctx.beginPath();
    ctx.moveTo(X + dx, Y + dy);
    ctx.lineTo(X + dx - 7 * Math.cos(a - 0.4), Y + dy - 7 * Math.sin(a - 0.4));
    ctx.lineTo(X + dx - 7 * Math.cos(a + 0.4), Y + dy - 7 * Math.sin(a + 0.4));
    ctx.closePath();
    ctx.fillStyle = col;
    ctx.fill();
  };
  axis(0, 4, '#f87171');
  axis(1, 5, '#4ade80');
  ctx.fillStyle = '#fff';
  ctx.beginPath(); ctx.arc(X, Y, 3, 0, Math.PI * 2); ctx.fill();
}

function labelFor(e) {
  const a = quatAngleZ(e.rot);
  const rot = a !== null && a > 0.01 ? ` ${fmt(a, 1)}°` : '';
  return [e.name, `${e.type}${rot}`];
}

function drawLabels() {
  const mode = S.opts.labels;
  if (mode === 'none') return;
  ctx.font = '11px ui-monospace, monospace';
  ctx.textBaseline = 'top';
  const placed = [];
  const tryPlace = (k, force) => {
    const e = E(k), w = S.world.get(k);
    if (!e || !w || hiddenEl(e)) return;
    const box = w.aabb || [w.pivot[0], w.pivot[1], w.pivot[0], w.pivot[1]];
    const X0 = sx(box[0]), Y0 = sy(box[3]), X1 = sx(box[2]), Y1 = sy(box[1]);
    if (!force) {
      if (mode === 'sel') return;
      if (!w.aabb) return;
      if (mode === 'auto' && (X1 - X0 < 64 || Y1 - Y0 < 18)) return;
    }
    const [name, sub] = labelFor(e);
    const tw = Math.max(ctx.measureText(name).width, ctx.measureText(sub).width * 0.9) + 8;
    const r = w.aabb ? [X0 + 2, Y0 + 2, X0 + 2 + tw, Y0 + 30] : [X0 + 8, Y0 - 14, X0 + 8 + tw, Y0 + 14];
    if (!force && placed.some((p) => r[0] < p[2] && r[2] > p[0] && r[1] < p[3] && r[3] > p[1])) return;
    placed.push(r);
    ctx.fillStyle = force ? 'rgba(8,10,13,0.92)' : 'rgba(8,10,13,0.72)';
    ctx.fillRect(r[0], r[1], r[2] - r[0], 28);
    ctx.fillStyle = force ? '#ffffff' : '#dfe5ec';
    ctx.fillText(name, r[0] + 4, r[1] + 2);
    ctx.font = '10px ui-monospace, monospace';
    ctx.fillStyle = typeColor(e.type);
    ctx.fillText(sub, r[0] + 4, r[1] + 15);
    ctx.font = '11px ui-monospace, monospace';
  };
  if (S.sel) tryPlace(S.sel, true);
  if (S.hov && S.hov !== S.sel) tryPlace(S.hov, true);
  for (const k of S.drawOrder) if (k !== S.sel && k !== S.hov) tryPlace(k, mode === 'all');
}

function render() {
  ctx.setTransform(DPR, 0, 0, DPR, 0, 0);
  ctx.fillStyle = '#0b0e12';
  ctx.fillRect(0, 0, W, H);
  if (S.opts.grid) drawGrid();
  if (S.opts.frame) drawFrame();
  if (!S.model) return;

  if (S.opts.textures) drawPaints();
  const outlines = S.opts.outlines || !S.opts.textures;

  if (S.opts.xrefs && outlines) {
    ctx.lineWidth = 0.8;
    for (const [k, w] of S.world) {
      if (!w.ghosts || hiddenEl(E(k))) continue;
      const strong = S.sel && isSelfOrDesc(k, S.sel);
      for (const g of w.ghosts) {
        if (S.hiddenTypes.has(g.t) || g.poly.length < 2) continue;
        ctx.strokeStyle = typeColor(g.t);
        ctx.globalAlpha = (strong ? 0.6 : 0.3) * (S.opts.textures ? 0.45 : 1);
        pathPoly(g.poly);
        ctx.stroke();
      }
    }
    ctx.globalAlpha = 1;
  }

  for (const k of S.drawOrder) {
    const e = E(k);
    if (!outlines || hiddenEl(e) || k === S.sel || k === S.hov) continue;
    visualFor(e)(e, S.world.get(k), null);
  }
  if (S.opts.pivots && outlines) {
    for (const [k, w] of S.world) {
      const e = E(k);
      if (w.poly || hiddenEl(e) || e.parent === null || k === S.sel || k === S.hov) continue;
      drawPivot(e, w, null);
    }
  }
  for (const [k, state] of [[S.hov, 'hov'], [S.sel, 'sel']]) {
    if (!k || !E(k) || (state === 'hov' && k === S.sel)) continue;
    const e = E(k), w = S.world.get(k);
    if (state === 'sel' && w.gaabb && (!w.aabb || w.gaabb.join() !== w.aabb.join())) {
      ctx.setLineDash([5, 4]);
      ctx.strokeStyle = 'rgba(255,255,255,0.55)';
      ctx.lineWidth = 1;
      ctx.strokeRect(sx(w.gaabb[0]), sy(w.gaabb[3]), (w.gaabb[2] - w.gaabb[0]) * S.view.s, (w.gaabb[3] - w.gaabb[1]) * S.view.s);
      ctx.setLineDash([]);
    }
    if (w.poly) visualFor(e)(e, w, state);
    else drawPivot(e, w, state);
    if (state === 'sel') drawGizmo(w);
  }
  if (outlines || S.opts.labels === 'all') drawLabels();
  else if (S.sel || S.hov) { const keep = S.opts.labels; S.opts.labels = 'sel'; drawLabels(); S.opts.labels = keep; }
}

/* ---------- textured preview ---------- */

const NO_DRAW_SHADERS = new Set(['ui_mask_texture.nefx2', 'ui_font_sdf.nefx2', 'ui_font_sdf_shadowed.nefx2']);
const GRADIENT_SHADERS = new Set(['ui_gradient.nefx2', 'ui_colour_border_gradient.nefx2']);
const patterns = new WeakMap();
const texQueue = [];
let texActive = 0;
let visLoading = false;

function lin2srgb(c) {
  c = Math.max(0, Math.min(1, c));
  return c <= 0.0031308 ? 12.92 * c : 1.055 * Math.pow(c, 1 / 2.4) - 0.055;
}

function cssColour(rgb, a) {
  const f = (x) => Math.round(lin2srgb(x) * 255);
  return `rgba(${f(rgb[0])},${f(rgb[1])},${f(rgb[2])},${Math.max(0, Math.min(1, a))})`;
}

function matColour(p) {
  const c = p.me_colour || [1, 1, 1, 1], t = p.me_finalColourTint || [1, 1, 1, 1];
  return [c[0] * t[0], c[1] * t[1], c[2] * t[2], Math.max(0, Math.min(1, c[3] * t[3]))];
}

function affineMap(uv, sc) {
  const [u0, v0] = uv[0], [u1, v1] = uv[1], [u2, v2] = uv[2];
  const du1 = u1 - u0, dv1 = v1 - v0, du2 = u2 - u0, dv2 = v2 - v0;
  const det = du1 * dv2 - du2 * dv1;
  if (Math.abs(det) < 1e-12) return null;
  const dx1 = sc[1][0] - sc[0][0], dy1 = sc[1][1] - sc[0][1], dx2 = sc[2][0] - sc[0][0], dy2 = sc[2][1] - sc[0][1];
  const a = (dx1 * dv2 - dx2 * dv1) / det, c = (dx2 * du1 - dx1 * du2) / det;
  const b = (dy1 * dv2 - dy2 * dv1) / det, d = (dy2 * du1 - dy1 * du2) / det;
  return [a, b, c, d, sc[0][0] - a * u0 - c * v0, sc[0][1] - b * u0 - d * v0];
}

function roundedPath(b, A, radii) {
  const su = Math.hypot(A[0], A[1]) || 1, sv = Math.hypot(A[2], A[3]) || 1;
  const [x0, y0, x1, y1] = b;
  const k = S.view.s / 100;
  const r = radii.map((p) => [Math.min((p || 0) * k / su, (x1 - x0) / 2), Math.min((p || 0) * k / sv, (y1 - y0) / 2)]);
  const [tl, tr, br, bl] = r;
  ctx.moveTo(x0 + tl[0], y0);
  ctx.lineTo(x1 - tr[0], y0);
  if (tr[0] > 0) ctx.ellipse(x1 - tr[0], y0 + tr[1], tr[0], tr[1], 0, -Math.PI / 2, 0);
  ctx.lineTo(x1, y1 - br[1]);
  if (br[0] > 0) ctx.ellipse(x1 - br[0], y1 - br[1], br[0], br[1], 0, 0, Math.PI / 2);
  ctx.lineTo(x0 + bl[0], y1);
  if (bl[0] > 0) ctx.ellipse(x0 + bl[0], y1 - bl[1], bl[0], bl[1], 0, Math.PI / 2, Math.PI);
  ctx.lineTo(x0, y0 + tl[1]);
  if (tl[0] > 0) ctx.ellipse(x0 + tl[0], y0 + tl[1], tl[0], tl[1], 0, Math.PI, Math.PI * 1.5);
  ctx.closePath();
}

function fillMapped(scr, uv, idx, sw, sh, setFill, opts = {}) {
  const uvp = uv.map(([u, v]) => [u * sw, v * sh]);
  const fillWith = (A, poly, whole) => {
    ctx.save();
    ctx.setTransform(DPR * A[0], DPR * A[1], DPR * A[2], DPR * A[3], DPR * A[4], DPR * A[5]);
    ctx.beginPath();
    if (whole && opts.radii) roundedPath(aabbOf(uvp), A, opts.radii);
    else {
      poly.forEach((q, i) => (i ? ctx.lineTo(q[0], q[1]) : ctx.moveTo(q[0], q[1])));
      ctx.closePath();
    }
    if (setFill) {
      setFill(A);
      ctx.fill();
    }
    if (whole && opts.border) {
      const su = Math.hypot(A[0], A[1]) || 1, sv = Math.hypot(A[2], A[3]) || 1;
      ctx.globalAlpha = opts.border.alpha;
      ctx.strokeStyle = opts.border.colour;
      ctx.lineWidth = Math.max(1, opts.border.px * S.view.s / 100) * 2 / (su + sv);
      ctx.stroke();
    }
    ctx.restore();
  };
  const t0 = [idx[0], idx[1], idx[2]];
  const A = affineMap(t0.map((i) => uvp[i]), t0.map((i) => scr[i]));
  if (A && uvp.every((q, i) => Math.hypot(A[0] * q[0] + A[2] * q[1] + A[4] - scr[i][0], A[1] * q[0] + A[3] * q[1] + A[5] - scr[i][1]) < 0.75)) {
    fillWith(A, hull(uvp), true);
    return true;
  }
  let any = false;
  for (let i = 0; i + 2 < idx.length; i += 3) {
    const t = [idx[i], idx[i + 1], idx[i + 2]];
    const At = affineMap(t.map((j) => uvp[j]), t.map((j) => scr[j]));
    if (At) { fillWith(At, t.map((j) => uvp[j])); any = true; }
  }
  return any;
}

function tintedCanvas(name, t, rgb) {
  if (rgb[0] > 0.999 && rgb[1] > 0.999 && rgb[2] > 0.999) return t.canvas;
  const key = name + '|' + rgb.map((x) => x.toFixed(3)).join(',');
  let c = S.tints.get(key);
  if (c) return c;
  c = document.createElement('canvas');
  c.width = t.w;
  c.height = t.h;
  const g = c.getContext('2d');
  g.drawImage(t.canvas, 0, 0);
  g.globalCompositeOperation = 'multiply';
  g.fillStyle = cssColour(rgb, 1);
  g.fillRect(0, 0, t.w, t.h);
  g.globalCompositeOperation = 'destination-in';
  g.drawImage(t.canvas, 0, 0);
  S.tints.set(key, c);
  return c;
}

function patternOf(c) {
  let p = patterns.get(c);
  if (!p) { p = ctx.createPattern(c, 'repeat'); patterns.set(c, p); }
  return p;
}

function gradientStops(prm, base) {
  const stops = [];
  for (let i = 1; i <= 8; i++) {
    const c = prm[`me_${base}Colour${i}`];
    if (!Array.isArray(c)) continue;
    const en = prm[`me_enable${base[0].toUpperCase()}${base.slice(1)}Colour${i}`];
    if (i > 1 && en === 0) continue;
    const pos = prm[`me_${base}ColourLerp${i}`];
    if (typeof pos !== 'number' || pos < 0 || pos > 1) continue;
    stops.push([pos, c]);
  }
  return stops.sort((a, b) => a[0] - b[0]);
}

function drawGeom(p, vis) {
  if (!vis || !vis.verts || !vis.idx || vis.idx.length < 3 || vis.matError) return;
  const sh = vis.shader || '';
  if (NO_DRAW_SHADERS.has(sh)) return;
  const prm = vis.params || {};
  const glass = sh.startsWith('ui_glass');
  let col = matColour(prm);
  if (glass && (sh === 'ui_glass_alpha.nefx2' || col[3] < 0.05)) col = [0.012, 0.016, 0.03, 0.32 * ((prm.me_finalColourTint || [1, 1, 1, 1])[3])];
  const alpha = glass ? Math.max(col[3], 0.3) : col[3];
  const hasBorder = !!borderOf(sh, prm, prm.me_finalColourTint || [1, 1, 1, 1]);
  const hasGradient = GRADIENT_SHADERS.has(sh) || sh === 'ui_radial_gradient.nefx2' || sh === 'ui_vignette.nefx2';
  if (alpha < 0.004 && !hasBorder && !hasGradient) return;
  const scr = vis.verts.map((v) => { const w = apply(p.m, v[0], v[1], v[2]); return [sx(w[0]), sy(w[1])]; });
  const bb = aabbOf(scr);
  if (bb[2] - bb[0] < 0.4 && bb[3] - bb[1] < 0.4) return;
  if (bb[2] < 0 || bb[0] > W || bb[3] < 0 || bb[1] > H) return;
  const texName = (vis.textures || [])[0];
  const tint = prm.me_finalColourTint || [1, 1, 1, 1];
  const radii = 'me_curvePixelsTL' in prm ? [prm.me_curvePixelsTL, prm.me_curvePixelsTR, prm.me_curvePixelsBR, prm.me_curvePixelsBL] : null;
  const border = borderOf(sh, prm, tint);
  const opts = { radii, border };
  if (!vis.uv) {
    ctx.globalAlpha = alpha;
    ctx.fillStyle = cssColour(col, 1);
    pathPoly(hull(scr).map((q) => [wx(q[0]), wy(q[1])]));
    ctx.fill();
    ctx.globalAlpha = 1;
    return;
  }
  if (texName) {
    const t = S.tex.get(texName);
    if (!t) ensureTexture(texName, vis.erp);
    if (t && t.canvas) {
      ctx.globalAlpha = alpha;
      const img = tintedCanvas(texName, t, col);
      fillMapped(scr, vis.uv, vis.idx, t.w, t.h, () => { ctx.fillStyle = patternOf(img); }, { radii });
    }
  } else if (sh === 'ui_circular_fill.nefx2') {
    const start = (prm.me_startAngle ?? 0) * Math.PI * 2 - Math.PI / 2;
    const sweep = Math.max(0.001, Math.min(1, prm.me_fillMax ?? 1)) * Math.PI * 2;
    const ccw = prm.me_fillClockwise === 0;
    ctx.globalAlpha = alpha;
    fillMapped(scr, vis.uv, vis.idx, 1, 1, () => {
      ctx.strokeStyle = cssColour(col, 1);
      ctx.lineWidth = 0.09;
      ctx.beginPath();
      ctx.arc(0.5, 0.5, 0.44, start, ccw ? start - sweep : start + sweep, ccw);
      ctx.stroke();
      ctx.beginPath();
    });
  } else {
    const radial = sh === 'ui_radial_gradient.nefx2' || sh === 'ui_vignette.nefx2';
    let stops = null;
    if (sh === 'ui_vignette.nefx2') stops = [[0.45, [col[0], col[1], col[2], 0]], [1, [col[0], col[1], col[2], 1]]];
    else if (radial) stops = gradientStops(prm, 'radialGradient');
    else if (GRADIENT_SHADERS.has(sh) && prm.me_enableGradient !== 0) stops = gradientStops(prm, 'gradient');
    if (stops && stops.length) {
      const base = prm.me_colour && sh !== 'ui_gradient.nefx2' ? prm.me_colour : [1, 1, 1, 1];
      ctx.globalAlpha = Math.min(1, tint[3] * base[3]);
      fillMapped(scr, vis.uv, vis.idx, 1, 1, () => {
        let g;
        if (radial) {
          g = ctx.createRadialGradient(0.5, 0.5, 0, 0.5, 0.5, sh === 'ui_vignette.nefx2' ? 0.75 : 0.5);
        } else {
          const ang = ((prm.me_gradientAngleDegrees || 0) * Math.PI) / 180, cx = Math.cos(ang) / 2, cy = Math.sin(ang) / 2;
          g = ctx.createLinearGradient(0.5 - cx, 0.5 - cy, 0.5 + cx, 0.5 + cy);
        }
        for (const [pos, c] of stops) g.addColorStop(pos, cssColour([c[0] * tint[0], c[1] * tint[1], c[2] * tint[2]], c[3]));
        ctx.fillStyle = g;
      }, opts);
    } else {
      ctx.globalAlpha = alpha;
      fillMapped(scr, vis.uv, vis.idx, 1, 1, alpha >= 0.004 ? () => { ctx.fillStyle = cssColour(col, 1); } : null, opts);
    }
  }
  ctx.globalAlpha = 1;
}

function borderOf(sh, prm, tint) {
  let c = null;
  if (sh.includes('border_gradient') || sh === 'ui_colour_border.nefx2' || sh === 'ui_texture_border.nefx2') {
    const sides = ['Top', 'Bottom', 'Left', 'Right'].map((k) => prm[`me_enable${k}Border`]);
    if (sides.every((v) => v === 0)) return null;
    c = prm.me_borderColour;
  } else if (sh === 'ui_colour_rounded_border.nefx2') {
    c = prm.me_roundedBorderColour;
  }
  if (!Array.isArray(c)) return null;
  const a = c[3] * tint[3] * (prm.me_borderIntensity ?? 1);
  if (a < 0.01) return null;
  return { colour: cssColour([c[0] * tint[0], c[1] * tint[1], c[2] * tint[2]], 1), alpha: Math.min(1, a), px: prm.me_borderThickness || 1 };
}

function drawTextPaint(p) {
  if (!p.box) return;
  const pts = corners(p.box.min, p.box.max).map((c) => { const w = apply(p.m, c[0], c[1], c[2]); return [sx(w[0]), sy(w[1])]; });
  const bb = aabbOf(pts);
  const hpx = bb[3] - bb[1];
  if (hpx < 5 || bb[2] < 0 || bb[0] > W || bb[3] < 0 || bb[1] > H) return;
  let txt = p.n.replace(/_/g, ' ');
  if (String(p.c || '').toLowerCase() === 'upper') txt = txt.toUpperCase();
  ctx.save();
  ctx.beginPath();
  ctx.rect(bb[0], bb[1] - hpx * 0.3, bb[2] - bb[0], hpx * 1.6);
  ctx.clip();
  ctx.font = `600 ${Math.max(6, hpx * 0.74)}px "Titillium Web", "Segoe UI", system-ui, sans-serif`;
  ctx.fillStyle = 'rgba(236,240,245,0.78)';
  ctx.textBaseline = 'middle';
  const al = String(p.a || 'left').toLowerCase();
  ctx.textAlign = al.startsWith('right') ? 'right' : al.startsWith('cent') ? 'center' : 'left';
  const x = ctx.textAlign === 'right' ? bb[2] : ctx.textAlign === 'center' ? (bb[0] + bb[2]) / 2 : bb[0];
  ctx.fillText(txt, x, (bb[1] + bb[3]) / 2);
  ctx.restore();
}

function drawPaints() {
  for (const p of S.paints) {
    const owner = E(p.k);
    if (!owner || hiddenEl(owner) || S.hiddenTypes.has(p.t)) continue;
    if (p.ghost && !S.opts.xrefs) continue;
    for (const key of p.g) drawGeom(p, S.vis.get(key));
    if (p.t === 'eText' && S.opts.text) drawTextPaint(p);
  }
}

async function loadVisuals() {
  if (!S.opts.textures || visLoading || !S.model) return;
  const need = new Map();
  for (const p of S.paints) {
    for (const key of p.g) {
      if (S.vis.has(key) || need.has(key)) continue;
      const [res, mat] = key.split('|');
      need.set(key, { res, mat: mat === 'null' || mat === 'undefined' ? null : mat, erp: p.erp || (S.origin && S.origin.erp) || null });
    }
  }
  if (!need.size) return;
  visLoading = true;
  for (const k of need.keys()) S.vis.set(k, { loading: true });
  try {
    const r = await api('/api/visuals', { items: [...need.values()] });
    for (const [k, v] of Object.entries(r.visuals)) S.vis.set(k, v);
  } catch (err) {
    for (const k of need.keys()) S.vis.delete(k);
    toast('Loading visuals failed: ' + err.message, 'err');
  }
  visLoading = false;
  queueRender();
  if (S.paints.some((p) => p.g.some((k) => !S.vis.has(k)))) loadVisuals();
}

function ensureTexture(name, erp) {
  if (S.tex.has(name)) return;
  S.tex.set(name, { loading: true });
  texQueue.push([name, erp]);
  pumpTextures();
}

function pumpTextures() {
  while (texActive < 6 && texQueue.length) {
    const [name, erp] = texQueue.shift();
    texActive++;
    fetch(`/api/texture?name=${encodeURIComponent(name)}${erp ? '&erp=' + encodeURIComponent(erp) : ''}`)
      .then(async (r) => {
        if (!r.ok) throw new Error((await r.json()).error || `HTTP ${r.status}`);
        const w = Number(r.headers.get('X-Tex-Width')), hh = Number(r.headers.get('X-Tex-Height'));
        const px = decodeBC7(await r.arrayBuffer(), w, hh);
        const c = document.createElement('canvas');
        c.width = w;
        c.height = hh;
        c.getContext('2d').putImageData(new ImageData(px, w, hh), 0, 0);
        S.tex.set(name, { canvas: c, w, h: hh });
      })
      .catch((err) => S.tex.set(name, { error: err.message }))
      .finally(() => { texActive--; pumpTextures(); queueRender(); });
  }
}

function textureStats() {
  let ok = 0, all = 0;
  for (const t of S.tex.values()) { all++; if (t.canvas) ok++; }
  return all ? `textures ${ok}/${all}` : '';
}

/* ---------- picking ---------- */

function pickAll(x, y) {
  const tol = 6 / S.view.s;
  const out = [];
  for (const [k, w] of S.world) {
    const e = E(k);
    if (e.parent === null || hiddenEl(e)) continue;
    if (w.poly) {
      const hit = w.poly.length >= 3 ? inPoly(x, y, w.poly) : w.poly.length === 2 && distSeg(x, y, w.poly[0], w.poly[1]) < tol;
      if (hit) out.push({ k, area: w.area, depth: e.depth });
    } else if (S.opts.pivots && Math.hypot(x - w.pivot[0], y - w.pivot[1]) < tol) {
      out.push({ k, area: -1, depth: e.depth });
    }
  }
  out.sort((a, b) => a.area - b.area || b.depth - a.depth);
  return out.map((o) => o.k);
}

/* ---------- view ---------- */

function sceneBounds(keys) {
  let b = null;
  for (const k of keys) {
    const w = S.world.get(k);
    if (!w) continue;
    b = unionBox(b, w.gaabb || [w.pivot[0], w.pivot[1], w.pivot[0], w.pivot[1]]);
    if (S.opts.xrefs) {
      const stack = [k];
      while (stack.length) {
        const c = stack.pop();
        const cw = S.world.get(c);
        if (cw && cw.ghosts) for (const g of cw.ghosts) if (g.poly.length) b = unionBox(b, aabbOf(g.poly));
        stack.push(...E(c).ch);
      }
    }
  }
  return b;
}

function fitTo(b, pad = 1.12) {
  if (!b || !W) return;
  let bw = b[2] - b[0], bh = b[3] - b[1];
  if (bw < 1e-3 && bh < 1e-3) { bw = 2; bh = 2; }
  S.view.s = Math.min(W / Math.max(bw * pad, 1e-3), H / Math.max(bh * pad, 1e-3));
  S.view.s = Math.max(2, Math.min(40000, S.view.s));
  S.view.cx = (b[0] + b[2]) / 2;
  S.view.cy = (b[1] + b[3]) / 2;
  queueRender();
}

function fitScene() {
  if (!S.model) return fitTo([-SCREEN.w / 2, -SCREEN.h / 2, SCREEN.w / 2, SCREEN.h / 2]);
  fitTo(sceneBounds(S.model.scenes.map((s) => s.root)) || [-SCREEN.w / 2, -SCREEN.h / 2, SCREEN.w / 2, SCREEN.h / 2]);
}

function fitSelection() {
  if (S.sel) fitTo(sceneBounds([S.sel]), 1.6);
}

/* ---------- selection ---------- */

function select(k, opts = {}) {
  if (k && !E(k)) k = null;
  const changed = S.sel !== k;
  S.sel = k;
  if (k) {
    let p = E(k).parent, expanded = false;
    while (p != null) {
      if (S.collapsed.has(p)) { S.collapsed.delete(p); expanded = true; }
      p = E(p).parent;
    }
    if (expanded) renderTree();
  }
  syncTreeClasses(opts.scroll !== false);
  if (changed || opts.force) renderInspector();
  updateButtons();
  queueRender();
}

function setHover(k) {
  if (S.hov === k) return;
  S.hov = k;
  syncTreeClasses(false);
  hudStatus();
  queueRender();
}

/* ---------- tree ---------- */

const treeEl = $('#tree');

function defaultCollapse() {
  S.collapsed.clear();
  const n = Object.keys(S.els).length;
  if (n <= 260) return;
  for (const e of Object.values(S.els)) if (e.depth >= 2 && e.ch.length) S.collapsed.add(e.k);
}

function renderTree() {
  treeEl.textContent = '';
  if (!S.model) return;
  const q = $('#tree-filter').value.trim().toLowerCase();
  let show = null;
  if (q) {
    show = new Set();
    for (const e of Object.values(S.els)) {
      if (e.name.toLowerCase().includes(q) || e.type.toLowerCase() === q) {
        for (let c = e.k; c != null; c = E(c).parent) show.add(c);
      }
    }
  }
  const frag = document.createDocumentFragment();
  const add = (k) => {
    const e = E(k);
    if (show && !show.has(k)) return;
    const open = !S.collapsed.has(k) || !!q;
    const a = quatAngleZ(e.rot);
    const row = h('div', { class: 'row' + (hiddenEl(e) ? ' dim' : ''), 'data-k': k, role: 'treeitem', style: `padding-left:${4 + e.depth * 13}px` },
      h('span', { class: 'caret', 'data-caret': '1' }, e.ch.length ? (open ? '▾' : '▸') : ''),
      h('span', { class: 'dot', style: `background:${typeColor(e.type)}` }),
      h('span', { class: 'nm' }, e.name),
      h('span', { class: 'ty' }, e.type),
      a !== null && a > 0.01 ? h('span', { class: 'badge', title: 'Z rotation' }, `${fmt(a, 1)}°`) : null,
      e.fileRef ? h('span', { class: 'badge', title: e.fileRef }, 'xref') : null,
      !e.idOk ? h('span', { class: 'badge bad', title: 'elementId != djb2(path)' }, 'id!') : null,
      S.clonedPaths.has(e.path) ? h('span', { class: 'badge new' }, 'new') : null);
    frag.append(row);
    if (open) for (const c of e.ch) add(c);
  };
  for (const sc of S.model.scenes) add(sc.root);
  treeEl.append(frag);
  syncTreeClasses(false);
}

function syncTreeClasses(scroll) {
  for (const r of treeEl.querySelectorAll('.row.sel, .row.hov')) r.classList.remove('sel', 'hov');
  if (S.hov) {
    const r = treeEl.querySelector(`[data-k="${S.hov}"]`);
    if (r) r.classList.add('hov');
  }
  if (S.sel) {
    const r = treeEl.querySelector(`[data-k="${S.sel}"]`);
    if (r) {
      r.classList.add('sel');
      if (scroll) r.scrollIntoView({ block: 'nearest' });
    }
  }
}

treeEl.addEventListener('click', (ev) => {
  const row = ev.target.closest('.row');
  if (!row) return;
  const k = row.dataset.k;
  if (ev.target.dataset.caret && E(k).ch.length) {
    if (S.collapsed.has(k)) S.collapsed.delete(k); else S.collapsed.add(k);
    renderTree();
    return;
  }
  select(k, { scroll: false });
});
treeEl.addEventListener('dblclick', (ev) => {
  const row = ev.target.closest('.row');
  if (row) { select(row.dataset.k); fitSelection(); }
});
treeEl.addEventListener('mousemove', (ev) => {
  const row = ev.target.closest('.row');
  setHover(row ? row.dataset.k : null);
});
treeEl.addEventListener('mouseleave', () => setHover(null));
$('#tree-filter').addEventListener('input', renderTree);

/* ---------- legend ---------- */

function renderLegend() {
  const lg = $('#legend');
  lg.textContent = '';
  if (!S.model) return;
  const counts = {};
  for (const e of Object.values(S.els)) counts[e.type] = (counts[e.type] || 0) + 1;
  for (const t of Object.keys(counts).sort((a, b) => counts[b] - counts[a])) {
    const chip = h('span', { class: 'chip' + (S.hiddenTypes.has(t) ? ' off' : ''), title: 'Click to show/hide' },
      h('span', { class: 'sw', style: `background:${typeColor(t)}` }), t, h('span', { class: 'n' }, counts[t]));
    chip.addEventListener('click', () => {
      if (S.hiddenTypes.has(t)) S.hiddenTypes.delete(t); else S.hiddenTypes.add(t);
      renderLegend();
      renderTree();
      queueRender();
    });
    lg.append(chip);
  }
}

/* ---------- inspector ---------- */

const insp = $('#inspector');

function numInput(value, onCommit, opts = {}) {
  const inp = h('input', { class: 'input num', value: fmt(value, 6), spellcheck: 'false', inputmode: 'decimal', title: opts.title });
  inp.dataset.orig = inp.value;
  const commit = () => {
    if (inp.value === inp.dataset.orig) return;
    const v = Number(inp.value);
    if (!Number.isFinite(v)) { inp.classList.add('bad'); return; }
    inp.classList.remove('bad', 'dirty');
    inp.dataset.orig = inp.value;
    onCommit(v);
  };
  inp.addEventListener('input', () => inp.classList.toggle('dirty', inp.value !== inp.dataset.orig));
  inp.addEventListener('keydown', (ev) => {
    if (ev.key === 'Enter') { commit(); inp.blur(); }
    if (ev.key === 'Escape') { inp.value = inp.dataset.orig; inp.classList.remove('bad', 'dirty'); inp.blur(); }
    if (ev.key === 'ArrowUp' || ev.key === 'ArrowDown') {
      const step = (ev.shiftKey ? 0.1 : 0.01) * (ev.key === 'ArrowUp' ? 1 : -1) * (opts.stepScale || 1);
      const v = Number(inp.value);
      if (Number.isFinite(v)) { inp.value = fmt(v + step, 6); commit(); }
      ev.preventDefault();
    }
  });
  inp.addEventListener('blur', commit);
  return inp;
}

function vecRow(label, vals, n, onCommit, cls = '') {
  const axes = ['x', 'y', 'z', 'w'];
  const row = h('div', { class: 'vec' + (n === 4 ? ' four' : '') + cls }, h('label', {}, label));
  const cur = [...vals];
  for (let i = 0; i < n; i++) {
    row.append(h('div', { class: `axis ${axes[i]}` }, numInput(vals[i], (v) => { cur[i] = v; onCommit([...cur]); })));
  }
  return row;
}

function sec(title, ...kids) {
  return h('div', { class: 'sec' }, h('h3', {}, ...[].concat(title)), ...kids);
}

function kv(...pairs) {
  const d = h('div', { class: 'kv' });
  for (const [k, v] of pairs) if (v !== undefined && v !== null) d.append(h('div', { class: 'k' }, k), h('div', { class: 'v' }, v));
  return d;
}

function renderInspector() {
  insp.textContent = '';
  const e = S.sel && E(S.sel);
  if (!e) {
    insp.append(h('p', { class: 'muted pad' }, S.model ? 'Select an element on the canvas or in the hierarchy. Drag to move; Ctrl+D clones; F fits.' : 'Open a scene to begin.'));
    if (S.model) insp.append(sceneInfo());
    return;
  }
  const w = S.world.get(e.k);
  const nameInput = h('input', { class: 'input', value: e.name, spellcheck: 'false', disabled: e.parent === null, title: 'Rename (recomputes elementIds of this subtree)' });
  const rename = () => {
    const v = nameInput.value.trim();
    if (v && v !== e.name) pushOp({ op: 'rename', key: e.k, name: v }, `Rename ${e.name} → ${v}`);
  };
  nameInput.addEventListener('keydown', (ev) => { if (ev.key === 'Enter') nameInput.blur(); if (ev.key === 'Escape') { nameInput.value = e.name; nameInput.blur(); } });
  nameInput.addEventListener('blur', rename);

  insp.append(h('div', { class: 'sec' },
    h('div', { class: 'title-row' }, nameInput, h('span', { class: 'type-chip', style: `background:${typeColor(e.type)}` }, e.type)),
    h('div', { class: 'hint' }, e.parent === null ? 'Scene root (cannot be moved, cloned or deleted).' : '')));

  const guid = param(e, 'guid');
  insp.append(sec('Identity', kv(
    ['path', e.path || '(scene root)'],
    ['elementId', h('span', {}, `${e.id} `, h('span', { class: 'muted' }, `0x${(Number(e.id) >>> 0).toString(16).padStart(8, '0')} `),
      h('span', { class: e.idOk ? 'ok' : 'bad', title: 'elementId == djb2(dotted path)' }, e.idOk ? '✓ djb2(path)' : '✗ djb2 mismatch'))],
    ['guid', guid],
    ['key', h('span', { class: 'muted' }, e.k)])));

  const ang = quatAngleZ(e.rot);
  const tsec = sec('Transform (local)');
  if (e.parent !== null) {
    tsec.append(vecRow('Position', e.pos, 3, (v) => pushOp({ op: 'set', key: e.k, field: 'position', value: v }, `Position ${e.name} → ${fmtVec(v, 3)}`)));
    tsec.append(vecRow('Scale', e.scale, 3, (v) => pushOp({ op: 'set', key: e.k, field: 'scale', value: v }, `Scale ${e.name} → ${fmtVec(v, 3)}`)));
    tsec.append(vecRow('Rotation', e.rot, 4, (v) => pushOp({ op: 'set', key: e.k, field: 'rotation', value: v }, `Rotation ${e.name} → ${fmtVec(v, 3)}`)));
    const angRow = h('div', { class: 'vec' }, h('label', { title: 'Rotation about the parent Z axis (screen plane)' }, 'Z angle °'),
      numInput(ang === null ? 0 : ang, (v) => {
        const cur = ang === null ? 0 : ang;
        const d = ((v - cur) * Math.PI) / 180;
        const q = quatMul([0, 0, Math.sin(d / 2), Math.cos(d / 2)], quatUnit(e.rot)).map((x) => parseFloat(x.toFixed(6)));
        pushOp({ op: 'set', key: e.k, field: 'rotation', value: q }, `Rotate ${e.name} → ${fmt(v, 2)}°`);
      }, { stepScale: 100, title: 'Up/Down: 1°, Shift: 10°' }));
    tsec.append(angRow);
  } else {
    tsec.append(kv(['position', fmtVec(e.pos)], ['scale', fmtVec(e.scale)], ['rotation', fmtVec(e.rot)]));
  }
  if (w) {
    tsec.append(kv(
      ['world pos', fmtVec([w.m[3], w.m[7], w.m[11]], 4)],
      ['world size', w.aabb ? `${fmt(w.aabb[2] - w.aabb[0], 4)} x ${fmt(w.aabb[3] - w.aabb[1], 4)}` : w.gaabb ? `group ${fmt(w.gaabb[2] - w.gaabb[0], 3)} x ${fmt(w.gaabb[3] - w.gaabb[1], 3)}` : '—'],
    ));
  }
  insp.append(tsec);

  if (e.box) {
    const src = { extents: 'minExtents / maxExtents params', geom: 'geom extentsmin / extentsmax', text: 'text box estimate (boxWidth x previewHeight)', xref: 'bounds of the referenced scene', size: 'width x length params' }[e.box.src];
    insp.append(sec('Box', kv(['source', src], ['local min', fmtVec(e.box.min)], ['local max', fmtVec(e.box.max)])));
  }

  if (e.fileRef) {
    const x = (S.model.xrefs || {})[e.fileRef] || {};
    insp.append(sec('XRef', kv(
      ['fileRef', shortPath(e.fileRef)],
      ['componentId', e.xref],
      ['resolved', x.found ? h('span', { class: 'ok' }, `✓ ${x.erp}, ${x.count} box(es)${x.truncated ? ' (truncated)' : ''}`) : h('span', { class: 'warn' }, x.error || 'unresolved')],
    ), h('div', { class: 'actions', style: 'margin-top:6px' },
      h('button', { class: 'btn small', onclick: () => openScene(shortPath(e.fileRef)) }, 'Open referenced scene'))));
  }

  insp.append(paramsSection(e));

  if (e.geoms.length) {
    const ul = h('ul', { class: 'list' });
    e.geoms.forEach((g, i) => {
      const res = h('div', { class: 'small muted' });
      const btn = h('button', { class: 'btn link small', onclick: () => listResources(g.resourceid, res) }, 'list ERP resources');
      ul.append(h('li', {},
        h('div', {}, h('span', { class: 'lbl' }, `#${i} resource`), shortPath(g.resourceid)),
        h('div', {}, h('span', { class: 'lbl' }, 'material'), shortPath(g.materialid)),
        h('div', {}, h('span', { class: 'lbl' }, 'extents'), `${fmtVec((g.extentsmin || []).slice(0, 3))} → ${fmtVec((g.extentsmax || []).slice(0, 3))}`),
        h('div', {}, h('span', { class: 'lbl' }, 'asset'), String(g.asset)),
        visualInfo(S.vis.get(visKey(g.resourceid, g.materialid))),
        S.origin && S.origin.erp ? btn : null, res));
    });
    insp.append(sec(['Geoms', h('span', { class: 'muted' }, 'drawn art (not rendered here)')], ul));
  }
  if (e.anims.length) {
    insp.append(sec('Anims', h('ul', { class: 'list' }, e.anims.map((a) => h('li', {}, shortPath(a.asset))))));
  }
  if (e.ch.length) {
    insp.append(sec(`Children (${e.ch.length})`, h('div', { class: 'kids' },
      e.ch.map((c) => h('span', { class: 'chip', onclick: () => select(c) }, h('span', { class: 'sw', style: `background:${typeColor(E(c).type)}` }), E(c).name)))));
  }

  if (e.parent !== null) {
    const cloneName = h('input', { class: 'input', id: 'clone-name', value: nextName(e.name), spellcheck: 'false' });
    insp.append(sec('Actions', h('div', { class: 'actions' },
      cloneName,
      h('button', { class: 'btn small', onclick: () => doClone(cloneName.value.trim()) }, 'Clone'),
      h('button', { class: 'btn small', onclick: doDelete }, 'Delete'),
      h('button', { class: 'btn small', onclick: () => select(e.parent) }, 'Parent'),
      h('button', { class: 'btn small', onclick: fitSelection }, 'Fit')),
    h('div', { class: 'hint' }, 'Clone copies the subtree as a new sibling, recomputes elementIds (djb2 of the dotted path), gives fresh guids and rebuilds nodeidlookup.')));
  }
}

function visualInfo(v) {
  if (!v || v.loading) return h('div', { class: 'small muted' }, S.opts.textures ? 'visual: loading…' : 'visual: textures off');
  if (v.error) return h('div', { class: 'small warn' }, `visual: ${v.error}`);
  const col = matColour(v.params || {});
  const tex = (v.textures || [])[0];
  const t = tex && S.tex.get(tex);
  return h('div', { class: 'small' },
    h('div', {}, h('span', { class: 'lbl' }, 'shader'), v.shader || '?', ` · slot ${v.slot}`),
    h('div', {}, h('span', { class: 'lbl' }, 'material'), shortPath(v.material)),
    tex ? h('div', {}, h('span', { class: 'lbl' }, 'texture'), shortPath(tex), t && t.canvas ? ` (${t.w}x${t.h} BC7)` : t && t.error ? ` (${t.error})` : '') : null,
    h('div', {}, h('span', { class: 'lbl' }, 'colour'),
      h('span', { style: `display:inline-block;width:10px;height:10px;border-radius:2px;vertical-align:middle;margin-right:4px;background:${cssColour(col, 1)}` }),
      `${fmtVec(col.slice(0, 3), 3)} a=${fmt(col[3], 3)}`),
    v.matError ? h('div', { class: 'warn' }, v.matError) : null);
}

function paramsSection(e) {
  const filter = h('input', { class: 'input tiny', placeholder: 'filter', spellcheck: 'false', style: 'width:110px' });
  const table = h('table', { class: 'params' });
  const fill = () => {
    table.textContent = '';
    const q = filter.value.trim().toLowerCase();
    for (const p of e.params) {
      if (p.n === 'guid' || (q && !p.n.toLowerCase().includes(q))) continue;
      let cell;
      if (p.ed && Array.isArray(p.v)) {
        const cur = [...p.v];
        cell = h('div', { class: 'multi' }, p.v.map((x, i) => numInput(x, (v) => {
          cur[i] = v;
          pushOp({ op: 'param', key: e.k, name: p.n, value: [...cur] }, `${e.name}.${p.n} → ${fmtVec(cur, 3)}`);
        })));
      } else if (p.ed && p.t === 'bool') {
        const cb = h('input', { type: 'checkbox', checked: !!p.v });
        cb.addEventListener('change', () => pushOp({ op: 'param', key: e.k, name: p.n, value: cb.checked }, `${e.name}.${p.n} → ${cb.checked}`));
        cell = cb;
      } else if (p.ed) {
        cell = numInput(p.v, (v) => pushOp({ op: 'param', key: e.k, name: p.n, value: v }, `${e.name}.${p.n} → ${fmt(v)}`));
      } else {
        cell = h('span', { title: String(p.v) }, typeof p.v === 'string' ? shortPath(p.v) : JSON.stringify(p.v));
      }
      table.append(h('tr', {}, h('td', { class: 'pn' }, p.n), h('td', { class: 'pt' }, p.t), h('td', { class: 'pv' }, cell)));
    }
  };
  filter.addEventListener('input', fill);
  fill();
  return sec([`Params (${e.params.length})`, filter], table);
}

async function listResources(resourceid, target) {
  target.textContent = 'loading…';
  try {
    const r = await api(`/api/resources?erp=${encodeURIComponent(S.origin.erp)}&prefix=${encodeURIComponent(resourceid)}`);
    target.textContent = '';
    if (!r.resources.length) target.textContent = 'no matching resources in this ERP';
    for (const x of r.resources) {
      target.append(h('div', {}, `${x.kind}  ${x.name.slice(resourceid.length) || '(exact)'}  [${x.fragments.map((f) => f.size).join(', ')} B]`));
    }
  } catch (err) {
    target.textContent = err.message;
  }
}

function sceneInfo() {
  const m = S.model;
  const d = h('div');
  for (const sc of m.scenes) {
    d.append(sec(`Scene ${sc.index}`, kv(
      ['componentId', `${sc.componentId} (0x${sc.componentIdHex})`],
      ['matches path', sc.componentIdMatchesPath === null ? 'n/a' : h('span', { class: sc.componentIdMatchesPath ? 'ok' : 'bad' }, sc.componentIdMatchesPath ? '✓ md5-64(eaid path)' : '✗')],
      ['nodeidlookup', h('span', { class: sc.lookupOk ? 'ok' : 'bad' }, `${sc.lookupCount} ${sc.lookupOk ? '✓ sorted djb2(names)' : '✗ differs from sorted djb2(names)'}`)],
      ['elements', String(m.stats.elements)],
      ['xrefs', String(m.stats.xrefs)],
      ['source', S.origin.erp ? `${S.origin.erp}` : S.origin.file || S.origin.label],
    )));
  }
  return d;
}

/* ---------- edits ---------- */

function nextName(name) {
  const names = new Set(Object.values(S.els).map((e) => e.name));
  const m = /^(.*?)(\d+)$/.exec(name);
  if (m) {
    let n = Number(m[2]) + 1;
    while (names.has(m[1] + n)) n++;
    return m[1] + n;
  }
  let i = 2;
  while (names.has(`${name}_${i}`)) i++;
  return `${name}_${i}`;
}

function clonePosition(e) {
  const m = /^(.*?)(\d+)$/.exec(e.name);
  const sibs = E(e.parent).ch.map(E);
  if (m) {
    const prev = sibs.find((s) => s.name === m[1] + (Number(m[2]) - 1));
    if (prev) {
      const d = e.pos.map((v, i) => v - prev.pos[i]);
      if (Math.hypot(...d) > 1e-6) return e.pos.map((v, i) => parseFloat((v + d[i]).toFixed(6)));
    }
  }
  const w = S.world.get(e.k);
  const local = w && worldToLocalDelta(w.pm, 0.3, -0.3);
  return local ? e.pos.map((v, i) => parseFloat((v + local[i]).toFixed(6))) : [...e.pos];
}

function doClone(name) {
  const e = S.sel && E(S.sel);
  if (!e || e.parent === null) return;
  name = name || nextName(e.name);
  pushOp({ op: 'clone', key: e.k, name, position: clonePosition(e) }, `Clone ${e.name} → ${name}`, { selectResult: true });
}

function doDelete() {
  const e = S.sel && E(S.sel);
  if (!e || e.parent === null) return;
  pushOp({ op: 'delete', key: e.k }, `Delete ${e.name}`, { selectResult: true });
}

function pushOp(op, desc, opts = {}) {
  if (!S.doc) return;
  const last = S.ops[S.ops.length - 1];
  const now = Date.now();
  if (opts.coalesce && last && last.c === opts.coalesce && now - last.t < 900) {
    S.ops[S.ops.length - 1] = { op, desc, c: opts.coalesce, t: now };
  } else {
    S.ops.push({ op, desc, c: opts.coalesce || null, t: now });
  }
  S.redo = [];
  return sync({ selectResult: opts.selectResult ? S.ops.length - 1 : null });
}

function undo() {
  if (!S.ops.length) return;
  S.redo.push(S.ops.pop());
  sync({});
}

function redo() {
  if (!S.redo.length) return;
  S.ops.push(S.redo.pop());
  sync({ selectResult: null });
}

async function sync(after) {
  const seq = ++S.applySeq;
  const ops = S.ops.map((o) => o.op);
  saveLocal();
  renderHistory();
  updateButtons();
  try {
    let m;
    try {
      m = await api('/api/apply', { doc: S.doc, ops });
    } catch (err) {
      if (err.status !== 410) throw err;
      await reopenOrigin();
      m = await api('/api/apply', { doc: S.doc, ops });
    }
    if (seq !== S.applySeq) return;
    setModel(m, after);
  } catch (err) {
    if (seq !== S.applySeq) return;
    if (err.opIndex !== undefined) {
      toast(err.message, 'err');
      S.ops.splice(err.opIndex);
      sync({});
    } else {
      toast(err.message, 'err');
    }
  }
}

function setModel(m, after = {}) {
  const prevSel = S.sel;
  S.model = m;
  S.els = m.elements;
  S.clonedPaths = new Set();
  (m.results || []).forEach((r, i) => { if (S.ops[i] && S.ops[i].op.op === 'clone' && r.path) S.clonedPaths.add(r.path); });
  let sel = prevSel && E(prevSel) ? prevSel : null;
  if (after.selectResult != null && m.results && m.results[after.selectResult]) {
    sel = m.results[after.selectResult].key;
    if (m.results[after.selectResult].warning) toast(m.results[after.selectResult].warning);
  }
  S.sel = sel && E(sel) ? sel : null;
  if (S.hov && !E(S.hov)) S.hov = null;
  compose();
  renderTree();
  renderLegend();
  renderInspector();
  renderHistory();
  updateButtons();
  updateStatus();
  loadGhosts();
  loadVisuals();
  queueRender();
}

function renderHistory() {
  const ol = $('#history');
  ol.textContent = '';
  for (const o of S.ops) ol.append(h('li', {}, o.desc));
  for (let i = S.redo.length - 1; i >= 0; i--) ol.append(h('li', { class: 'undone' }, S.redo[i].desc));
  $('#edit-count').textContent = S.ops.length ? `${S.ops.length} applied` : '';
  ol.scrollTop = ol.scrollHeight;
}

function updateButtons() {
  const e = S.sel && E(S.sel);
  const movable = !!(e && e.parent !== null);
  $('#btn-undo').disabled = !S.ops.length;
  $('#btn-redo').disabled = !S.redo.length;
  $('#btn-clone').disabled = !movable;
  $('#btn-delete').disabled = !movable;
  $('#btn-export').disabled = !S.doc;
}

function updateStatus() {
  const sb = $('#statusbar');
  sb.textContent = '';
  if (!S.model) return;
  const m = S.model;
  const sc = m.scenes[0] || {};
  const bits = [
    `${m.stats.elements} elements`,
    `${m.stats.xrefs} xref targets`,
    h('span', { class: m.stats.idBad ? 'bad' : 'ok' }, m.stats.idBad ? `${m.stats.idBad} elementId mismatch` : 'elementIds ✓'),
    h('span', { class: sc.lookupOk ? 'ok' : 'bad' }, sc.lookupOk ? 'nodeidlookup ✓' : 'nodeidlookup ✗'),
    sc.componentIdMatchesPath === null ? null : h('span', { class: sc.componentIdMatchesPath ? 'ok' : 'bad' }, sc.componentIdMatchesPath ? 'componentId ✓' : 'componentId ✗'),
    `${S.ops.length} edit(s)`,
    S.lastExport ? `last export: ${S.lastExport}` : null,
  ].filter(Boolean);
  bits.forEach((b, i) => { if (i) sb.append(h('span', { class: 'sep' }, '|')); sb.append(b); });
}

function hudStatus(world) {
  const hud = $('#hud-status');
  const parts = [];
  if (world) parts.push(`x ${fmt(world[0], 3)}  y ${fmt(world[1], 3)}`);
  parts.push(`zoom ${fmt(S.view.s / 40 * 100, 0)}%`);
  if (S.opts.textures && textureStats()) parts.push(textureStats());
  if (S.hov && E(S.hov)) {
    const e = E(S.hov);
    parts.push(`${e.name} [${e.type}]${e.fileRef ? ' → ' + baseName(e.fileRef) : ''}`);
  }
  hud.textContent = parts.join('   ');
}

/* ---------- xref ghosts ---------- */

let composeQueued = false;
function queueCompose() {
  if (composeQueued) return;
  composeQueued = true;
  requestAnimationFrame(() => { composeQueued = false; compose(); render(); loadVisuals(); });
}

const ghostKey = (ref) => (S.opts.reveal ? 'r|' : '') + ref;

function loadGhosts() {
  for (const e of Object.values(S.els)) {
    const gk = e.fileRef && ghostKey(e.fileRef);
    if (!gk || S.ghosts.has(gk)) continue;
    S.ghosts.set(gk, { loading: true });
    const erp = S.origin && S.origin.erp ? `&erp=${encodeURIComponent(S.origin.erp)}` : '';
    api(`/api/xref?eaid=${encodeURIComponent(e.fileRef)}${erp}${S.opts.reveal ? '&reveal=1' : ''}`)
      .then((g) => { S.ghosts.set(gk, g); queueCompose(); })
      .catch(() => S.ghosts.set(gk, { error: true }));
  }
}

/* ---------- pointer interaction ---------- */

function evWorld(ev) {
  const r = cv.getBoundingClientRect();
  const px = ev.clientX - r.left, py = ev.clientY - r.top;
  return { px, py, x: wx(px), y: wy(py) };
}

cv.addEventListener('contextmenu', (ev) => ev.preventDefault());

cv.addEventListener('pointerdown', (ev) => {
  if (!S.model && ev.button === 0) return;
  const p = evWorld(ev);
  cv.setPointerCapture(ev.pointerId);
  const base = { sx: p.px, sy: p.py, x: p.x, y: p.y, moved: false, view: { ...S.view } };
  if (ev.button !== 0 || S.space) {
    S.drag = { ...base, mode: 'pan' };
    cv.className = 'grabbing';
    return;
  }
  const hits = pickAll(p.x, p.y);
  const hit = hits[0] || null;
  let target = hit;
  if (S.sel && !isRoot(S.sel)) {
    const w = S.world.get(S.sel);
    if ((hit && isSelfOrDesc(hit, S.sel)) || (!hit && w && inBox(p.x, p.y, w.gaabb))) target = S.sel;
  }
  if (target && isRoot(target)) target = null;
  if (!target) {
    S.drag = { ...base, mode: 'pan', hits, alt: ev.altKey };
    return;
  }
  const e = E(target), w = S.world.get(target);
  S.drag = { ...base, mode: 'move', target, hits, alt: ev.altKey, startPos: [...e.pos], pm: w.pm };
});

cv.addEventListener('pointermove', (ev) => {
  const p = evWorld(ev);
  const d = S.drag;
  if (!d) {
    if (S.model) setHover(pickAll(p.x, p.y)[0] || null);
    hudStatus([p.x, p.y]);
    cv.className = S.space ? 'grab' : S.hov ? 'move' : '';
    return;
  }
  if (!d.moved && Math.hypot(p.px - d.sx, p.py - d.sy) > 3) d.moved = true;
  if (!d.moved) return;
  if (d.mode === 'pan') {
    S.view.cx = d.view.cx - (p.px - d.sx) / S.view.s;
    S.view.cy = d.view.cy + (p.py - d.sy) / S.view.s;
    cv.className = 'grabbing';
    queueRender();
    return;
  }
  let dx = (p.px - d.sx) / S.view.s, dy = -(p.py - d.sy) / S.view.s;
  if (ev.shiftKey) { if (Math.abs(dx) > Math.abs(dy)) dy = 0; else dx = 0; }
  const local = worldToLocalDelta(d.pm, dx, dy);
  if (!local) { toast('Parent transform is degenerate (zero scale); cannot drag here.', 'err'); S.drag = null; return; }
  const e = E(d.target);
  e.pos = d.startPos.map((v, i) => parseFloat((v + local[i]).toFixed(5)));
  if (S.sel !== d.target) select(d.target, { scroll: true });
  compose();
  queueRender();
  liveTransform(e);
  hudStatus([p.x, p.y]);
});

function liveTransform(e) {
  const inputs = insp.querySelectorAll('.vec')[0];
  if (!inputs || S.sel !== e.k) return;
  inputs.querySelectorAll('input').forEach((inp, i) => { inp.value = fmt(e.pos[i], 6); inp.dataset.orig = inp.value; });
}

cv.addEventListener('pointerup', (ev) => {
  const d = S.drag;
  S.drag = null;
  cv.className = S.space ? 'grab' : '';
  if (!d) return;
  if (!d.moved) {
    if (ev.button !== 0) return;
    if (d.alt && d.hits && d.hits.length > 1) {
      const i = d.hits.indexOf(S.sel);
      select(d.hits[(i + 1) % d.hits.length]);
    } else {
      select(d.hits && d.hits.length ? d.hits[0] : null);
    }
    return;
  }
  if (d.mode === 'move') {
    const e = E(d.target);
    pushOp({ op: 'set', key: e.k, field: 'position', value: [...e.pos] }, `Move ${e.name} → ${fmtVec(e.pos, 3)}`);
  }
});

cv.addEventListener('pointerleave', () => { if (!S.drag) setHover(null); });

cv.addEventListener('wheel', (ev) => {
  ev.preventDefault();
  const p = evWorld(ev);
  const f = Math.exp(-ev.deltaY * (ev.deltaMode === 1 ? 0.05 : 0.0015));
  const s = Math.max(2, Math.min(40000, S.view.s * f));
  S.view.cx = p.x - (p.px - W / 2) / s;
  S.view.cy = p.y + (p.py - H / 2) / s;
  S.view.s = s;
  hudStatus([p.x, p.y]);
  queueRender();
}, { passive: false });

/* ---------- keyboard ---------- */

function nudge(dx, dy) {
  const e = S.sel && E(S.sel);
  if (!e || e.parent === null) return;
  const w = S.world.get(e.k);
  const local = worldToLocalDelta(w.pm, dx, dy);
  if (!local) return;
  e.pos = e.pos.map((v, i) => parseFloat((v + local[i]).toFixed(5)));
  compose();
  queueRender();
  liveTransform(e);
  pushOp({ op: 'set', key: e.k, field: 'position', value: [...e.pos] }, `Nudge ${e.name} → ${fmtVec(e.pos, 3)}`, { coalesce: `nudge:${e.k}` });
}

window.addEventListener('keydown', (ev) => {
  const tag = (ev.target.tagName || '').toLowerCase();
  const typing = tag === 'input' || tag === 'textarea' || tag === 'select';
  const mod = ev.ctrlKey || ev.metaKey;
  if (mod && ev.key.toLowerCase() === 'k') { ev.preventDefault(); openPalette(); return; }
  if (mod && ev.key.toLowerCase() === 's') { ev.preventDefault(); doExport(); return; }
  if (ev.key === 'Escape') {
    if (!$('#palette').hidden) { closePalette(); return; }
    if (!$('#export-modal').hidden) { $('#export-modal').hidden = true; return; }
    if (!typing) select(null);
    return;
  }
  if (typing || !$('#palette').hidden) return;
  if (mod && ev.key.toLowerCase() === 'z') { ev.preventDefault(); if (ev.shiftKey) redo(); else undo(); return; }
  if (mod && ev.key.toLowerCase() === 'y') { ev.preventDefault(); redo(); return; }
  if (mod && ev.key.toLowerCase() === 'd') { ev.preventDefault(); doClone(); return; }
  if (mod) return;
  if (ev.key === ' ') { S.space = true; cv.className = 'grab'; ev.preventDefault(); return; }
  if (ev.key === 'Delete' || ev.key === 'Backspace') { doDelete(); return; }
  if (ev.key === 'f' || ev.key === 'F') { if (ev.shiftKey) fitSelection(); else fitScene(); return; }
  if (ev.key === 'p' || ev.key === 'P') { const e = S.sel && E(S.sel); if (e && e.parent !== null) select(e.parent); return; }
  const toggles = { x: 'opt-xrefs', t: 'opt-textures', o: 'opt-outlines' };
  if (toggles[ev.key.toLowerCase()]) { const c = $('#' + toggles[ev.key.toLowerCase()]); c.checked = !c.checked; c.dispatchEvent(new Event('change')); return; }
  if (ev.key === '/') { ev.preventDefault(); openPalette(); return; }
  const step = ev.shiftKey ? 0.1 : 0.01;
  const arrows = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] };
  if (arrows[ev.key] && S.sel) { ev.preventDefault(); nudge(...arrows[ev.key]); }
});

window.addEventListener('keyup', (ev) => {
  if (ev.key === ' ') { S.space = false; cv.className = ''; }
});

/* ---------- options ---------- */

for (const [id, key] of [['opt-textures', 'textures'], ['opt-outlines', 'outlines'], ['opt-text', 'text'], ['opt-reveal', 'reveal'], ['opt-xrefs', 'xrefs'], ['opt-frame', 'frame'], ['opt-grid', 'grid'], ['opt-pivots', 'pivots']]) {
  $('#' + id).addEventListener('change', (ev) => {
    S.opts[key] = ev.target.checked;
    savePrefs();
    if (key === 'textures') loadVisuals();
    if (key === 'reveal') { loadGhosts(); queueCompose(); }
    queueRender();
  });
}
$('#opt-labels').addEventListener('change', (ev) => { S.opts.labels = ev.target.value; savePrefs(); queueRender(); });

function savePrefs() {
  try { localStorage.setItem('swofty-ui:prefs', JSON.stringify(S.opts)); } catch (_) { /* storage unavailable */ }
}

function loadPrefs() {
  try {
    const p = JSON.parse(localStorage.getItem('swofty-ui:prefs') || 'null');
    if (p) Object.assign(S.opts, p);
  } catch (_) { /* storage unavailable */ }
  $('#opt-textures').checked = S.opts.textures;
  $('#opt-outlines').checked = S.opts.outlines;
  $('#opt-text').checked = S.opts.text;
  $('#opt-reveal').checked = S.opts.reveal;
  $('#opt-xrefs').checked = S.opts.xrefs;
  $('#opt-frame').checked = S.opts.frame;
  $('#opt-grid').checked = S.opts.grid;
  $('#opt-pivots').checked = S.opts.pivots;
  $('#opt-labels').value = S.opts.labels;
}

/* ---------- persistence of unsaved edits ---------- */

function originKey(o) {
  if (!o) return null;
  if (o.kind === 'erp') return 'erp:' + o.eaid;
  if (o.kind === 'file') return 'file:' + o.file;
  return 'upload:' + S.doc;
}

function saveLocal() {
  const k = originKey(S.origin);
  if (!k) return;
  try {
    if (S.ops.length || S.redo.length) localStorage.setItem('swofty-ui:edits:' + k, JSON.stringify({ doc: S.doc, ops: S.ops, redo: S.redo, t: Date.now() }));
    else localStorage.removeItem('swofty-ui:edits:' + k);
  } catch (_) { /* storage unavailable */ }
}

function offerRestore() {
  const k = originKey(S.origin);
  let saved = null;
  try { saved = JSON.parse(localStorage.getItem('swofty-ui:edits:' + k) || 'null'); } catch (_) { saved = null; }
  const old = $('.banner');
  if (old) old.remove();
  if (!saved || !saved.ops || !saved.ops.length) return;
  if (saved.doc && saved.doc !== S.doc) return;
  const banner = h('div', { class: 'banner' },
    h('span', {}, `${saved.ops.length} edit(s) from an earlier session.`),
    h('button', { class: 'btn small primary', onclick: () => { banner.remove(); S.ops = saved.ops; S.redo = saved.redo || []; sync({}); } }, 'Restore'),
    h('button', { class: 'btn small', onclick: () => { banner.remove(); S.ops = []; S.redo = []; saveLocal(); } }, 'Discard'));
  $('#stage').append(banner);
}

/* ---------- opening scenes ---------- */

function startDoc(m) {
  S.doc = m.doc;
  S.origin = m.origin;
  S.ops = [];
  S.redo = [];
  S.sel = null;
  S.hov = null;
  S.lastExport = null;
  S.els = m.elements;
  defaultCollapse();
  setModel(m);
  $('#empty').hidden = true;
  renderTitle();
  fitScene();
  offerRestore();
  const hash = m.origin.kind === 'erp' ? '#scene=' + encodeURIComponent(shortPath(m.origin.eaid))
    : m.origin.kind === 'file' ? '#file=' + encodeURIComponent(m.origin.file) : '';
  if (hash && location.hash !== hash) history.pushState(null, '', hash);
  document.title = `${baseName(m.origin.label)} · Swofty UI`;
}

async function openScene(short, erp) {
  try {
    const m = await api(`/api/open?eaid=${encodeURIComponent(short)}${erp ? '&erp=' + encodeURIComponent(erp) : ''}`);
    startDoc(m);
  } catch (err) {
    toast(err.message, 'err');
  }
}

async function openFile(path) {
  try {
    startDoc(await api(`/api/open-file?path=${encodeURIComponent(path)}`));
  } catch (err) {
    toast(err.message, 'err');
  }
}

async function uploadFile(file) {
  try {
    const buf = await file.arrayBuffer();
    startDoc(await api(`/api/upload?name=${encodeURIComponent(file.name)}`, buf));
  } catch (err) {
    toast(err.message, 'err');
  }
}

async function reopenOrigin() {
  const o = S.origin;
  let m;
  if (o.kind === 'erp') m = await api(`/api/open?eaid=${encodeURIComponent(o.eaid)}&erp=${encodeURIComponent(o.erp)}`);
  else if (o.kind === 'file') m = await api(`/api/open-file?path=${encodeURIComponent(o.file)}`);
  else throw new Error('The server restarted and this upload is gone; open the file again.');
  S.doc = m.doc;
}

function renderTitle() {
  const t = $('#scene-title');
  t.textContent = '';
  const o = S.origin;
  if (!o) return;
  const label = o.kind === 'erp' ? shortPath(o.eaid) : o.file || o.label;
  const base = baseName(label);
  t.append(h('span', { class: 'path', title: label }, label.slice(0, label.length - base.length), h('b', {}, base)));
  if (o.kind === 'erp' && o.erps && o.erps.length > 1) {
    const s = h('select', { class: 'input tiny', title: 'This scene ships in several ERPs; pick the source' },
      o.erps.map((x) => h('option', { value: x, selected: x === o.erp }, x)));
    s.addEventListener('change', () => openScene(shortPath(o.eaid), s.value));
    t.append(s);
  } else if (o.erp) {
    t.append(h('span', { class: 'muted' }, o.erp));
  }
}

window.addEventListener('popstate', routeFromHash);

function routeFromHash() {
  const m = /^#(scene|file)=(.+)$/.exec(location.hash);
  if (!m) return false;
  const v = decodeURIComponent(m[2]);
  if (m[1] === 'scene') {
    if (!S.origin || shortPath(S.origin.eaid) !== v) openScene(v);
  } else if (!S.origin || S.origin.file !== v) {
    openFile(v);
  }
  return true;
}

/* ---------- palette ---------- */

const pal = { items: [], active: 0 };

function openPalette() {
  $('#palette').hidden = false;
  const inp = $('#palette-input');
  inp.focus();
  inp.select();
  renderPalette();
}

function closePalette() {
  $('#palette').hidden = true;
}

function renderPalette() {
  const list = $('#palette-list');
  list.textContent = '';
  const q = $('#palette-input').value.trim().toLowerCase();
  const cat = S.catalog;
  pal.items = [];
  if (/^([/~]|[a-z]:[\\/]|\\\\)/.test(q)) {
    pal.items.push({ file: $('#palette-input').value.trim() });
  }
  if (cat) {
    if (!q) {
      for (const [p] of VALIDATION_SCENES) pal.items.push({ scene: p, head: 'Examples' });
    }
    const toks = q.split(/\s+/).filter(Boolean);
    const hits = [];
    for (const [p, erps] of cat.scenes) {
      const lp = p.toLowerCase();
      if (toks.every((t) => lp.includes(t))) {
        const b = baseName(lp);
        const score = (toks.length && toks.every((t) => b.includes(t)) ? 0 : 1) * 1e4 + p.length;
        hits.push([score, p, erps]);
      }
    }
    hits.sort((a, b) => a[0] - b[0]);
    $('#palette-count').textContent = `${hits.length} of ${cat.scenes.length} scenes${hits.length > 300 ? ' (showing 300)' : ''}`;
    for (const [, p, erps] of hits.slice(0, 300)) pal.items.push({ scene: p, erps, head: !q ? 'All scenes' : null });
  } else {
    $('#palette-count').textContent = 'No game catalog loaded; open a file.';
  }
  pal.active = Math.min(pal.active, Math.max(0, pal.items.length - 1));
  let lastHead = null;
  pal.items.forEach((it, i) => {
    if (it.head && it.head !== lastHead) { list.append(h('li', { class: 'head' }, it.head)); lastHead = it.head; }
    const li = it.file
      ? h('li', {}, h('span', { class: 'base' }, `Open file ${it.file}`), h('span', { class: 'dir' }, 'local .idf path'))
      : (() => {
        const b = baseName(it.scene), dir = it.scene.slice(0, it.scene.length - b.length);
        const desc = VALIDATION_SCENES.find((v) => v[0] === it.scene && it.head === 'Validation screens');
        return h('li', {},
          h('span', { class: 'base' }, highlight(b, q)),
          h('span', { class: 'dir' }, desc ? desc[1] : dir),
          h('span', { class: 'erps' }, it.erps ? `${it.erps.length} ERP${it.erps.length > 1 ? 's' : ''}` : ''));
      })();
    li.dataset.i = i;
    if (i === pal.active) li.classList.add('active');
    li.addEventListener('click', () => choosePalette(i));
    li.addEventListener('mousemove', () => { if (pal.active !== i) { pal.active = i; syncPalette(); } });
    list.append(li);
  });
}

function highlight(text, q) {
  const toks = q.split(/\s+/).filter(Boolean);
  if (!toks.length) return text;
  const lower = text.toLowerCase();
  const mark = new Array(text.length).fill(false);
  for (const t of toks) {
    let i = lower.indexOf(t);
    while (i >= 0) { for (let j = i; j < i + t.length; j++) mark[j] = true; i = lower.indexOf(t, i + 1); }
  }
  const out = [];
  let buf = '', on = false;
  for (let i = 0; i < text.length; i++) {
    if (mark[i] !== on) { if (buf) out.push(on ? h('mark', {}, buf) : buf); buf = ''; on = mark[i]; }
    buf += text[i];
  }
  if (buf) out.push(on ? h('mark', {}, buf) : buf);
  return out;
}

function syncPalette() {
  for (const li of $('#palette-list').querySelectorAll('li[data-i]')) li.classList.toggle('active', Number(li.dataset.i) === pal.active);
  const a = $('#palette-list').querySelector('li.active');
  if (a) a.scrollIntoView({ block: 'nearest' });
}

function choosePalette(i) {
  const it = pal.items[i];
  if (!it) return;
  closePalette();
  if (it.file) openFile(it.file);
  else openScene(it.scene);
}

$('#palette-input').addEventListener('input', () => { pal.active = 0; renderPalette(); });
$('#palette-input').addEventListener('keydown', (ev) => {
  if (ev.key === 'ArrowDown') { pal.active = Math.min(pal.items.length - 1, pal.active + 1); syncPalette(); ev.preventDefault(); }
  if (ev.key === 'ArrowUp') { pal.active = Math.max(0, pal.active - 1); syncPalette(); ev.preventDefault(); }
  if (ev.key === 'Enter') { choosePalette(pal.active); ev.preventDefault(); }
});
$('#palette').addEventListener('mousedown', (ev) => { if (ev.target.id === 'palette') closePalette(); });
$('#file-input').addEventListener('change', (ev) => {
  const f = ev.target.files[0];
  if (f) { closePalette(); uploadFile(f); }
  ev.target.value = '';
});

/* ---------- export ---------- */

async function doExport() {
  if (!S.doc) return;
  const btn = $('#btn-export');
  btn.disabled = true;
  try {
    const r = await api('/api/export', { doc: S.doc, ops: S.ops.map((o) => o.op) });
    showExport(r);
    S.lastExport = `${baseName(r.file)} ${r.checks.ok ? '✓' : '✗'}`;
    updateStatus();
  } catch (err) {
    toast(err.message, 'err');
  } finally {
    btn.disabled = false;
  }
}

function showExport(r) {
  const c = r.checks;
  const row = (ok, text, info) => h('li', {}, h('b', { class: info ? 'muted' : ok ? 'ok' : 'bad' }, info ? '•' : ok ? '✓' : '✗'), h('span', {}, text));
  const body = $('#export-body');
  body.textContent = '';
  body.append(
    h('ul', { class: 'checks' },
      row(c.reparse, 'Re-parses with idf.py'),
      row(c.roundtrip, 'Round-trips byte-identically (parse, write, same bytes)'),
      row(c.elementIdsOk, `Every elementId == djb2(dotted path) (${c.elements} elements)${c.elementIdsBad && c.elementIdsBad.length ? ': ' + c.elementIdsBad.join(', ') : ''}`),
      row(c.nodeidlookupOk, 'nodeidlookup == sorted djb2(element names)'),
      c.componentIdsKept !== undefined ? row(c.componentIdsKept, 'Scene componentId unchanged') : null,
      row(true, c.identicalToSource ? 'Identical to the source bytes (no effective edits)' : `Differs from the source (${S.ops.length} edit(s) applied)`, true)),
    h('div', { class: 'small muted' }, `${r.size} bytes`),
    pathBox('File', r.file),
    pathBox('Edit script', r.sidecar),
    r.operations
      ? h('div', {}, pathBox('ERP operations', r.operationsFile),
        snippetBox(`Your addon ships the .idf as ${r.operations.ops[0].source} with these ERP operations, one per archive that holds the scene:`, JSON.stringify(r.operations, null, 2)))
      : h('div', { class: 'small muted' }, 'No game archive holds this scene, so no ERP operations were written.'),
    h('div', { class: 'actions', style: 'margin-top:10px' },
      h('a', { class: 'btn primary', href: r.download, download: '' }, 'Download .idf'),
      h('button', { class: 'btn', onclick: revealExports }, 'Open export folder')));
  $('#export-title').textContent = c.ok ? 'Exported and verified' : 'Exported with problems';
  $('#export-modal').hidden = false;
}

function snippetBox(label, text) {
  return h('div', {}, h('div', { class: 'small muted' }, label),
    h('div', { class: 'snippet' }, h('pre', {}, text),
      h('button', { class: 'btn small', onclick: () => { navigator.clipboard && navigator.clipboard.writeText(text); toast('Copied'); } }, 'Copy')));
}

async function revealExports() {
  try {
    await api('/api/reveal', {});
  } catch (err) {
    toast(err.message, 'err');
  }
}

function pathBox(label, path) {
  const code = h('code', {}, path);
  return h('div', {}, h('div', { class: 'small muted' }, label),
    h('div', { class: 'path-box' }, code,
      h('button', { class: 'btn small', onclick: () => { navigator.clipboard && navigator.clipboard.writeText(path); toast('Copied'); } }, 'Copy')));
}

$('#export-close').addEventListener('click', () => { $('#export-modal').hidden = true; });
$('#export-modal').addEventListener('mousedown', (ev) => { if (ev.target.id === 'export-modal') $('#export-modal').hidden = true; });

/* ---------- buttons ---------- */

$('#btn-open').addEventListener('click', openPalette);
$('#btn-open-2').addEventListener('click', openPalette);
$('#btn-undo').addEventListener('click', undo);
$('#btn-redo').addEventListener('click', redo);
$('#btn-clone').addEventListener('click', () => doClone());
$('#btn-delete').addEventListener('click', doDelete);
$('#btn-fit').addEventListener('click', (ev) => (ev.shiftKey ? fitSelection() : fitScene()));
$('#btn-export').addEventListener('click', doExport);

function renderQuickPicks() {
  const q = $('#quick-picks');
  q.textContent = '';
  for (const [p, desc] of VALIDATION_SCENES) {
    q.append(h('button', { class: 'btn', onclick: () => openScene(p) }, baseName(p), h('span', {}, desc)));
  }
}

/* ---------- boot ---------- */

window.VIEWER = { S, VISUALS, openScene, openFile, compose, render, pickAll, select, pushOp, undo, redo, doClone, doExport, fitScene, fitSelection };

new ResizeObserver(resize).observe($('#stage'));
loadPrefs();
renderQuickPicks();
updateButtons();
resize();
api('/api/catalog').then((c) => {
  S.catalog = c;
  if (!$('#palette').hidden) renderPalette();
}).catch((err) => toast('Catalog failed: ' + err.message, 'err'));
routeFromHash();
