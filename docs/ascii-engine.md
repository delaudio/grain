# Native ASCII engine, grain-ascii-v1

The runtime emits terminal cells directly, not a raster image. It is available
through `AsciiSession`, the `EngineAdapter` interface, and
`PreviewWorker::submit_for_engine(EngineId::Ascii, request)`. Application engine
selection and generation integration are still in progress.

## Sketch API

Define `main`, optionally with `boot`, `pre`, and `post`. Plain function
declarations and named ES-module function declarations are accepted. Hooks must
be synchronous. External imports, top-level await, promises returned by hooks,
and browser services are unsupported.

```js
export function boot(context, buffer, data) {
    data.offset = Math.floor(Math.random() * 26);
}

export function main(coord, context, cursor, buffer, data) {
    const phase = Math.floor(context.time * 4);
    return {
        char: String.fromCharCode(65 + (coord.x + coord.y + phase + data.offset) % 26),
        color: [230, Math.round(80 + context.audio.amplitude * 150), 60],
        backgroundColor: '#101820'
    };
}
```

This example is original Grain code, not an upstream Play example.

`boot(context, buffer, data)` runs once before the first rendered frame.
`pre(context, cursor, buffer, data)` runs before the cells.
`main(coord, context, cursor, buffer, data)` runs in row-major order.
`post(context, cursor, buffer, data)` runs after the cells and may overlay them.
`coord` contains `x`, `y`, and linear `index`.

Return a character or `{char, color, backgroundColor}`. `undefined` retains the
previous buffer entry. The flat buffer persists across frames; on grid resize
it is replaced with spaces, but module variables and `data` survive. Buffer
length must stay equal to columns times rows. Both colors accept three integer
RGB bytes or `#rrggbb`. Defaults are `[230,230,230]` on `[0,0,0]`. CSS color names,
alpha, font weight, and other cell fields are errors, not silently ignored.
See [the frame contract](engine-contract.md) for glyph-width restrictions.

## Shared inputs and lifecycle

`context` contains `cols`, `rows`, logical canvas `width`/`height`, `frame`,
`time` in **seconds**, `seed`, and `audio` (`amplitude`, `low`, `mid`, `high`).
`seedText` preserves the exact unsigned 64-bit seed; JavaScript numbers cannot
represent all such integers. `Math.random()` is initialized from all seed digits
before module evaluation. `Date` is unavailable; animation uses host time.
`metrics.aspect` is cell width divided by height, default `0.5`. It is an explicit
host value, not a browser font measurement. Context and audio are read-only.

Repeated identical requests return the cached frame without calling hooks.
Changed audio or grid dimensions rerun the hooks even at the same time/frame.
This means a hook count is not a playback clock. A backward seek, changed seed,
logical canvas resize, source reload, or explicit restart needs a new session.
The new session starts from its seed at the requested host position; it does
not replay every skipped historical frame. Forward frame skips invoke hooks
only at the selected position.

## Isolation and budgets

Each session owns an embedded QuickJS runtime without host capabilities or a
module loader. Source is at most 256 KiB, the JS heap is limited to 32 MiB, and
the JS stack to 256 KiB. Initialization and each frame have a 250 ms interrupt
deadline, including exception-property access and output serialization. The
output transport is capped at 4 MiB; grids are capped at 32,768 cells. These are
runtime budgets, not an OS process memory quota or a hard real-time guarantee.

Any execution/output error poisons the session. The host must retain its last
good frame and create a fresh session before trying again. Invalid input grid
dimensions do not poison an otherwise healthy session. No asynchronous jobs
are driven, no network/filesystem API is exposed, and no external AI service is
used by runtime tests.

## Play compatibility matrix

Grain is inspired by Andreas Gysin's Play, not affiliated with it. This is a
deliberately limited compatibility assessment, not a claim that arbitrary Play
programs work unchanged. The original API is documented in the
[Play manual](https://play.ertdfgcvb.xyz/abc.html) and
[play.core repository](https://github.com/ertdfgcvb/play.core).

| Area | Grain mapping |
| --- | --- |
| `boot/pre/main/post` | Same argument order; synchronous hooks only |
| Coordinates | `x`, `y`, `index`; row-major traversal |
| Time | Seconds rather than Play milliseconds; divide existing factors accordingly |
| Modules | Local module declarations/exports; no external imports |
| Buffer/data | Persistent; grid resize clears cells, not user data |
| Cursor | Unavailable, explicitly `available: false`, coordinates zero, not pressed |
| Metrics | Explicit cell aspect only; no CSS font measurements |
| Colors | RGB bytes or six-digit hex; not general CSS |
| Fonts/attributes | Terminal font; weight/CSS fields rejected |
| Settings | Rejected; host owns layout and playback |
| Browser add-ons | DOM, webcam, canvas, storage, and pointer APIs unavailable |

The regression tests exercise original coordinate, typography, seeded, and
persistent-buffer sketches. No upstream source or examples have been copied.
