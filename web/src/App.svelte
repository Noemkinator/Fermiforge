<script lang="ts">
import { onMount } from "svelte";
import init, {
  decode_scene_fragment,
  derive_bonded,
  encode_scene_fragment,
  solve_simple_huckel,
} from "../wasm-pkg/fermiforge_core.js";
import { Renderer, type AtomView, type Lobe } from "./renderer";

interface Atom { symbol: string; x: number; y: number; z: number }
interface Bond { a: number; b: number; order: number }
type WireBond = [number, number] | [number, number, string | number];
interface Scene {
  schema: number;
  mode: string;
  atoms: Atom[];
  bonds: Bond[];
  charge: number;
  overrides: { key: string; value: string | number }[];
  lepton: string | null;
  nucleus_z: number | null;
}
interface ModelParam { value: number; method?: string; source: string; edition: string }

const ORDER_CYCLE = [1, 1.5, 2, 3];
const SUBSCRIPT = "₀₁₂₃₄";

const DEFAULT_BENZENE = JSON.stringify({
  schema: 3,
  mode: "huckel",
  atoms: Array.from({ length: 6 }, (_, k) => {
    const a = (Math.PI / 3) * k;
    return { symbol: "C", x: +(1.39 * Math.cos(a)).toFixed(2), y: +(1.39 * Math.sin(a)).toFixed(2), z: 0 };
  }),
  bonds: [],
  charge: 0,
  overrides: [],
});

let canvas: HTMLCanvasElement;
let labelCanvas: HTMLCanvasElement | undefined;
let renderer: Renderer;
let scene: Scene = JSON.parse(DEFAULT_BENZENE);
let models: Record<string, { value: number; method?: string; source: string; edition: string }> = {};
let bondRefs: unknown[] = [];
let valences: Record<string, number> = {};
let bonds3: Bond[] = [];
let piIndex: Map<number, number> = new Map();
let energies: { value: number; source: string; method?: string }[] = [];
let coefficients: number[][] = [];
let homo: number | null = null;
let lumo: number | null = null;
let gap: number | null = null;
let totalEnergy: number | null = null;
let selectedMo: number | null = null;
let linkError = "";
let computeError = "";
let copied = false;
let ready = false;
let urlTimer: ReturnType<typeof setTimeout> | undefined;

function valueOf(id: string, fallback: number) {
  const m = models[id];
  return m ? m.value : fallback;
}

const PARAMS = [
  { id: "models/huckel/alpha_eV", label: "α (eV)", fallback: 0.0 },
  { id: "models/huckel/beta_eV", label: "β (eV)", fallback: -2.7 },
];

// [symbol, grid column, grid row] — standard 18-column layout, f-block on row 9
const ELEMENTS: [string, number, number][] = [
  ["H", 1, 1], ["He", 18, 1],
  ["Li", 1, 2], ["Be", 2, 2], ["B", 13, 2], ["C", 14, 2], ["N", 15, 2], ["O", 16, 2], ["F", 17, 2], ["Ne", 18, 2],
  ["Na", 1, 3], ["Mg", 2, 3], ["Al", 13, 3], ["Si", 14, 3], ["P", 15, 3], ["S", 16, 3], ["Cl", 17, 3], ["Ar", 18, 3],
  ["K", 1, 4], ["Ca", 2, 4], ["Sc", 3, 4], ["Ti", 4, 4], ["V", 5, 4], ["Cr", 6, 4], ["Mn", 7, 4], ["Fe", 8, 4], ["Co", 9, 4], ["Ni", 10, 4], ["Cu", 11, 4], ["Zn", 12, 4], ["Ga", 13, 4], ["Ge", 14, 4], ["As", 15, 4], ["Se", 16, 4], ["Br", 17, 4], ["Kr", 18, 4],
  ["Rb", 1, 5], ["Sr", 2, 5], ["Y", 3, 5], ["Zr", 4, 5], ["Nb", 5, 5], ["Mo", 6, 5], ["Tc", 7, 5], ["Ru", 8, 5], ["Rh", 9, 5], ["Pd", 10, 5], ["Ag", 11, 5], ["Cd", 12, 5], ["In", 13, 5], ["Sn", 14, 5], ["Sb", 15, 5], ["Te", 16, 5], ["I", 17, 5], ["Xe", 18, 5],
  ["Cs", 1, 6], ["Ba", 2, 6], ["La", 3, 6], ["Hf", 4, 6], ["Ta", 5, 6], ["W", 6, 6], ["Re", 7, 6], ["Os", 8, 6], ["Ir", 9, 6], ["Pt", 10, 6], ["Au", 11, 6], ["Hg", 12, 6], ["Tl", 13, 6], ["Pb", 14, 6], ["Bi", 15, 6], ["Po", 16, 6], ["At", 17, 6], ["Rn", 18, 6],
  ["U", 6, 9],
];

function param(id: string, fallback: number) {
  const o = scene.overrides.find((x) => x.key === id);
  if (o) return { value: parseFloat(String(o.value)), source: "user", edition: "user override" };
  const m = models[id];
  return {
    value: m ? m.value : fallback,
    source: m?.source ?? "literature",
    edition: m?.edition ?? "Pauling-1960",
  };
}

function setOverride(id: string, fallback: number, raw: string) {
  const v = parseFloat(raw);
  if (!Number.isFinite(v)) return;
  const base = valueOf(id, fallback);
  scene.overrides = scene.overrides.filter((o) => o.key !== id);
  if (v !== base) scene.overrides.push({ key: id, value: String(v) });
  scene = scene;
  recompute();
  scheduleUrl();
}

// core Atom serde uses integer multiples of 0.01 A; UI works in angstroms
function toWire(s: Scene) {
  return {
    ...s,
    atoms: s.atoms.map((a) => ({
      symbol: a.symbol,
      x: Math.round(a.x * 100),
      y: Math.round(a.y * 100),
      z: Math.round(a.z * 100),
    })),
    overrides: s.overrides.map((o) => ({ key: o.key, value: String(o.value) })),
    bonds: s.bonds.map((b): WireBond =>
      b.order === 1 ? [b.a, b.b] : [b.a, b.b, String(b.order)]),
  };
}

interface WireScene extends Omit<Scene, "bonds"> { bonds: WireBond[] }

function fromWire(w: WireScene): Scene {
  return {
    ...w,
    atoms: w.atoms.map((a) => ({ symbol: a.symbol, x: a.x / 100, y: a.y / 100, z: a.z / 100 })),
    bonds: (w.bonds ?? []).map((b) => ({
      a: b[0],
      b: b[1],
      order: b.length > 2 ? parseFloat(String(b[2])) : 1,
    })),
  };
}

function deriveBonded(): Bond[] {
  try {
    return (JSON.parse(derive_bonded(JSON.stringify({
      atoms: toWire(scene).atoms,
      refs: bondRefs,
      valences,
      fallback_single: valueOf("models/huckel/bond_cutoff_angstrom", 1.6),
    }))) as WireBond[]).map((wb) => ({
      a: wb[0],
      b: wb[1],
      order: wb.length > 2 ? parseFloat(String(wb[2])) : 1,
    }));
  } catch {
    return [];
  }
}

function currentBonds(): Bond[] {
  return scene.bonds.length > 0 ? scene.bonds : deriveBonded();
}

let showAll = false;
let piExists = false;
let frameworkShown = false;

function clampExplicit(bonds: Bond[]): Bond[] {
  const out = bonds.map((b) => ({ ...b }));
  for (let iter = 0; iter < 128; iter++) {
    const sums = scene.atoms.map(() => 0);
    for (const b of out) {
      if (b.a < sums.length) sums[b.a] += b.order;
      if (b.b < sums.length) sums[b.b] += b.order;
    }
    let worst = -1;
    for (let i = 0; i < sums.length; i++) {
      const v = valences[scene.atoms[i].symbol];
      if (v !== undefined && sums[i] > v) {
        worst = i;
        break;
      }
    }
    if (worst < 0) return out;
    const target = out
      .filter((b) => (b.a === worst || b.b === worst) && b.order > 1)
      .sort((x, y) => y.order - x.order)[0];
    if (!target) return out;
    target.order = target.order > 2 ? 2 : target.order > 1.5 ? 1.5 : 1;
  }
  return out;
}

function implicitH(bonds: Bond[]): number[] {
  const sums = scene.atoms.map(() => 0);
  for (const b of bonds) {
    sums[b.a] += b.order;
    sums[b.b] += b.order;
  }
  return scene.atoms.map((a, i) => {
    const v = valences[a.symbol];
    return v === undefined ? 0 : Math.max(0, Math.round(v - sums[i]));
  });
}

function recompute() {
  if (!ready) return;
  try {
    bonds3 = currentBonds();
    const pi = bonds3.filter((b) => b.order >= 1.5);
    piExists = pi.length > 0;
    frameworkShown = showAll || !piExists;
    let solveAtoms: number[];
    let solveBonds: [number, number][];
    if (frameworkShown) {
      solveAtoms = scene.atoms.map((_, i) => i);
      solveBonds = bonds3.map((b) => [b.a, b.b] as [number, number]);
    } else {
      solveAtoms = [...new Set(pi.flatMap((b) => [b.a, b.b]))].sort((x, y) => x - y);
      const idx = new Map(solveAtoms.map((a, k) => [a, k]));
      solveBonds = pi.map((b) => [idx.get(b.a)!, idx.get(b.b)!] as [number, number]);
    }
    piIndex = new Map(solveAtoms.map((a, k) => [a, k]));
    if (solveAtoms.length === 0) {
      energies = [];
      coefficients = [];
      homo = lumo = gap = totalEnergy = null;
      computeError = "";
      render();
      return;
    }
    const electrons = Math.max(0, solveAtoms.length - scene.charge);
    const alpha = param("models/huckel/alpha_eV", 0.0);
    const beta = param("models/huckel/beta_eV", -2.7);
    const request: Record<string, unknown> = {
      nAtoms: solveAtoms.length,
      bonds: solveBonds,
      alpha,
      beta,
      electrons,
    };
    if (frameworkShown) request.orders = bonds3.map((b) => String(b.order));
    const res = JSON.parse(solve_simple_huckel(JSON.stringify(request)));
    energies = res.energies;
    coefficients = res.coefficients;
    homo = res.homo;
    lumo = res.lumo;
    gap = res.gap;
    totalEnergy = res.totalEnergy;
    computeError = "";
  } catch (e) {
    computeError = String(e);
  }
  render();
}

$: maxAbs = Math.max(1e-9, ...energies.map((x) => Math.abs(x.value)));

function lobes(): Lobe[] {
  const mo = selectedMo;
  if (mo === null || !coefficients[mo]) return [];
  return scene.atoms.map((a, i) => {
    const c = piIndex.has(i) ? coefficients[mo][piIndex.get(i)!] : 0;
    return { x: a.x, y: a.y, radius: 0.25 + 0.85 * Math.abs(c), sign: c };
  });
}

function render() {
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth * dpr, h = canvas.clientHeight * dpr;
  if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
  const atoms: AtomView[] = scene.atoms;
  renderer.draw(atoms, bonds3.map((b) => [b.a, b.b, b.order]), lobes());
  drawLabels(dpr);
}

function drawLabels(dpr: number) {
  if (!labelCanvas) return;
  const cw = canvas.clientWidth, ch = canvas.clientHeight;
  labelCanvas.style.width = cw + "px";
  labelCanvas.style.height = ch + "px";
  const w = cw * dpr, h = ch * dpr;
  if (labelCanvas.width !== w || labelCanvas.height !== h) { labelCanvas.width = w; labelCanvas.height = h; }
  const ctx = labelCanvas.getContext("2d");
  if (!ctx) return;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, cw, ch);
  ctx.font = "12px system-ui, sans-serif";
  ctx.fillStyle = "#e6eaf2";
  const hs = implicitH(bonds3);
  scene.atoms.forEach((a, i) => {
    const [cx, cy] = renderer.toClip(a.x + 0.4, a.y + 0.4);
    const h = hs[i] > 1 ? "H" + [...String(hs[i])].map((d) => SUBSCRIPT[+d]).join("") : hs[i] === 1 ? "H" : "";
    ctx.fillText(a.symbol + h, (cx * 0.5 + 0.5) * cw, (1 - (cy * 0.5 + 0.5)) * ch);
  });
}

function scheduleUrl() {
  clearTimeout(urlTimer);
  urlTimer = setTimeout(() => {
    try {
      location.hash = encode_scene_fragment(JSON.stringify(toWire(scene))).replace(/^#/, "");
    } catch (e) { linkError = `URL: ${e}`; }
  }, 500);
}

let dragging = -1;
let panning = false;
let rotating = false;
let lastPx = 0;
let lastPy = 0;
let lastClip: [number, number] = [0, 0];
let moved = false;
let selectedAtom = -1;
let showPalette = false;

function clipOf(ev: PointerEvent | MouseEvent | WheelEvent): [number, number] {
  const rect = canvas.getBoundingClientRect();
  return [
    ((ev.clientX - rect.left) / rect.width) * 2 - 1,
    1 - ((ev.clientY - rect.top) / rect.height) * 2,
  ];
}

function pick(clipX: number, clipY: number): number {
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  let best = -1, bestD = 14 * dpr / Math.min(rect.width, rect.height) * 2;
  scene.atoms.forEach((a, i) => {
    const [cx, cy] = renderer.toClip(a.x, a.y);
    const d = Math.hypot(cx - clipX, cy - clipY);
    if (d < bestD) { bestD = d; best = i; }
  });
  return best;
}

function onDown(ev: PointerEvent) {
  const [cx, cy] = clipOf(ev);
  moved = false;
  lastClip = [cx, cy];
  lastPx = ev.clientX;
  lastPy = ev.clientY;
  if (ev.button === 2 || ev.shiftKey) {
    rotating = true;
  } else {
    dragging = pick(cx, cy);
    if (dragging >= 0) {
      selectedAtom = dragging;
      renderer.selected = dragging;
    } else {
      panning = true;
    }
  }
  canvas.setPointerCapture(ev.pointerId);
  render();
}

function onMove(ev: PointerEvent) {
  const [cx, cy] = clipOf(ev);
  if (rotating) {
    renderer.rotY += (ev.clientX - lastPx) * 0.006;
    renderer.rotX = Math.max(-1.35, Math.min(1.35, renderer.rotX + (ev.clientY - lastPy) * 0.006));
    moved = true;
    lastPx = ev.clientX;
    lastPy = ev.clientY;
    render();
  } else if (dragging >= 0) {
    const [x, y] = renderer.fromClip(cx, cy);
    if (Math.hypot(cx - lastClip[0], cy - lastClip[1]) > 0.01) moved = true;
    scene.atoms[dragging] = { ...scene.atoms[dragging], x, y };
    scene = scene;
    recompute();
    scheduleUrl();
  } else if (panning) {
    renderer.panX += cx - lastClip[0];
    renderer.panY += cy - lastClip[1];
    moved = true;
    render();
  } else {
    canvas.style.cursor = pick(cx, cy) >= 0 ? "grab" : pickBond(ev) >= 0 ? "pointer" : "crosshair";
  }
  lastClip = [cx, cy];
}

function pickBond(ev: PointerEvent): number {
  const rect = canvas.getBoundingClientRect();
  const sx = (c: number) => (c * 0.5 + 0.5) * rect.width;
  const sy = (c: number) => (1 - (c * 0.5 + 0.5)) * rect.height;
  const mx = ev.clientX - rect.left, my = ev.clientY - rect.top;
  let best = -1, bestD = 10;
  bonds3.forEach((b, k) => {
    const [cax, cay] = renderer.toClip(scene.atoms[b.a].x, scene.atoms[b.a].y);
    const [cbx, cby] = renderer.toClip(scene.atoms[b.b].x, scene.atoms[b.b].y);
    const x1 = sx(cax), y1 = sy(cay), x2 = sx(cbx), y2 = sy(cby);
    const dx = x2 - x1, dy = y2 - y1;
    const t = Math.max(0, Math.min(1, ((mx - x1) * dx + (my - y1) * dy) / (dx * dx + dy * dy || 1)));
    const d = Math.hypot(mx - (x1 + t * dx), my - (y1 + t * dy));
    if (d < bestD) { bestD = d; best = k; }
  });
  return best;
}

function orderAllowed(bonds: Bond[], k: number, order: number): boolean {
  const b = bonds[k];
  for (const ai of [b.a, b.b]) {
    const v = valences[scene.atoms[ai].symbol];
    if (v === undefined) continue;
    const sum = bonds.reduce(
      (s, x, i) => s + (i === k ? 0 : x.a === ai || x.b === ai ? x.order : 0),
      0,
    ) + order;
    if (sum > v + 1e-9) return false;
  }
  return true;
}

function cycleBondOrder(k: number) {
  const bonds = currentBonds().map((b) => ({ ...b }));
  const cur = ORDER_CYCLE.indexOf(bonds[k].order);
  for (let s = 1; s <= ORDER_CYCLE.length; s++) {
    const cand = ORDER_CYCLE[(cur + s) % ORDER_CYCLE.length];
    if (orderAllowed(bonds, k, cand)) {
      bonds[k].order = cand;
      break;
    }
  }
  scene.bonds = bonds;
  scene = scene;
  recompute();
  scheduleUrl();
}

function onUp(ev: PointerEvent) {
  if (dragging >= 0 && !moved) selectedAtom = dragging;
  if (panning && !moved) {
    const k = pickBond(ev);
    if (k >= 0) {
      cycleBondOrder(k);
    } else {
      selectedAtom = -1;
      renderer.selected = -1;
      render();
    }
  }
  dragging = -1;
  panning = false;
  rotating = false;
}

function onWheel(ev: WheelEvent) {
  ev.preventDefault();
  const [cx, cy] = clipOf(ev);
  renderer.zoomAt(cx, cy, Math.exp(-ev.deltaY * 0.0012));
  render();
}

function onDblClick() {
  renderer.resetView();
  render();
}

function removeSelected() {
  if (selectedAtom < 0) return;
  const i = selectedAtom;
  scene.atoms.splice(i, 1);
  if (scene.bonds.length > 0) {
    scene.bonds = scene.bonds
      .filter((b) => b.a !== i && b.b !== i)
      .map((b) => ({
        a: b.a > i ? b.a - 1 : b.a,
        b: b.b > i ? b.b - 1 : b.b,
        order: b.order,
      }));
  }
  selectedAtom = -1;
  renderer.selected = -1;
  scene = scene;
  recompute();
  scheduleUrl();
}

function onKey(ev: KeyboardEvent) {
  const tag = (document.activeElement as HTMLElement | null)?.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA") return;
  if (ev.key === "Delete" || ev.key === "Backspace") {
    ev.preventDefault();
    removeSelected();
  } else if (ev.key === "Escape") {
    selectedAtom = -1;
    renderer.selected = -1;
    render();
  } else if (ev.key === "f" || ev.key === "F") {
    renderer.resetView();
    render();
  }
}

function addAtom(symbol = "C") {
  // spiral outward from the centroid until >= 1.3 A from every atom
  const cx = scene.atoms.reduce((s, a) => s + a.x, 0) / Math.max(scene.atoms.length, 1);
  const cy = scene.atoms.reduce((s, a) => s + a.y, 0) / Math.max(scene.atoms.length, 1);
  const maxR = Math.max(0, ...scene.atoms.map((a) => Math.hypot(a.x - cx, a.y - cy)));
  let x = cx + maxR + 1.4, y = cy;
  for (let t = 0; t < 200; t += 0.35) {
    x = cx + (maxR + 1.4 + t * 0.12) * Math.cos(t);
    y = cy + (maxR + 1.4 + t * 0.12) * Math.sin(t);
    if (scene.atoms.every((a) => Math.hypot(a.x - x, a.y - y) >= 1.3)) break;
  }
  scene.atoms.push({ symbol, x: +x.toFixed(2), y: +y.toFixed(2), z: 0 });
  scene = scene;
  recompute();
  scheduleUrl();
}

function clearSelection() { selectedMo = null; render(); }

async function copyLink() {
  try {
    location.hash = encode_scene_fragment(JSON.stringify(toWire(scene))).replace(/^#/, "");
    await navigator.clipboard.writeText(location.href);
    copied = true;
    setTimeout(() => (copied = false), 1500);
  } catch (e) { linkError = String(e); }
}

onMount(async () => {
  renderer = new Renderer(canvas);
  (window as unknown as Record<string, unknown>).__fermiforge = {
    get scene() { return scene; },
    get bonds() { return bonds3; },
    get implicitH() { return implicitH(bonds3); },
    renderer,
    setAtoms(list: Atom[]) {
      scene.atoms = list;
      scene.bonds = [];
      scene = scene;
      recompute();
    },
  };
  await init();
  try {
    const data = (await (await fetch("./models.json")).json()) as {
      values?: typeof models;
      bond_lengths?: { values?: unknown[] };
      valences?: { values?: Record<string, number> };
    };
    models = data.values ?? {};
    bondRefs = data.bond_lengths?.values ?? [];
    valences = data.valences?.values ?? {};
  } catch { /* offline fallback defaults */ }
  if (location.hash.startsWith("#/s=")) {
    try {
      scene = fromWire(JSON.parse(decode_scene_fragment(location.hash)));
      scene.bonds = clampExplicit(scene.bonds);
      if (scene.mode !== "huckel") linkError = "atom scenes arrive in M3; showing Hückel layer";
    } catch (e) {
      linkError = `Bad shared link: ${e}. Loaded benzene instead.`;
    }
  }
  ready = true;
  recompute();
  window.addEventListener("keydown", onKey);
});
</script>

<main>
  <header>
    <h1>Fermiforge</h1>
    <span class="tag">simple Hückel · π system · bond orders from geometry (labeled approximation)</span>
    <div class="spacer"></div>
    <button on:click={() => (showPalette = !showPalette)}>elements</button>
    <button on:click={() => addAtom("C")}>+ C atom</button>
    <button on:click={copyLink}>{copied ? "copied!" : "copy link"}</button>
  </header>
  {#if linkError || computeError}<div class="error">{linkError || computeError}</div>{/if}
  <section>
    <canvas
      bind:this={canvas}
      on:pointerdown={onDown}
      on:pointermove={onMove}
      on:pointerup={onUp}
      on:wheel={onWheel}
      on:dblclick={onDblClick}
      on:contextmenu={(e) => e.preventDefault()}
    ></canvas>
    <canvas bind:this={labelCanvas} class="labels" aria-hidden="true"></canvas>
    {#if showPalette}
      <div class="palette">
        {#each ELEMENTS as [sym, col, row]}
          <button
            class="el"
            style="grid-column:{col}; grid-row:{row}"
            on:click={() => addAtom(sym)}
          >{sym}</button>
        {/each}
      </div>
    {/if}
    <aside>
      <h2>{frameworkShown ? "σ+π" : "π"} orbital energies (eV)</h2>
      <div class="modes">
        <button class="mode" class:sel={!frameworkShown} disabled={!piExists} on:click={() => { showAll = false; recompute(); }}>π only</button>
        <button class="mode" class:sel={frameworkShown} disabled={!scene.atoms.length} on:click={() => { showAll = true; recompute(); }}>σ+π framework</button>
      </div>
      {#each energies as e, i}
        <button
          class="level"
          class:selected={selectedMo === i}
          class:homo={homo === i}
          class:lumo={lumo === i}
          on:click={() => { selectedMo = selectedMo === i ? null : i; render(); }}
        >
          <span class="bar"><i style="left: {e.value < 0 ? 50 - (Math.abs(e.value) / maxAbs) * 50 : 50}%; width: {(Math.abs(e.value) / maxAbs) * 50}%"></i></span>
          {e.value.toFixed(3)}
          {#if homo === i}<em>HOMO</em>{/if}
          {#if lumo === i}<em>LUMO</em>{/if}
        </button>
      {/each}
      {#if gap !== null}<p class="stat">HOMO–LUMO gap: <strong>{gap.toFixed(3)} eV</strong></p>{/if}
      {#if totalEnergy !== null}<p class="stat">total {frameworkShown ? "framework" : "π"} energy: <strong>{totalEnergy.toFixed(3)} eV</strong></p>{/if}
      {#if !piExists && scene.atoms.length}
        <p class="stat">no π bonds — showing σ+π framework (click a bond to add π character)</p>
      {/if}
      <h2>model parameters</h2>
      {#each PARAMS as p}
        {@const ov = scene.overrides.find((o) => o.key === p.id)}
        {@const eff = param(p.id, p.fallback)}
        <label class="param" class:user={!!ov}>
          <span>{p.label}</span>
          <input
            type="number"
            step="0.1"
            value={eff.value}
            on:change={(e) => setOverride(p.id, p.fallback, e.currentTarget.value)}
          />
          {#if ov}
            <button class="reset" on:click={() => setOverride(p.id, p.fallback, String(valueOf(p.id, p.fallback)))}>reset</button>
          {/if}
          <em class="src">{ov ? "user (base " + valueOf(p.id, p.fallback) + ")" : eff.source}</em>
        </label>
      {/each}
      <p class="note">
        energies are derived values (provenance: α, β from data/models.json,
        method simple-huckel/linear-in-alpha-beta). Bond orders come from distances
        (single/double/triple/aromatic reference lengths, valence-clamped; implicit H in labels).
        Drag atoms; click a bond to cycle its order; wheel zooms; drag background pans;
        right-drag (or Shift-drag) rotates in 3D; double-click (or F) resets view;
        click atom + Delete removes it; link updates live.
      </p>
    </aside>
  </section>
</main>

<style>
  :global(body) { margin: 0; background: #12141b; color: #dfe3ec; font: 14px/1.45 system-ui, sans-serif; }
  main { display: flex; flex-direction: column; height: 100vh; }
  header { display: flex; align-items: baseline; gap: 12px; padding: 10px 16px; border-bottom: 1px solid #262b38; }
  h1 { font-size: 18px; margin: 0; }
  .tag { color: #8b93a7; font-size: 12px; }
  .spacer { flex: 1; }
  button { background: #232a3a; color: #dfe3ec; border: 1px solid #333d52; border-radius: 6px; padding: 4px 10px; cursor: pointer; }
  button:hover { background: #2c3548; }
  .error { background: #4a2030; padding: 6px 16px; }
  section { flex: 1; display: flex; min-height: 0; position: relative; }
  canvas { flex: 1; min-width: 0; display: block; touch-action: none; cursor: crosshair; }
  canvas.labels { position: absolute; left: 0; top: 0; pointer-events: none; }
  .palette {
    position: absolute; left: 12px; top: 12px; z-index: 5;
    background: rgba(16, 19, 28, 0.97); border: 1px solid #333d52; border-radius: 8px;
    padding: 10px; display: grid; grid-template-columns: repeat(18, 22px); grid-auto-rows: 22px;
    gap: 2px; box-shadow: 0 8px 30px rgba(0, 0, 0, 0.55);
  }
  .palette .el {
    width: 22px; height: 22px; padding: 0; margin: 0; font-size: 10px; border-radius: 3px;
    background: #232a3a; color: #dfe3ec; border: none; cursor: pointer;
    display: flex; align-items: center; justify-content: center;
  }
  .palette .el:hover { background: #3a4560; outline: 1px solid #6ea8ff; }
  aside { width: 260px; padding: 12px; border-left: 1px solid #262b38; overflow-y: auto; }
  h2 { font-size: 13px; text-transform: uppercase; letter-spacing: 0.06em; color: #8b93a7; }
  .modes { display: flex; gap: 6px; margin: 6px 0 10px; }
  .mode { flex: 1; padding: 4px 6px; font-size: 12px; justify-content: center; }
  .mode.sel { outline: 1px solid #6ea8ff; }
  .mode:disabled { opacity: 0.4; }
  aside button { display: flex; align-items: center; gap: 8px; width: 100%; margin: 2px 0; text-align: left; font-variant-numeric: tabular-nums; }
  aside button.selected { outline: 1px solid #6ea8ff; }
  aside button.homo em { color: #ffb066; }
  aside button.lumo em { color: #6ea8ff; }
  .bar { position: relative; display: inline-block; width: 54px; height: 2px; background: #333d52; }
  .param { display: flex; align-items: center; gap: 8px; margin: 4px 0; }
  .param > span { white-space: nowrap; min-width: 44px; }
  .param input { width: 70px; background: #1a1f2b; color: #dfe3ec; border: 1px solid #333d52; border-radius: 4px; padding: 3px 6px; }
  .param.user input { border-color: #ffb066; }
  .param.user > span { color: #ffb066; }
  .param .reset { padding: 2px 8px; font-size: 12px; }
  .param .src { font-style: normal; color: #8b93a7; font-size: 11px; }
  .bar i { position: absolute; top: -1px; height: 4px; background: #9aa3b8; }
  .bar::after { content: ""; position: absolute; left: 50%; top: -3px; width: 1px; height: 8px; background: #5a6376; }
  @media (max-width: 720px) {
    .tag { display: none; }
    section { flex-direction: column; }
    canvas { flex: none; height: 52vh; }
    aside { width: auto; border-left: none; border-top: 1px solid #262b38; }
  }
  .stat { font-size: 13px; }
  .note { font-size: 11px; color: #8b93a7; }
  em { font-style: normal; font-size: 11px; }
</style>
