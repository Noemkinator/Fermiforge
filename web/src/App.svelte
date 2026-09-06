<script lang="ts">
import { onMount } from "svelte";
import init, {
  decode_scene_fragment,
  derive_bonds,
  encode_scene_fragment,
  solve_simple_huckel,
} from "../wasm-pkg/fermiforge_core.js";
import { Renderer, type AtomView, type Lobe } from "./renderer";

interface Atom { symbol: string; x: number; y: number; z: number }
interface Scene {
  schema: number;
  mode: string;
  atoms: Atom[];
  bonds: [number, number][];
  charge: number;
  overrides: { key: string; value: string | number }[];
  lepton: string | null;
  nucleus_z: number | null;
}
interface ModelParam { value: number; method?: string; source: string; edition: string }

const DEFAULT_BENZENE = JSON.stringify({
  schema: 2,
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
let renderer: Renderer;
let scene: Scene = JSON.parse(DEFAULT_BENZENE);
let models: Record<string, { value: number; method?: string; source: string; edition: string }> = {};
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
  };
}

function fromWire(w: Scene): Scene {
  return {
    ...w,
    atoms: w.atoms.map((a) => ({ symbol: a.symbol, x: a.x / 100, y: a.y / 100, z: a.z / 100 })),
  };
}

function recompute() {
  if (!ready) return;
  try {
    const cutoff = param("models/huckel/bond_cutoff_angstrom", 1.6);
    const bonds: [number, number][] = scene.bonds.length > 0
      ? scene.bonds
      : JSON.parse(derive_bonds(JSON.stringify({ atoms: toWire(scene).atoms, cutoff })));
    const alpha = param("models/huckel/alpha_eV", 0.0);
    const beta = param("models/huckel/beta_eV", -2.7);
    const res = JSON.parse(solve_simple_huckel(JSON.stringify({
      nAtoms: scene.atoms.length,
      bonds,
      alpha,
      beta,
      electrons: scene.atoms.length, // approximation: one pi electron per atom
    })));
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
  return scene.atoms.map((a, i) => ({
    x: a.x,
    y: a.y,
    radius: 0.25 + 0.85 * Math.abs(coefficients[mo][i]),
    sign: coefficients[mo][i],
  }));
}

function render() {
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth * dpr, h = canvas.clientHeight * dpr;
  if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
  const atoms: AtomView[] = scene.atoms;
  const bonds: [number, number][] = scene.bonds.length > 0
    ? scene.bonds
    : (() => { try {
        const cutoff = param("models/huckel/bond_cutoff_angstrom", 1.6);
        return JSON.parse(derive_bonds(JSON.stringify({ atoms: toWire(scene).atoms, cutoff })));
      } catch { return []; } })();
  renderer.draw(atoms, bonds, lobes());
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

function pick(ev: PointerEvent): number {
  const rect = canvas.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  const clipX = ((ev.clientX - rect.left) / rect.width) * 2 - 1;
  const clipY = 1 - ((ev.clientY - rect.top) / rect.height) * 2;
  let best = -1, bestD = 14 * dpr / Math.min(rect.width, rect.height) * 2;
  scene.atoms.forEach((a, i) => {
    const [cx, cy] = renderer.toClip(a.x, a.y);
    const d = Math.hypot(cx - clipX, cy - clipY);
    if (d < bestD) { bestD = d; best = i; }
  });
  return best;
}

function onDown(ev: PointerEvent) {
  dragging = pick(ev);
  if (dragging >= 0) canvas.setPointerCapture(ev.pointerId);
}

function onMove(ev: PointerEvent) {
  if (dragging < 0) return;
  const rect = canvas.getBoundingClientRect();
  const clipX = ((ev.clientX - rect.left) / rect.width) * 2 - 1;
  const clipY = 1 - ((ev.clientY - rect.top) / rect.height) * 2;
  const [x, y] = renderer.fromClip(clipX, clipY);
  scene.atoms[dragging] = { ...scene.atoms[dragging], x, y };
  scene = scene;
  recompute();
  scheduleUrl();
}

function onUp() { dragging = -1; }

function addAtom() {
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
  scene.atoms.push({ symbol: "C", x: +x.toFixed(2), y: +y.toFixed(2), z: 0 });
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
    renderer,
  };
  await init();
  try {
    models = (await (await fetch("./models.json")).json()).values;
  } catch { /* offline fallback defaults */ }
  if (location.hash.startsWith("#/s=")) {
    try {
      scene = fromWire(JSON.parse(decode_scene_fragment(location.hash)));
      if (scene.mode !== "huckel") linkError = "atom scenes arrive in M3; showing Hückel layer";
    } catch (e) {
      linkError = `Bad shared link: ${e}. Loaded benzene instead.`;
    }
  }
  ready = true;
  recompute();
});
</script>

<main>
  <header>
    <h1>Fermiforge</h1>
    <span class="tag">simple Hückel · π system · one electron per atom (labeled approximation)</span>
    <div class="spacer"></div>
    <button on:click={addAtom}>+ C atom</button>
    <button on:click={copyLink}>{copied ? "copied!" : "copy link"}</button>
  </header>
  {#if linkError || computeError}<div class="error">{linkError || computeError}</div>{/if}
  <section>
    <canvas
      bind:this={canvas}
      on:pointerdown={onDown}
      on:pointermove={onMove}
      on:pointerup={onUp}
    ></canvas>
    <aside>
      <h2>π orbital energies (eV)</h2>
      {#each energies as e, i}
        <button
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
      {#if totalEnergy !== null}<p class="stat">total π energy: <strong>{totalEnergy.toFixed(3)} eV</strong></p>{/if}
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
        method simple-huckel/linear-in-alpha-beta). Drag atoms; link updates live.
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
  section { flex: 1; display: flex; min-height: 0; }
  canvas { flex: 1; min-width: 0; display: block; touch-action: none; cursor: crosshair; }
  aside { width: 260px; padding: 12px; border-left: 1px solid #262b38; overflow-y: auto; }
  h2 { font-size: 13px; text-transform: uppercase; letter-spacing: 0.06em; color: #8b93a7; }
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
