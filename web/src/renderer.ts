// WebGL2 renderer: shaded 3D spheres, bonds, MO lobes, perspective orbit view.

export interface AtomView {
  x: number;
  y: number;
  z?: number;
  symbol: string;
}

export interface Lobe {
  x: number;
  y: number;
  radius: number; // angstrom-scaled
  sign: number; // +1 / -1
}

const CPK: Record<string, [number, number, number]> = {
  H: [0.92, 0.92, 0.92],
  He: [0.85, 1.0, 1.0],
  Li: [0.8, 0.5, 1.0],
  Be: [0.75, 0.85, 0.13],
  B: [1.0, 0.71, 0.71],
  C: [0.25, 0.25, 0.25],
  N: [0.19, 0.31, 0.97],
  O: [0.94, 0.16, 0.16],
  F: [0.56, 0.88, 0.31],
  Ne: [0.7, 0.89, 0.96],
  Na: [0.75, 0.5, 1.0],
  Mg: [0.54, 0.88, 0.54],
  Al: [0.75, 0.65, 0.65],
  Si: [0.94, 0.78, 0.63],
  P: [1.0, 0.5, 0.0],
  S: [1.0, 0.78, 0.19],
  Cl: [0.12, 0.94, 0.12],
  Ar: [0.5, 0.82, 0.89],
  K: [0.56, 0.25, 0.83],
  Ca: [0.24, 1.0, 0.0],
  Sc: [0.9, 0.85, 0.79],
  Y: [0.62, 1.0, 1.0],
  Zr: [0.58, 1.0, 1.0],
  Nb: [0.73, 0.71, 0.65],
  Mo: [0.54, 0.6, 0.78],
  Tc: [0.44, 0.56, 1.0],
  Ru: [0.24, 0.5, 1.0],
  Rh: [0.0, 0.41, 1.0],
  Pd: [0.0, 0.36, 1.0],
  Cd: [1.0, 0.75, 0.5],
  In: [0.74, 0.33, 0.33],
  Sb: [0.61, 0.4, 0.71],
  Te: [0.83, 0.48, 0.0],
  Po: [0.6, 0.3, 0.3],
  At: [0.5, 0.3, 0.2],
  Ti: [0.75, 0.76, 0.78],
  V: [0.65, 0.65, 0.67],
  Cr: [0.54, 0.6, 0.78],
  Mn: [0.61, 0.48, 0.78],
  Fe: [0.89, 0.47, 0.13],
  Co: [0.94, 0.56, 0.63],
  Ni: [0.31, 0.82, 0.31],
  Cu: [0.78, 0.5, 0.2],
  Zn: [0.49, 0.5, 0.69],
  Ga: [0.76, 0.56, 0.56],
  Ge: [0.4, 0.56, 0.56],
  As: [0.74, 0.5, 0.89],
  Se: [1.0, 0.63, 0.0],
  Br: [0.65, 0.16, 0.16],
  Kr: [0.36, 0.72, 0.82],
  Ag: [0.75, 0.75, 0.75],
  Sn: [0.66, 0.66, 0.66],
  I: [0.58, 0.0, 0.58],
  W: [0.13, 0.22, 0.29],
  Pt: [0.82, 0.82, 0.88],
  Au: [1.0, 0.82, 0.14],
  Hg: [0.72, 0.72, 0.82],
  Pb: [0.34, 0.35, 0.38],
  Bi: [0.62, 0.47, 0.6],
  U: [0.0, 0.56, 0.84],
};

const DISC_VS = `#version 300 es
in vec2 corner;
in vec2 center;
in float radius;
in vec4 color;
in float shaded;
uniform vec2 radiusScale;
out vec2 v_uv;
out vec4 v_color;
out float v_shaded;
void main() {
  v_uv = corner;
  v_color = color;
  v_shaded = shaded;
  gl_Position = vec4(center + corner * radius * radiusScale, 0.0, 1.0);
}`;

const DISC_FS = `#version 300 es
precision mediump float;
in vec2 v_uv;
in vec4 v_color;
in float v_shaded;
out vec4 outColor;
void main() {
  float d = length(v_uv);
  if (d > 1.0) discard;
  float edge = smoothstep(1.0, 0.93, d);
  vec3 col = v_color.rgb;
  if (v_shaded > 0.5) {
    vec3 n = vec3(v_uv, sqrt(max(0.0, 1.0 - d * d)));
    vec3 L = normalize(vec3(-0.45, 0.55, 0.7));
    float diff = max(dot(n, L), 0.0);
    float spec = pow(max(dot(reflect(-L, n), vec3(0.0, 0.0, 1.0)), 0.0), 28.0);
    col = col * (0.35 + 0.75 * diff) + vec3(0.4 * spec);
    edge *= smoothstep(1.0, 0.97, d);
  }
  outColor = vec4(col, v_color.a * edge);
}`;

const LINE_VS = `#version 300 es
in vec2 pos;
void main() { gl_Position = vec4(pos, 0.0, 1.0); }`;

const LINE_FS = `#version 300 es
precision mediump float;
uniform vec4 color;
out vec4 outColor;
void main() { outColor = color; }`;

function compile(gl: WebGL2RenderingContext, type: number, src: string): WebGLShader {
  const sh = gl.createShader(type)!;
  gl.shaderSource(sh, src);
  gl.compileShader(sh);
  if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
    throw new Error(gl.getShaderInfoLog(sh) ?? "shader error");
  }
  return sh;
}

function program(gl: WebGL2RenderingContext, vs: string, fs: string): WebGLProgram {
  const p = gl.createProgram()!;
  gl.attachShader(p, compile(gl, gl.VERTEX_SHADER, vs));
  gl.attachShader(p, compile(gl, gl.FRAGMENT_SHADER, fs));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) {
    throw new Error(gl.getProgramInfoLog(p) ?? "link error");
  }
  return p;
}

const PERSP = 6.0; // perspective distance in clip units

export class Renderer {
  private gl: WebGL2RenderingContext;
  private disc: WebGLProgram;
  private line: WebGLProgram;
  private atomVao: WebGLVertexArrayObject;
  private atomBuf: WebGLBuffer;
  private lobeVao: WebGLVertexArrayObject;
  private lobeBuf: WebGLBuffer;
  private lineVao: WebGLVertexArrayObject;
  private lineBuf: WebGLBuffer;
  scale: [number, number] = [1, 1];
  offset: [number, number] = [0, 0];
  zoom = 1;
  panX = 0;
  panY = 0;
  rotX = 0;
  rotY = 0;
  selected = -1;

  resetView() {
    this.zoom = 1;
    this.panX = 0;
    this.panY = 0;
    this.rotX = 0;
    this.rotY = 0;
  }

  zoomAt(clipX: number, clipY: number, factor: number) {
    const zOld = this.zoom;
    const zNew = Math.min(20, Math.max(0.15, zOld * factor));
    this.panX = clipX - ((clipX - this.panX) / zOld) * zNew;
    this.panY = clipY - ((clipY - this.panY) / zOld) * zNew;
    this.zoom = zNew;
  }

  constructor(private canvas: HTMLCanvasElement) {
    const gl = canvas.getContext("webgl2", { antialias: true, alpha: true });
    if (!gl) throw new Error("WebGL2 not available");
    this.gl = gl;
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    this.disc = program(gl, DISC_VS, DISC_FS);
    this.line = program(gl, LINE_VS, LINE_FS);

    this.atomVao = gl.createVertexArray()!;
    this.atomBuf = gl.createBuffer()!;
    this.lobeVao = gl.createVertexArray()!;
    this.lobeBuf = gl.createBuffer()!;
    this.lineVao = gl.createVertexArray()!;
    this.lineBuf = gl.createBuffer()!;

    const quad = new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]);
    const qb = gl.createBuffer()!;
    gl.bindBuffer(gl.ARRAY_BUFFER, qb);
    gl.bufferData(gl.ARRAY_BUFFER, quad, gl.STATIC_DRAW);
    for (const vao of [this.atomVao, this.lobeVao]) {
      gl.bindVertexArray(vao);
      gl.bindBuffer(gl.ARRAY_BUFFER, qb);
      const loc = gl.getAttribLocation(this.disc, "corner");
      gl.enableVertexAttribArray(loc);
      gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
      gl.vertexAttribDivisor(loc, 0);
    }
    gl.bindVertexArray(null);
  }

  private rotate(x: number, y: number, z: number): [number, number, number] {
    const ct = Math.cos(this.rotY), st = Math.sin(this.rotY);
    const cp = Math.cos(this.rotX), sp = Math.sin(this.rotX);
    const x1 = x * ct + z * st;
    const z1 = -x * st + z * ct;
    const y2 = y * cp - z1 * sp;
    const z2 = y * sp + z1 * cp;
    return [x1, y2, z2];
  }

  private fit(atoms: AtomView[]) {
    const { width, height } = this.canvas;
    const aspect = width / height;
    if (atoms.length === 0) {
      this.scale = [0.2 * this.zoom, 0.2 * this.zoom * aspect];
      this.offset = [this.panX, this.panY];
      return;
    }
    let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
    for (const a of atoms) {
      const [x1, y2] = this.rotate(a.x, a.y, a.z ?? 0);
      minX = Math.min(minX, x1); maxX = Math.max(maxX, x1);
      minY = Math.min(minY, y2); maxY = Math.max(maxY, y2);
    }
    const spanX = Math.max(maxX - minX, 1.0) * 1.15 + 1.4;
    const spanY = Math.max(maxY - minY, 1.0) * 1.15 + 1.4;
    const s = Math.min(1.8 / spanX, 1.8 / (spanY * aspect)) * this.zoom;
    this.scale = [s, s * aspect];
    const cx = (minX + maxX) / 2, cy = (minY + maxY) / 2;
    this.offset = [-cx * this.scale[0] + this.panX, -cy * this.scale[1] + this.panY];
  }

  project(x: number, y: number, z: number): [number, number, number] {
    const [x1, y2, z2] = this.rotate(x, y, z);
    const f = PERSP / Math.max(1.0, PERSP - z2 * this.scale[0]);
    return [x1 * this.scale[0] * f + this.offset[0], y2 * this.scale[1] * f + this.offset[1], f];
  }

  toClip(x: number, y: number): [number, number] {
    const p = this.project(x, y, 0);
    return [p[0], p[1]];
  }

  fromClip(cx: number, cy: number): [number, number] {
    let f = 1;
    let x = 0, y = 0;
    for (let i = 0; i < 8; i++) {
      x = (cx - this.offset[0]) / (this.scale[0] * f);
      y = (cy - this.offset[1]) / (this.scale[1] * f);
      const [, , z2] = this.rotate(x, y, 0);
      f = PERSP / Math.max(1.0, PERSP - z2 * this.scale[0]);
    }
    return [x, y];
  }

  draw(atoms: AtomView[], bonds: [number, number][], lobes: Lobe[]) {
    const gl = this.gl;
    gl.viewport(0, 0, this.canvas.width, this.canvas.height);
    gl.clearColor(0.06, 0.07, 0.1, 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    this.fit(atoms);
    const aspect = this.canvas.width / this.canvas.height;

    // bonds as screen-space-thick quads, back-to-front depth tint
    const w2 = this.canvas.width / 2, h2 = this.canvas.height / 2;
    const lineData = new Float32Array(bonds.length * 12);
    let nverts = 0;
    for (const [i, j] of bonds) {
      if (!atoms[i] || !atoms[j]) continue;
      const [ax, ay] = this.project(atoms[i].x, atoms[i].y, atoms[i].z ?? 0);
      const [bx, by] = this.project(atoms[j].x, atoms[j].y, atoms[j].z ?? 0);
      const px1 = ax * w2, py1 = ay * h2, px2 = bx * w2, py2 = by * h2;
      let dx = px2 - px1, dy = py2 - py1;
      const len = Math.hypot(dx, dy) || 1;
      const nx = (-dy / len) * 1.9, ny = (dx / len) * 1.9;
      const q = [
        [px1 + nx, py1 + ny], [px1 - nx, py1 - ny],
        [px2 + nx, py2 + ny], [px2 - nx, py2 - ny],
      ].map(([x, y]) => [x / w2, y / h2]);
      lineData.set([q[0][0], q[0][1], q[1][0], q[1][1], q[2][0], q[2][1],
        q[2][0], q[2][1], q[1][0], q[1][1], q[3][0], q[3][1]], nverts * 2);
      nverts += 6;
    }
    gl.useProgram(this.line);
    gl.bindVertexArray(this.lineVao);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.lineBuf);
    gl.bufferData(gl.ARRAY_BUFFER, lineData, gl.DYNAMIC_DRAW);
    const lp = gl.getAttribLocation(this.line, "pos");
    gl.enableVertexAttribArray(lp);
    gl.vertexAttribPointer(lp, 2, gl.FLOAT, false, 0, 0);
    gl.uniform4f(gl.getUniformLocation(this.line, "color"), 0.6, 0.65, 0.75, 0.85);
    gl.drawArrays(gl.TRIANGLES, 0, nverts);

    gl.useProgram(this.disc);
    gl.uniform2f(gl.getUniformLocation(this.disc, "radiusScale"), 1, aspect);

    const drawDiscs = (
      vao: WebGLVertexArrayObject,
      buf: WebGLBuffer,
      data: Float32Array,
      count: number,
    ) => {
      if (count === 0) return;
      gl.bindVertexArray(vao);
      gl.bindBuffer(gl.ARRAY_BUFFER, buf);
      gl.bufferData(gl.ARRAY_BUFFER, data, gl.DYNAMIC_DRAW);
      const stride = 8 * 4;
      const cLoc = gl.getAttribLocation(this.disc, "center");
      const rLoc = gl.getAttribLocation(this.disc, "radius");
      const colLoc = gl.getAttribLocation(this.disc, "color");
      const shLoc = gl.getAttribLocation(this.disc, "shaded");
      gl.enableVertexAttribArray(cLoc);
      gl.vertexAttribPointer(cLoc, 2, gl.FLOAT, false, stride, 0);
      gl.vertexAttribDivisor(cLoc, 1);
      gl.enableVertexAttribArray(rLoc);
      gl.vertexAttribPointer(rLoc, 1, gl.FLOAT, false, stride, 8);
      gl.vertexAttribDivisor(rLoc, 1);
      gl.enableVertexAttribArray(colLoc);
      gl.vertexAttribPointer(colLoc, 4, gl.FLOAT, false, stride, 12);
      gl.vertexAttribDivisor(colLoc, 1);
      gl.enableVertexAttribArray(shLoc);
      gl.vertexAttribPointer(shLoc, 1, gl.FLOAT, false, stride, 28);
      gl.vertexAttribDivisor(shLoc, 1);
      gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, count);
    };

    // MO lobes under atoms (flat, unshaded)
    const lobeData = new Float32Array(lobes.length * 8);
    lobes.forEach((l, k) => {
      const [cx, cy, f] = this.project(l.x, l.y, 0);
      const col = l.sign >= 0 ? [0.9, 0.35, 0.2, 0.4] : [0.2, 0.45, 0.95, 0.4];
      lobeData.set([cx, cy, l.radius * this.scale[0] * f, ...col, 0], k * 8);
    });
    drawDiscs(this.lobeVao, this.lobeBuf, lobeData, lobes.length);

    // selection halo under atoms
    if (this.selected >= 0 && this.selected < atoms.length) {
      const sa = atoms[this.selected];
      const [cx, cy, f] = this.project(sa.x, sa.y, sa.z ?? 0);
      drawDiscs(this.lobeVao, this.lobeBuf, new Float32Array([cx, cy, 0.34 * this.scale[0] * f, 1, 1, 1, 0.45, 0]), 1);
    }

    // atoms: shaded spheres, depth-sorted far to near
    const order = atoms
      .map((a, i) => [this.rotate(a.x, a.y, a.z ?? 0)[2], i] as [number, number])
      .sort((p, q) => p[0] - q[0]);
    const atomData = new Float32Array(atoms.length * 8);
    order.forEach(([, i], k) => {
      const a = atoms[i];
      const rgb = CPK[a.symbol] ?? [0.7, 0.7, 0.4];
      const [cx, cy, f] = this.project(a.x, a.y, a.z ?? 0);
      atomData.set([cx, cy, 0.19 * this.scale[0] * f, rgb[0], rgb[1], rgb[2], 1, 1], k * 8);
    });
    drawDiscs(this.atomVao, this.atomBuf, atomData, atoms.length);
    gl.bindVertexArray(null);
  }
}
