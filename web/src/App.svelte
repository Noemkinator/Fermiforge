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
    const cutoff = { value: valueOf("models/huckel/bond_cutoff_angstrom", 1.6), source: models["models/huckel/bond_cutoff_angstrom"]?.source ?? "literature", edition: models["models/huckel/bond_cutoff_angstrom"]?.edition ?? "Pauling-1960" };
    const bonds: [number, number][] = scene.bonds.length > 0
      ? scene.bonds
      : JSON.parse(derive_bonds(JSON.stringify({ atoms: toWire(scene).atoms, cutoff })));
    const alpha = { value: valueOf("models/huckel/alpha_eV", 0.0), source: models["models/huckel/alpha_eV"]?.source ?? "literature", edition: models["models/huckel/alpha_eV"]?.edition ?? "Pauling-1960" };
    const beta = { value: valueOf("models/huckel/beta_eV", -2.7), source: models["models/huckel/beta_eV"]?.source ?? "literature", edition: models["models/huckel/beta_eV"]?.edition ?? "Pauling-1960" };
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

function lobes(): Lobe[] {
  const mo = selectedMo;
  if (mo === null || !coefficients[mo]) return [];
  return scene.atoms.map((a, i) => ({
    x: a.x,
    y: a.y,
    radius: 0.35 + 2.2 * Math.abs(coefficients[mo][i]),
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
        const cutoff = { value: valueOf("models/huckel/bond_cutoff_angstrom", 1.6), source: "x", edition: "x" };
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
  scene.atoms.push({ symbol: "C", x: 0, y: 0, z: 0 });
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
          <span class="level" style="width: {6 + Math.abs(e.value) * 18}px"></span>
          {e.value.toFixed(3)}
          {#if homo === i}<em>HOMO</em>{/if}
          {#if lumo === i}<em>LUMO</em>{/if}
        </button>
      {/each}
      {#if gap !== null}<p class="stat">HOMO–LUMO gap: <strong>{gap.toFixed(3)} eV</strong></p>{/if}
      {#if totalEnergy !== null}<p class="stat">total π energy: <strong>{totalEnergy.toFixed(3)} eV</strong></p>{/if}
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
  .level { display: inline-block; height: 2px; background: #8b93a7; }
  .stat { font-size: 13px; }
  .note { font-size: 11px; color: #8b93a7; }
  em { font-style: normal; font-size: 11px; }
</style>
