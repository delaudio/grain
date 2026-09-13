/**
 * Grain Headless p5.js Runtime Runner
 * Executes p5.js sketches deterministically with frame-aligned audio features
 * and produces bounded raster drawing commands.
 */

function createPRNG(seed) {
  let s = seed % 2147483647;
  if (s <= 0) s += 2147483646;
  return function() {
    s = (s * 16807) % 2147483647;
    return (s - 1) / 2147483646;
  };
}

// Simple deterministic Perlin-like 1D/2D/3D noise
function createNoise(seed) {
  const p = new Uint8Array(512);
  const prng = createPRNG(seed || 1337);
  const perm = Array.from({ length: 256 }, (_, i) => i);
  for (let i = 255; i > 0; i--) {
    const j = Math.floor(prng() * (i + 1));
    [perm[i], perm[j]] = [perm[j], perm[i]];
  }
  for (let i = 0; i < 512; i++) {
    p[i] = perm[i & 255];
  }

  function fade(t) { return t * t * t * (t * (t * 6 - 15) + 10); }
  function lerp(t, a, b) { return a + t * (b - a); }
  function grad(hash, x, y) {
    const h = hash & 3;
    const u = h < 2 ? x : y;
    const v = h < 2 ? y : x;
    return ((h & 1) === 0 ? u : -u) + ((h & 2) === 0 ? v : -v);
  }

  return function(x = 0, y = 0) {
    const X = Math.floor(x) & 255;
    const Y = Math.floor(y) & 255;
    const xf = x - Math.floor(x);
    const yf = y - Math.floor(y);
    const u = fade(xf);
    const v = fade(yf);

    const a = p[X] + Y;
    const aa = p[a];
    const ab = p[a + 1];
    const b = p[X + 1] + Y;
    const ba = p[b];
    const bb = p[b + 1];

    const res = lerp(v,
      lerp(u, grad(p[aa], xf, yf), grad(p[ba], xf - 1, yf)),
      lerp(u, grad(p[ab], xf, yf - 1), grad(p[bb], xf - 1, yf - 1))
    );
    return (res + 1) / 2;
  };
}

function hsbToRgb(h, s, v) {
  h = ((h % 360) + 360) % 360;
  s = Math.max(0, Math.min(100, s)) / 100;
  v = Math.max(0, Math.min(100, v)) / 100;
  const c = v * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = v - c;
  let r = 0, g = 0, b = 0;
  if (h < 60) { r = c; g = x; b = 0; }
  else if (h < 120) { r = x; g = c; b = 0; }
  else if (h < 180) { r = 0; g = c; b = x; }
  else if (h < 240) { r = 0; g = x; b = c; }
  else if (h < 300) { r = x; g = 0; b = c; }
  else { r = c; g = 0; b = x; }
  return [Math.round((r + m) * 255), Math.round((g + m) * 255), Math.round((b + m) * 255)];
}

class P5Vector {
  constructor(x = 0, y = 0, z = 0) {
    this.x = x;
    this.y = y;
    this.z = z;
  }

  set(x, y, z) {
    if (x instanceof P5Vector) {
      this.x = x.x; this.y = x.y; this.z = x.z;
    } else {
      this.x = x || 0; this.y = y || 0; this.z = z || 0;
    }
    return this;
  }

  copy() { return new P5Vector(this.x, this.y, this.z); }

  add(x, y = 0, z = 0) {
    if (x instanceof P5Vector) {
      this.x += x.x; this.y += x.y; this.z += x.z;
    } else {
      this.x += x; this.y += y; this.z += z;
    }
    return this;
  }

  sub(x, y = 0, z = 0) {
    if (x instanceof P5Vector) {
      this.x -= x.x; this.y -= x.y; this.z -= x.z;
    } else {
      this.x -= x; this.y -= y; this.z -= z;
    }
    return this;
  }

  mult(n) {
    this.x *= n; this.y *= n; this.z *= n;
    return this;
  }

  div(n) {
    if (n !== 0) {
      this.x /= n; this.y /= n; this.z /= n;
    }
    return this;
  }

  magSq() { return this.x * this.x + this.y * this.y + this.z * this.z; }
  mag() { return Math.sqrt(this.magSq()); }
  heading() { return Math.atan2(this.y, this.x); }

  normalize() {
    const m = this.mag();
    if (m !== 0) this.div(m);
    return this;
  }

  setMag(len) {
    return this.normalize().mult(len);
  }

  limit(max) {
    const mSq = this.magSq();
    if (mSq > max * max) {
      this.div(Math.sqrt(mSq)).mult(max);
    }
    return this;
  }

  dist(v) {
    const dx = this.x - v.x;
    const dy = this.y - v.y;
    const dz = this.z - v.z;
    return Math.sqrt(dx * dx + dy * dy + dz * dz);
  }

  static fromAngle(angle, length = 1) {
    return new P5Vector(length * Math.cos(angle), length * Math.sin(angle), 0);
  }

  static random2D() {
    return P5Vector.fromAngle(Math.random() * Math.PI * 2);
  }
}

// Parsed colors are distinct from raw component arrays: their alpha is already
// normalized and must not be divided by the active color range a second time.
class GrainColor {
  constructor(rgba) { this.rgba = rgba.slice(); }
}

class HeadlessP5 {
  constructor(width, height, seed) {
    this.width = width;
    this.height = height;
    this.commands = [];
    this.seed = seed || 42;
    this.prng = createPRNG(this.seed);
    this.noiseGen = createNoise(this.seed);

    this.colorModeType = 'rgb'; // 'rgb' or 'hsb'
    this.max1 = 255;
    this.max2 = 255;
    this.max3 = 255;
    this.maxA = 255;

    this.currentFill = [255, 255, 255, 1];
    this.currentStroke = [0, 0, 0, 1];
    this.doFill = true;
    this.doStroke = true;
    this.strokeWidth = 1;
    this.matrixStack = [];
    this.transform = [1, 0, 0, 1, 0, 0];
    this.rectModeType = 'corner';
    this.ellipseModeType = 'center';
    this.angleModeType = 'radians';

    // Vector helper
    this.Vector = P5Vector;

    // Constants
    this.PI = Math.PI;
    this.TWO_PI = Math.PI * 2;
    this.TAU = Math.PI * 2;
    this.HALF_PI = Math.PI / 2;
    this.QUARTER_PI = Math.PI / 4;
    this.RGB = 'rgb';
    this.HSB = 'hsb';
    this.CENTER = 'center';
    this.RADIUS = 'radius';
    this.CORNER = 'corner';
    this.CORNERS = 'corners';
    this.CLOSE = 'close';
    this.DEGREES = 'degrees';
    this.RADIANS = 'radians';
  }

  createCanvas(w, h) {
    if (!Number.isInteger(w) || !Number.isInteger(h) || w <= 0 || h <= 0 ||
        w > 4096 || h > 4096 || w * h > 4194304) {
      throw new Error('Canvas exceeds dimension or 4 megapixel limit');
    }
    this.width = w;
    this.height = h;
  }

  colorMode(mode, max1, max2, max3, maxA) {
    const m = String(mode).toLowerCase();
    if (m === 'hsb') {
      this.colorModeType = 'hsb';
      this.max1 = max1 || 360;
      this.max2 = max2 || 100;
      this.max3 = max3 || 100;
      this.maxA = maxA || 1;
    } else {
      this.colorModeType = 'rgb';
      this.max1 = max1 || 255;
      this.max2 = max2 || 255;
      this.max3 = max3 || 255;
      this.maxA = maxA || 255;
    }
  }

  parseColor(r, g, b, a) {
    if (r instanceof GrainColor) return r.rgba.slice();
    if (Array.isArray(r)) {
      return this.parseColor(r[0], r[1], r[2], r[3]);
    }
    if (typeof r === 'string') {
      const names = { black: '#000000', white: '#ffffff', red: '#ff0000',
        green: '#008000', blue: '#0000ff', yellow: '#ffff00', cyan: '#00ffff',
        magenta: '#ff00ff', transparent: '#00000000' };
      let hex = names[r.toLowerCase()] || r;
      if (/^#[0-9a-f]{3,4}$/i.test(hex)) {
        hex = '#' + hex.slice(1).split('').map(c => c + c).join('');
      }
      if (!/^#[0-9a-f]{6}([0-9a-f]{2})?$/i.test(hex)) {
        throw new Error('Unsupported color: use RGB/HSB components or a hex color');
      }
      return [parseInt(hex.slice(1, 3), 16), parseInt(hex.slice(3, 5), 16),
        parseInt(hex.slice(5, 7), 16), hex.length === 9 ? parseInt(hex.slice(7, 9), 16) / 255 : 1];
    }
    if (typeof r === 'number' && g === undefined) {
      // Grayscale
      const val = Math.max(0, Math.min(255, Math.round((r / this.max1) * 255)));
      return [val, val, val, 1];
    }
    if (typeof r === 'number' && typeof g === 'number' && b === undefined) {
      // Grayscale + Alpha
      const val = Math.max(0, Math.min(255, Math.round((r / this.max1) * 255)));
      const alpha = g / this.maxA;
      return [val, val, val, alpha];
    }

    const valR = r !== undefined ? r : 255;
    const valG = g !== undefined ? g : 255;
    const valB = b !== undefined ? b : 255;
    const valA = a !== undefined ? a / this.maxA : 1;

    if (this.colorModeType === 'hsb') {
      const h = (valR / this.max1) * 360;
      const s = (valG / this.max2) * 100;
      const v = (valB / this.max3) * 100;
      const rgb = hsbToRgb(h, s, v);
      return [rgb[0], rgb[1], rgb[2], valA];
    } else {
      const red = Math.max(0, Math.min(255, Math.round((valR / this.max1) * 255)));
      const green = Math.max(0, Math.min(255, Math.round((valG / this.max2) * 255)));
      const blue = Math.max(0, Math.min(255, Math.round((valB / this.max3) * 255)));
      return [red, green, blue, valA];
    }
  }

  randomSeed(s) {
    this.prng = createPRNG(s || 42);
  }

  noiseSeed(s) {
    this.noiseGen = createNoise(s || 42);
  }

  frameRate(fps) {}
  noLoop() {}
  loop() {}
  redraw() {}
  blendMode(mode) {
    if (mode !== 'source-over' && mode !== 'blend') throw new Error('Unsupported blendMode');
  }
  cursor() {}
  noCursor() {}
  smooth() {}
  noSmooth() {}

  createVector(x = 0, y = 0, z = 0) {
    return new P5Vector(x, y, z);
  }

rectMode(mode) {
    if (!['corner', 'corners', 'center', 'radius'].includes(mode)) throw new Error('Unsupported rectMode');
    this.rectModeType = mode;
  }
  ellipseMode(mode) {
    if (!['corner', 'corners', 'center', 'radius'].includes(mode)) throw new Error('Unsupported ellipseMode');
    this.ellipseModeType = mode;
  }

  color(r, g, b, a) {
    return new GrainColor(this.parseColor(r, g, b, a));
  }

  red(c) { return this.parseColor(c)[0]; }
  green(c) { return this.parseColor(c)[1]; }
  blue(c) { return this.parseColor(c)[2]; }
  alpha(c) { return this.parseColor(c)[3] * this.maxA; }

  angleMode(mode) {
    if (mode === 'degrees' || mode === 'DEGREES') {
      this.angleModeType = 'degrees';
    } else {
      this.angleModeType = 'radians';
    }
  }

  radians(deg) { return (deg * Math.PI) / 180; }
  degrees(rad) { return (rad * 180) / Math.PI; }
  sq(n) { return n * n; }
  norm(value, start, stop) { return this.map(value, start, stop, 0, 1); }
  mag(x, y) { return Math.hypot(x, y); }

  random(min = 0, max = 1) {
    if (typeof min === 'number' && typeof max === 'number') {
      return min + this.prng() * (max - min);
    }
    return this.prng() * min;
  }

  noise(x = 0, y = 0) {
    return this.noiseGen(x, y);
  }

  map(value, start1, stop1, start2, stop2) {
    return start2 + (stop2 - start2) * ((value - start1) / (stop1 - start1));
  }

  constrain(n, low, high) {
    return Math.max(Math.min(n, high), low);
  }

  dist(x1, y1, x2, y2) {
    return Math.hypot(x2 - x1, y2 - y1);
  }

  lerp(start, stop, amt) {
    return start + (stop - start) * amt;
  }

  sin(a) {
    const angle = this.angleModeType === 'degrees' ? (a * Math.PI) / 180 : a;
    return Math.sin(angle);
  }

  cos(a) {
    const angle = this.angleModeType === 'degrees' ? (a * Math.PI) / 180 : a;
    return Math.cos(angle);
  }

  tan(a) {
    const angle = this.angleModeType === 'degrees' ? (a * Math.PI) / 180 : a;
    return Math.tan(angle);
  }

  abs(n) { return Math.abs(n); }
  sqrt(n) { return Math.sqrt(n); }
  floor(n) { return Math.floor(n); }
  ceil(n) { return Math.ceil(n); }
  round(n) { return Math.round(n); }
  min(...args) { return Math.min(...args); }
  max(...args) { return Math.max(...args); }
  pow(n, e) { return Math.pow(n, e); }

  background(r, g, b, a) {
    const col = this.parseColor(r, g, b, a);
    this.emit({ type: 'background', color: col });
  }

  fill(r, g, b, a) {
    this.doFill = true;
    this.currentFill = this.parseColor(r, g, b, a);
  }

  noFill() {
    this.doFill = false;
  }

  stroke(r, g, b, a) {
    this.doStroke = true;
    this.currentStroke = this.parseColor(r, g, b, a);
  }

  noStroke() {
    this.doStroke = false;
  }

  strokeWeight(w) {
    this.strokeWidth = w;
  }

  emit(command) {
    if (this.commands.length >= 4096) throw new Error('Sketch exceeds the 4096 drawing command limit');
    this.commands.push(command);
  }

  style() {
    return { matrix: this.transform.slice(), fill: this.doFill ? this.currentFill.slice() : null,
      stroke: this.doStroke ? this.currentStroke.slice() : null, weight: this.strokeWidth };
  }

  clear() { this.emit({ type: 'clear' }); }

  circle(x, y, d) { this.ellipse(x, y, d, d); }

  ellipse(x, y, w, h = w) {
    if (this.ellipseModeType === 'corner') { x += w / 2; y += h / 2; }
    else if (this.ellipseModeType === 'corners') { w -= x; h -= y; x += w / 2; y += h / 2; }
    else if (this.ellipseModeType === 'radius') { w *= 2; h *= 2; }
    this.emit({ type: 'ellipse', x, y, w, h, style: this.style() });
  }

  point(x, y) {
    if (!this.doStroke || this.strokeWidth <= 0) return;
    const style = this.style();
    style.fill = style.stroke;
    style.stroke = null;
    this.emit({ type: 'ellipse', x, y, w: this.strokeWidth, h: this.strokeWidth, style });
  }

  rect(x, y, w, h) {
    if (this.rectModeType === 'center') { x -= w / 2; y -= h / 2; }
    else if (this.rectModeType === 'radius') { x -= w; y -= h; w *= 2; h *= 2; }
    else if (this.rectModeType === 'corners') { w -= x; h -= y; }
    this.emit({ type: 'rect', x, y, w, h, style: this.style() });
  }

  line(x1, y1, x2, y2) {
    const style = this.style();
    style.fill = null;
    this.emit({ type: 'path', vertices: [[x1, y1], [x2, y2]], close: false, style });
  }

  triangle(x1, y1, x2, y2, x3, y3) {
    this.emit({ type: 'path', vertices: [[x1, y1], [x2, y2], [x3, y3]], close: true, style: this.style() });
  }

  quad(x1, y1, x2, y2, x3, y3, x4, y4) {
    this.emit({ type: 'path', vertices: [[x1, y1], [x2, y2], [x3, y3], [x4, y4]], close: true, style: this.style() });
  }

  arc() { throw new Error('arc is not supported by the Grain p5 subset'); }

  beginShape(mode) {
    if (mode !== undefined) throw new Error('Only vertex paths are supported by beginShape');
    this.shapeVertices = [];
  }

  vertex(x, y) {
    if (!this.shapeVertices) throw new Error('vertex requires beginShape');
    if (this.shapeVertices.length >= 4096) throw new Error('Sketch exceeds the 4096 vertex limit');
    this.shapeVertices.push([x, y]);
  }

  endShape(close = false) {
    if (!this.shapeVertices) throw new Error('endShape requires beginShape');
    this.emit({ type: 'path', vertices: this.shapeVertices, close: close === true || close === this.CLOSE, style: this.style() });
    this.shapeVertices = null;
  }

  push() {
    if (this.matrixStack.length >= 256) throw new Error('Drawing state stack exceeds 256 entries');
    this.matrixStack.push({ transform: this.transform.slice(), fill: this.currentFill.slice(),
      stroke: this.currentStroke.slice(), doFill: this.doFill, doStroke: this.doStroke,
      weight: this.strokeWidth, rectMode: this.rectModeType, ellipseMode: this.ellipseModeType,
      colorMode: this.colorModeType, ranges: [this.max1, this.max2, this.max3, this.maxA] });
  }

  pop() {
    const state = this.matrixStack.pop();
    if (!state) throw new Error('pop requires a matching push');
    this.transform = state.transform;
    this.currentFill = state.fill; this.currentStroke = state.stroke;
    this.doFill = state.doFill; this.doStroke = state.doStroke;
    this.strokeWidth = state.weight;
    this.rectModeType = state.rectMode; this.ellipseModeType = state.ellipseMode;
    this.colorModeType = state.colorMode;
    [this.max1, this.max2, this.max3, this.maxA] = state.ranges;
  }

  applyMatrix(a, b, c, d, e, f) {
    const m = this.transform;
    this.transform = [m[0] * a + m[2] * b, m[1] * a + m[3] * b,
      m[0] * c + m[2] * d, m[1] * c + m[3] * d,
      m[0] * e + m[2] * f + m[4], m[1] * e + m[3] * f + m[5]];
  }

  translate(x, y) { this.applyMatrix(1, 0, 0, 1, x, y); }
  rotate(angle) {
    const rad = this.angleModeType === 'degrees' ? this.radians(angle) : angle;
    const c = Math.cos(rad), s = Math.sin(rad);
    this.applyMatrix(c, s, -s, c, 0, 0);
  }
  scale(x, y = x) { this.applyMatrix(x, 0, 0, y, 0, 0); }
  shearX(angle) { this.applyMatrix(1, 0, this.tan(angle), 1, 0, 0); }
  shearY(angle) { this.applyMatrix(1, this.tan(angle), 0, 1, 0, 0); }
  resetMatrix() { this.transform = [1, 0, 0, 1, 0, 0]; }
}

function run(req) {
    // Capture before user code can replace the global JSON binding.
    const stringify = JSON.stringify.bind(JSON);
    const sourceLineCount = req.source.split('\n').length;
    try {
      const { source, context, termCols, termRows } = req;
      Object.freeze(context.params);

      const p5 = new HeadlessP5(context.width, context.height, context.seed);

      // Aliases are a convenience, not an isolation boundary. The host creates
      // a capability-free QuickJS runtime with heap, stack and time limits.
      const sandboxFn = new Function(
        'p', 'ctx',
        'sin', 'cos', 'tan', 'abs', 'sqrt', 'floor', 'ceil', 'round', 'min', 'max', 'pow',
        'map', 'constrain', 'dist', 'lerp', 'noise', 'random', 'createVector', 'radians', 'degrees', 'sq',
        'PI', 'TWO_PI', 'TAU', 'HALF_PI', 'QUARTER_PI',
        `${source}
        return [typeof setup === 'function' ? setup : null, typeof draw === 'function' ? draw : null];
      `);

      const hooks = sandboxFn(
        p5, context,
        p5.sin.bind(p5), p5.cos.bind(p5), p5.tan.bind(p5),
        p5.abs.bind(p5), p5.sqrt.bind(p5), p5.floor.bind(p5), p5.ceil.bind(p5),
        p5.round.bind(p5), p5.min.bind(p5), p5.max.bind(p5), p5.pow.bind(p5),
        p5.map.bind(p5), p5.constrain.bind(p5), p5.dist.bind(p5), p5.lerp.bind(p5),
        p5.noise.bind(p5), p5.random.bind(p5), p5.createVector.bind(p5),
        p5.radians.bind(p5), p5.degrees.bind(p5), p5.sq.bind(p5),
        p5.PI, p5.TWO_PI, p5.TAU, p5.HALF_PI, p5.QUARTER_PI
      );
      if (!hooks[1]) throw new Error('Sketch must define draw(p, ctx)');
      if (hooks[0]) {
        const setupResult = hooks[0](p5, context);
        if (setupResult && typeof setupResult.then === 'function') {
          throw new Error('setup(p, ctx) must be synchronous');
        }
      }
      const result = hooks[1](p5, context);
      if (result && typeof result.then === 'function') {
        throw new Error('draw(p, ctx) must be synchronous');
      }

      const response = {
        success: true,
        width: p5.width,
        height: p5.height,
        commands: p5.commands
      };

      return stringify(response);
    } catch (err) {
      let line = null;
      let col = null;
      if (err && typeof err.stack === 'string') {
        // QuickJS Function bodies start on line 3 of the synthetic <input>
        // source. The sketch starts at body column 1; helper calls appended
        // after it must not be reported as locations in the user's sketch.
        const match = err.stack.match(/<input>:(\d+)(?::(\d+))?/);
        if (match) {
          const sourceLine = parseInt(match[1], 10) - 2;
          if (sourceLine >= 1 && sourceLine <= sourceLineCount) {
            line = sourceLine;
            col = match[2] ? parseInt(match[2], 10) : null;
          }
        }
      }

      const errResponse = {
        success: false,
        error: {
          message: String(err && err.message || err),
          line,
          column: col,
          stack: err && typeof err.stack === 'string' ? err.stack : null
        }
      };

      return stringify(errResponse);
    }
}
