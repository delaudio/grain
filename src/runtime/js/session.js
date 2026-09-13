// The returned callback is held privately by Rust, never on globalThis.
function makeGrainSession(source, initialContext) {
  const stringify = JSON.stringify.bind(JSON);
  const parse = JSON.parse.bind(JSON);
  const assign = Object.assign;
  const sourceLines = source.split('\n').length;
  const p = new HeadlessP5(initialContext.width, initialContext.height, initialContext.seed);
  const ctx = initialContext;
  const aliases = ['sin', 'cos', 'tan', 'abs', 'sqrt', 'floor', 'ceil', 'round', 'min', 'max', 'pow',
    'map', 'constrain', 'dist', 'lerp', 'noise', 'random', 'createVector', 'radians', 'degrees', 'sq'];
  const constants = ['PI', 'TWO_PI', 'TAU', 'HALF_PI', 'QUARTER_PI'];
  const compile = new Function('p', 'ctx', ...aliases, ...constants,
    `${source}\nreturn [typeof setup === 'function' ? setup : null, typeof draw === 'function' ? draw : null];`);
  const hooks = compile(p, ctx, ...aliases.map(name => p[name].bind(p)), ...constants.map(name => p[name]));
  if (!hooks[1]) throw new Error('Sketch must define draw(p, ctx)');
  const synchronous = (value, name) => {
    if (value && typeof value.then === 'function') throw new Error(name + ' must be synchronous');
  };
  p.millis = () => ctx.time * 1000;
  p.frameCount = ctx.frame;
  p.deltaTime = 0;
  // Reject asynchronous setup as well as draw: no pending job queue is run.
  if (hooks[0]) synchronous(hooks[0](p, ctx), 'setup(p, ctx)');
  let first = true;
  let previousTime = ctx.time;
  return function(encodedContext) {
    try {
      const next = parse(encodedContext);
      assign(ctx, next);
      p.frameCount = ctx.frame;
      p.deltaTime = Math.max(0, (ctx.time - previousTime) * 1000);
      previousTime = ctx.time;
      if (!first) p.commands = [];
      first = false;
      // p5 resets the transform each draw, but keeps styles and user state.
      p.resetMatrix();
      p.matrixStack = [];
      synchronous(hooks[1](p, ctx), 'draw(p, ctx)');
      return stringify({ success: true, width: p.width, height: p.height, commands: p.commands });
    } catch (err) {
      let line = null, column = null;
      const stack = err && typeof err.stack === 'string' ? err.stack : null;
      const match = stack && stack.match(/<input>:(\d+)(?::(\d+))?/);
      if (match) {
        const candidate = Number(match[1]) - 2;
        if (candidate > 0 && candidate <= sourceLines) {
          line = candidate;
          column = match[2] ? Number(match[2]) : null;
        }
      }
      return stringify({ success: false, error: {
        message: String(err && err.message || err), line, column, stack
      }});
    }
  };
}
