/**
 * Grain Headless p5.js Runtime Runner
 * Executes p5.js sketches deterministically with frame-aligned audio features.
 */

function createPRNG(seed) {
  let s = seed % 2147483647;
  if (s <= 0) s += 2147483646;
  return function() {
    s = (s * 16807) % 2147483647;
    return (s - 1) / 2147483646;
  };
}

class HeadlessP5 {
  constructor(width, height, seed) {
    this.width = width;
    this.height = height;
    this.commands = [];
    this.prng = createPRNG(seed || 42);
    this.currentFill = [255, 255, 255, 1];
    this.currentStroke = [0, 0, 0, 1];
    this.doFill = true;
    this.doStroke = true;
    this.strokeWidth = 1;
    this.matrixStack = [];
    this.transform = { x: 0, y: 0, rot: 0, scaleX: 1, scaleY: 1 };
  }

  random(min = 0, max = 1) {
    if (typeof min === 'number' && typeof max === 'number') {
      return min + this.prng() * (max - min);
    }
    return this.prng() * min;
  }

  background(r, g = r, b = r, a = 1) {
    this.commands.push({ type: 'background', color: [r, g, b, a] });
  }

  fill(r, g = r, b = r, a = 1) {
    this.doFill = true;
    this.currentFill = [r, g, b, a];
  }

  noFill() {
    this.doFill = false;
  }

  stroke(r, g = r, b = r, a = 1) {
    this.doStroke = true;
    this.currentStroke = [r, g, b, a];
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
    this.transform.rot += angle;
  }

  scale(s) {
    this.transform.scaleX *= s;
    this.transform.scaleY *= s;
  }
}

function renderAsciiPreview(commands, width, height, termCols = 54, termRows = 12) {
  const grid = Array.from({ length: termRows }, () => Array(termCols).fill(' '));
  const charRamp = " .:-=+*#%@";

  // Render circles and rects onto low-res terminal character grid
  for (const cmd of commands) {
    if (cmd.type === 'circle') {
      const col = Math.floor((cmd.x / width) * termCols);
      const row = Math.floor((cmd.y / height) * termRows);
      const radiusCols = Math.max(1, Math.floor((cmd.radius / width) * termCols));
      const radiusRows = Math.max(1, Math.floor((cmd.radius / height) * termRows * 0.5));

      for (let r = Math.max(0, row - radiusRows); r <= Math.min(termRows - 1, row + radiusRows); r++) {
        for (let c = Math.max(0, col - radiusCols); c <= Math.min(termCols - 1, col + radiusCols); c++) {
          const dx = (c - col) / radiusCols;
          const dy = (r - row) / radiusRows;
          const distSq = dx * dx + dy * dy;
          if (distSq <= 1.0) {
            const intensity = 1.0 - distSq * 0.5;
            const charIdx = Math.min(charRamp.length - 1, Math.floor(intensity * (charRamp.length - 1)));
            grid[r][c] = charRamp[charIdx];
          }
        }
      }
    } else if (cmd.type === 'rect') {
      const c1 = Math.max(0, Math.floor((cmd.x / width) * termCols));
      const r1 = Math.max(0, Math.floor((cmd.y / height) * termRows));
      const c2 = Math.min(termCols - 1, Math.floor(((cmd.x + cmd.w) / width) * termCols));
      const r2 = Math.min(termRows - 1, Math.floor(((cmd.y + cmd.h) / height) * termRows));

      for (let r = r1; r <= r2; r++) {
        for (let c = c1; c <= c2; c++) {
          grid[r][c] = '#';
        }
      }
    }
  }

  return grid.map(row => row.join('')).join('\n');
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

      // Create isolated sandbox context
      const sandboxFn = new Function('p', 'ctx', `
        ${source}
        if (typeof setup === 'function') {
          setup(p);
        }
        if (typeof draw === 'function') {
          draw(p, ctx);
        }
      `);

      sandboxFn(p5, context);

      const asciiArt = renderAsciiPreview(
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
