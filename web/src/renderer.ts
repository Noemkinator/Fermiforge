// Minimal WebGL2 renderer: atoms as discs, bonds as lines, MO lobes.

export interface AtomView {
  x: number;
  y: number;
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
  C: [0.25, 0.25, 0.25],
  N: [0.19, 0.31, 0.97],
  O: [0.94, 0.16, 0.16],
  F: [0.56, 0.88, 0.31],
  Si: [0.94, 0.78, 0.63],
  Cl: [0.12, 0.94, 0.12],
  Br: [0.65, 0.16, 0.16],
  Fe: [0.89, 0.47, 0.13],
};

const DISC_VS = `#version 300 es
in vec2 corner;
in vec2 center;
in float radius;
in vec4 color;
out vec2 v_uv;
out vec4 v_color;
uniform vec2 scale;
uniform vec2 offset;
void main() {
  v_uv = corner;
  v_color = color;
  vec2 pos = center + corner * radius;
  gl_Position = vec4(pos * scale + offset, 0.0, 1.0);
}`;

const DISC_FS = `#version 300 es
precision mediump float;
in vec2 v_uv;
in vec4 v_color;
out vec4 outColor;
void main() {
  float d = length(v_uv);
  if (d > 1.0) discard;
  float edge = smoothstep(1.0, 0.88, d);
  outColor = vec4(v_color.rgb, v_color.a * edge);
}`;

const LINE_VS = `#version 300 es
in vec2 pos;
uniform vec2 scale;
uniform vec2 offset;
void main() { gl_Position = vec4(pos * scale + offset, 0.0, 1.0); }`;

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

export class Renderer {
  private gl: WebGL2RenderingContext;
  private disc: WebGLProgram;
  private line: WebGLProgram;
  private quadVao: WebGLVertexArrayObject;
  private atomVao: WebGLVertexArrayObject;
  private atomBuf: WebGLBuffer;
  private lobeVao: WebGLVertexArrayObject;
  private lobeBuf: WebGLBuffer;
  private lineVao: WebGLVertexArrayObject;
  private lineBuf: WebGLBuffer;
  scale: [number, number] = [1, 1];
  offset: [number, number] = [0, 0];

  constructor(private canvas: HTMLCanvasElement) {
    const gl = canvas.getContext("webgl2", { antialias: true, alpha: true });
    if (!gl) throw new Error("WebGL2 not available");
    this.gl = gl;
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    this.disc = program(gl, DISC_VS, DISC_FS);
    this.line = program(gl, LINE_VS, LINE_FS);

    this.quadVao = gl.createVertexArray()!;
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

  private fit(atoms: AtomView[]) {
    const { width, height } = this.canvas;
    const aspect = width / height;
    if (atoms.length === 0) {
      this.scale = [0.2, 0.2 / aspect];
      this.offset = [0, 0];
      return;
    }
    let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
    for (const a of atoms) {
      minX = Math.min(minX, a.x); maxX = Math.max(maxX, a.x);
      minY = Math.min(minY, a.y); maxY = Math.max(maxY, a.y);
    }
    const spanX = Math.max(maxX - minX, 1.0) + 1.4;
    const spanY = Math.max(maxY - minY, 1.0) + 1.4;
    const s = Math.min(1.8 / spanX, (1.8 * aspect) / spanY);
    this.scale = [s, s / aspect];
    const cx = (minX + maxX) / 2, cy = (minY + maxY) / 2;
    this.offset = [-cx * this.scale[0], -cy * this.scale[1]];
  }

  toClip(x: number, y: number): [number, number] {
    return [x * this.scale[0] + this.offset[0], y * this.scale[1] + this.offset[1]];
  }

  fromClip(cx: number, cy: number): [number, number] {
    return [(cx - this.offset[0]) / this.scale[0], (cy - this.offset[1]) / this.scale[1]];
  }

  draw(atoms: AtomView[], bonds: [number, number][], lobes: Lobe[]) {
    const gl = this.gl;
    gl.viewport(0, 0, this.canvas.width, this.canvas.height);
    gl.clearColor(0.07, 0.08, 0.11, 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    this.fit(atoms);

    // bonds as screen-space-thick quads (gl.LINES is capped at 1px)
    const w2 = this.canvas.width / 2, h2 = this.canvas.height / 2;
    const lineData = new Float32Array(bonds.length * 12);
    let nverts = 0;
    for (const [i, j] of bonds) {
      if (!atoms[i] || !atoms[j]) continue;
      const [ax, ay] = this.toClip(atoms[i].x, atoms[i].y);
      const [bx, by] = this.toClip(atoms[j].x, atoms[j].y);
      // perpendicular in pixel space for uniform visual width
      const px1 = ax * w2, py1 = ay * h2, px2 = bx * w2, py2 = by * h2;
      let dx = px2 - px1, dy = py2 - py1;
      const len = Math.hypot(dx, dy) || 1;
      const nx = (-dy / len) * 1.6, ny = (dx / len) * 1.6;
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
    gl.uniform2f(gl.getUniformLocation(this.line, "scale"), 1, 1);
    gl.uniform2f(gl.getUniformLocation(this.line, "offset"), 0, 0);
    gl.uniform4f(gl.getUniformLocation(this.line, "color"), 0.55, 0.6, 0.7, 0.9);
    gl.drawArrays(gl.TRIANGLES, 0, nverts);

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
      const stride = 7 * 4;
      const cLoc = gl.getAttribLocation(this.disc, "center");
      const rLoc = gl.getAttribLocation(this.disc, "radius");
      const colLoc = gl.getAttribLocation(this.disc, "color");
      gl.enableVertexAttribArray(cLoc);
      gl.vertexAttribPointer(cLoc, 2, gl.FLOAT, false, stride, 0);
      gl.vertexAttribDivisor(cLoc, 1);
      gl.enableVertexAttribArray(rLoc);
      gl.vertexAttribPointer(rLoc, 1, gl.FLOAT, false, stride, 8);
      gl.vertexAttribDivisor(rLoc, 1);
      gl.enableVertexAttribArray(colLoc);
      gl.vertexAttribPointer(colLoc, 4, gl.FLOAT, false, stride, 12);
      gl.vertexAttribDivisor(colLoc, 1);
      gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, count);
    };

    gl.useProgram(this.disc);
    gl.uniform2fv(gl.getUniformLocation(this.disc, "scale"), this.scale);
    gl.uniform2fv(gl.getUniformLocation(this.disc, "offset"), this.offset);

    // MO lobes under atoms
    const lobeData = new Float32Array(lobes.length * 7);
    lobes.forEach((l, k) => {
      const col = l.sign >= 0 ? [0.9, 0.35, 0.2, 0.42] : [0.2, 0.45, 0.95, 0.42];
      lobeData.set([l.x, l.y, l.radius, ...col], k * 7);
    });
    drawDiscs(this.lobeVao, this.lobeBuf, lobeData, lobes.length);

    // atoms: small CPK-tinted discs, lightened for the dark background
    const atomData = new Float32Array(atoms.length * 7);
    atoms.forEach((a, k) => {
      const rgb = CPK[a.symbol] ?? [0.7, 0.7, 0.4];
      const light = (c: number) => Math.min(1, c * 0.7 + 0.42);
      atomData.set([a.x, a.y, 0.18, light(rgb[0]), light(rgb[1]), light(rgb[2]), 1], k * 7);
    });
    drawDiscs(this.atomVao, this.atomBuf, atomData, atoms.length);
    gl.bindVertexArray(null);
  }
}
