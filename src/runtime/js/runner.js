/**
 * Grain Headless p5.js Runtime Runner
 * Executes p5.js sketches deterministically with frame-aligned audio features
 * and produces TrueColor character cell grids.
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
    this.maxA = 1;

    this.currentFill = [255, 255, 255, 1];
    this.currentStroke = [0, 200, 255, 1];
    this.doFill = true;
    this.doStroke = true;
    this.strokeWidth = 1;
    this.matrixStack = [];
    this.transform = { x: 0, y: 0, rot: 0, scaleX: 1, scaleY: 1 };
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
    if (typeof w === 'number') this.width = w;
    if (typeof h === 'number') this.height = h;
  }

  colorMode(mode, max1 = 255, max2 = 255, max3 = 255, maxA = 1) {
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
      this.maxA = maxA || 1;
    }
  }

  parseColor(r, g, b, a) {
    if (Array.isArray(r)) {
      return this.parseColor(r[0], r[1], r[2], r[3]);
    }
    if (typeof r === 'string') {
      // Hex or named fallback
      return [200, 200, 200, 1];
    }
    if (typeof r === 'number' && g === undefined) {
      // Grayscale
      const val = Math.max(0, Math.min(255, Math.round((r / this.max1) * 255)));
      return [val, val, val, 1];
    }
    if (typeof r === 'number' && typeof g === 'number' && b === undefined) {
      // Grayscale + Alpha
      const val = Math.max(0, Math.min(255, Math.round((r / this.max1) * 255)));
      const alpha = g / (this.max2 || 1);
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
  blendMode(mode) {}
  cursor() {}
  noCursor() {}
  smooth() {}
  noSmooth() {}

  createVector(x = 0, y = 0, z = 0) {
    return new P5Vector(x, y, z);
  }

  rectMode(mode) {}
  ellipseMode(mode) {}

  color(r, g, b, a) {
    return this.parseColor(r, g, b, a);
  }

  red(c) { return Array.isArray(c) ? c[0] : 255; }
  green(c) { return Array.isArray(c) ? c[1] : 255; }
  blue(c) { return Array.isArray(c) ? c[2] : 255; }
  alpha(c) { return Array.isArray(c) ? c[3] : 1; }

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
    this.commands.push({ type: 'background', color: col });
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

  circle(x, y, d) {
    this.commands.push({
      type: 'circle',
      x: x + this.transform.x,
      y: y + this.transform.y,
      radius: d / 2,
      fill: this.doFill ? this.currentFill : null,
      stroke: this.doStroke ? this.currentStroke : null
    });
  }

  ellipse(x, y, w, h = w) {
    this.commands.push({
      type: 'circle',
      x: x + this.transform.x,
      y: y + this.transform.y,
      radius: Math.max(w, h) / 2,
      fill: this.doFill ? this.currentFill : null,
      stroke: this.doStroke ? this.currentStroke : null
    });
  }

  point(x, y) {
    this.commands.push({
      type: 'point',
      x: x + this.transform.x,
      y: y + this.transform.y,
      stroke: this.doStroke ? this.currentStroke : this.currentFill,
    });
  }

  rect(x, y, w, h) {
    this.commands.push({
      type: 'rect',
      x: x + this.transform.x,
      y: y + this.transform.y,
      w,
      h,
      fill: this.doFill ? this.currentFill : null,
      stroke: this.doStroke ? this.currentStroke : null
    });
  }

  line(x1, y1, x2, y2) {
    this.commands.push({
      type: 'line',
      x1: x1 + this.transform.x,
      y1: y1 + this.transform.y,
      x2: x2 + this.transform.x,
      y2: y2 + this.transform.y,
      stroke: this.currentStroke,
      weight: this.strokeWidth
    });
  }

  triangle(x1, y1, x2, y2, x3, y3) {
    this.line(x1, y1, x2, y2);
    this.line(x2, y2, x3, y3);
    this.line(x3, y3, x1, y1);
  }

  quad(x1, y1, x2, y2, x3, y3, x4, y4) {
    this.line(x1, y1, x2, y2);
    this.line(x2, y2, x3, y3);
    this.line(x3, y3, x4, y4);
    this.line(x4, y4, x1, y1);
  }

  arc(x, y, w, h, start, stop) {
    this.ellipse(x, y, w, h);
  }

  beginShape() {
    this.shapeVertices = [];
  }

  vertex(x, y) {
    if (!this.shapeVertices) this.shapeVertices = [];
    this.shapeVertices.push({ x, y });
  }

  endShape(close = false) {
    if (this.shapeVertices && this.shapeVertices.length > 1) {
      for (let i = 0; i < this.shapeVertices.length - 1; i++) {
        this.line(
          this.shapeVertices[i].x,
          this.shapeVertices[i].y,
          this.shapeVertices[i + 1].x,
          this.shapeVertices[i + 1].y
        );
      }
      if (close) {
        const last = this.shapeVertices.length - 1;
        this.line(
          this.shapeVertices[last].x,
          this.shapeVertices[last].y,
          this.shapeVertices[0].x,
          this.shapeVertices[0].y
        );
      }
    }
  }

  push() {
    this.matrixStack.push({ ...this.transform });
  }

  pop() {
    if (this.matrixStack.length > 0) {
      this.transform = this.matrixStack.pop();
    }
  }

  translate(x, y) {
    this.transform.x += x;
    this.transform.y += y;
  }

  rotate(angle) {
    const rad = this.angleModeType === 'degrees' ? (angle * Math.PI) / 180 : angle;
    this.transform.rot += rad;
  }

  scale(s) {
    this.transform.scaleX *= s;
    this.transform.scaleY *= s;
  }

  shearX(angle) {}
  shearY(angle) {}
  resetMatrix() {
    this.transform = { x: 0, y: 0, rot: 0, scaleX: 1, scaleY: 1 };
  }
}

function renderCellsAndAscii(commands, width, height, termCols = 54, termRows = 12) {
  const charRamp = " .:-=+*#%@";

  // Grid of cells with RGB color
  const cellGrid = Array.from({ length: termRows }, () =>
    Array.from({ length: termCols }, () => ({ symbol: ' ', r: 0, g: 0, b: 0 }))
  );

  function setCell(c, r, symbol, color) {
    if (r >= 0 && r < termRows && c >= 0 && c < termCols) {
      cellGrid[r][c].symbol = symbol;
      if (color) {
        cellGrid[r][c].r = color[0] || 0;
        cellGrid[r][c].g = color[1] || 0;
        cellGrid[r][c].b = color[2] || 0;
      }
    }
  }

  for (const cmd of commands) {
    if (cmd.type === 'circle') {
      const col = Math.floor((cmd.x / width) * termCols);
      const row = Math.floor((cmd.y / height) * termRows);
      const radiusCols = Math.max(1, Math.floor((cmd.radius / width) * termCols));
      const radiusRows = Math.max(1, Math.floor((cmd.radius / height) * termRows * 0.5));
      const color = cmd.fill || cmd.stroke || [200, 200, 200];

      for (let r = Math.max(0, row - radiusRows); r <= Math.min(termRows - 1, row + radiusRows); r++) {
        for (let c = Math.max(0, col - radiusCols); c <= Math.min(termCols - 1, col + radiusCols); c++) {
          const dx = (c - col) / radiusCols;
          const dy = (r - row) / radiusRows;
          const distSq = dx * dx + dy * dy;
          if (distSq <= 1.0) {
            const intensity = 1.0 - distSq * 0.4;
            const charIdx = Math.min(charRamp.length - 1, Math.floor(intensity * (charRamp.length - 1)));
            setCell(c, r, charRamp[charIdx], color);
          }
        }
      }
    } else if (cmd.type === 'rect') {
      const c1 = Math.max(0, Math.floor((cmd.x / width) * termCols));
      const r1 = Math.max(0, Math.floor((cmd.y / height) * termRows));
      const c2 = Math.min(termCols - 1, Math.floor(((cmd.x + cmd.w) / width) * termCols));
      const r2 = Math.min(termRows - 1, Math.floor(((cmd.y + cmd.h) / height) * termRows));
      const color = cmd.fill || cmd.stroke || [180, 180, 180];

      for (let r = r1; r <= r2; r++) {
        for (let c = c1; c <= c2; c++) {
          setCell(c, r, '#', color);
        }
      }
    } else if (cmd.type === 'point') {
      const c = Math.floor((cmd.x / width) * termCols);
      const r = Math.floor((cmd.y / height) * termRows);
      setCell(c, r, '*', cmd.stroke || [255, 255, 255]);
    } else if (cmd.type === 'line') {
      // Bresenham's line algorithm on terminal grid
      const c0 = Math.floor((cmd.x1 / width) * termCols);
      const r0 = Math.floor((cmd.y1 / height) * termRows);
      const c1 = Math.floor((cmd.x2 / width) * termCols);
      const r1 = Math.floor((cmd.y2 / height) * termRows);
      const color = cmd.stroke || [100, 200, 255];

      let dx = Math.abs(c1 - c0);
      let dy = Math.abs(r1 - r0);
      let sx = c0 < c1 ? 1 : -1;
      let sy = r0 < r1 ? 1 : -1;
      let err = dx - dy;

      let curX = c0;
      let curY = r0;

      while (true) {
        setCell(curX, curY, '+', color);
        if (curX === c1 && curY === r1) break;
        let e2 = 2 * err;
        if (e2 > -dy) {
          err -= dy;
          curX += sx;
        }
        if (e2 < dx) {
          err += dx;
          curY += sy;
        }
      }
    }
  }

  const asciiArt = cellGrid.map(row => row.map(cell => cell.symbol).join('')).join('\n');
  return { asciiArt, cells: cellGrid };
}

function run() {
  let inputData = '';
  process.stdin.setEncoding('utf-8');

  process.stdin.on('data', chunk => {
    inputData += chunk;
  });

  process.stdin.on('end', () => {
    try {
      const req = JSON.parse(inputData);
      const { source, context, termCols, termRows } = req;

      const p5 = new HeadlessP5(context.width, context.height, context.seed);

      // Create sandbox context with global math/p5 aliases
      const sandboxFn = new Function(
        'p', 'ctx',
        'sin', 'cos', 'tan', 'abs', 'sqrt', 'floor', 'ceil', 'round', 'min', 'max', 'pow',
        'map', 'constrain', 'dist', 'lerp', 'noise', 'random', 'createVector', 'radians', 'degrees', 'sq',
        'PI', 'TWO_PI', 'TAU', 'HALF_PI', 'QUARTER_PI',
        `
        ${source}
        if (typeof setup === 'function') {
          setup(p);
        }
        if (typeof draw === 'function') {
          draw(p, ctx);
        }
      `);

      sandboxFn(
        p5, context,
        p5.sin.bind(p5), p5.cos.bind(p5), p5.tan.bind(p5),
        p5.abs.bind(p5), p5.sqrt.bind(p5), p5.floor.bind(p5), p5.ceil.bind(p5),
        p5.round.bind(p5), p5.min.bind(p5), p5.max.bind(p5), p5.pow.bind(p5),
        p5.map.bind(p5), p5.constrain.bind(p5), p5.dist.bind(p5), p5.lerp.bind(p5),
        p5.noise.bind(p5), p5.random.bind(p5), p5.createVector.bind(p5),
        p5.radians.bind(p5), p5.degrees.bind(p5), p5.sq.bind(p5),
        p5.PI, p5.TWO_PI, p5.TAU, p5.HALF_PI, p5.QUARTER_PI
      );

      const { asciiArt, cells } = renderCellsAndAscii(
        p5.commands,
        context.width,
        context.height,
        termCols || 54,
        termRows || 12
      );

      const response = {
        success: true,
        frame: context.frame,
        width: context.width,
        height: context.height,
        ascii_art: asciiArt,
        cells: cells,
        draw_commands_count: p5.commands.length
      };

      process.stdout.write(JSON.stringify(response));
    } catch (err) {
      let line = null;
      let col = null;
      if (err.stack) {
        const match = err.stack.match(/<anonymous>:(\d+):(\d+)/);
        if (match) {
          line = parseInt(match[1], 10);
          col = parseInt(match[2], 10);
        }
      }

      const errResponse = {
        success: false,
        error: {
          message: err.message || String(err),
          line,
          column: col,
          stack: err.stack
        }
      };

      process.stdout.write(JSON.stringify(errResponse));
    }
  });
}

run();
