# Raster preview

The p5-compatible runtime produces drawing commands, which Grain rasterizes
with tiny-skia into a straight-alpha RGBA8 canvas. Terminal characters are no
longer used as intermediate geometry. The terminal fallback area-filters this
canvas into upper-half-block cells with independent foreground and background
RGB. Transparent pixels are composited against black. The canvas is centered
and letterboxed rather than stretched; the default physical cell aspect is 2:1.

Select the fallback explicitly with `GRAIN_PREVIEW_BACKEND=half-block`.
`GRAIN_ANSI_ONLY` remains a compatibility override. The historical Ratty name
is an API alias, not a separate graphics implementation.

Supported geometry includes circles, independent ellipse axes, rectangles,
lines, points, triangles, quads, and vertex paths. Affine translation, rotation,
nonuniform scaling and shear apply in call order. Push/pop save drawing style
as well as the transform. Fill, stroke weight, alpha and backgrounds are
rasterized with antialiasing. Unsupported arcs and blend modes report errors
instead of drawing substitute geometry.

The raster boundary rejects invalid numbers, canvases larger than 4096 on an
axis or 4 megapixels, more than 4096 commands or vertices per path, and excessive
transformed coordinates. Its 250 ms soft deadline is checked between commands;
individual native raster operations are not preempted. JavaScript has a separate
250 ms interrupt budget and bounded heap. Terminal conversion is limited to
32768 cells. These are safety limits, not a claim of guaranteed frame rate.

This first raster implementation still starts a fresh canvas and JavaScript
context per evaluation. Persistent lifecycle, background rendering and iTerm2
image output remain tracked separately; this document does not claim those
features are complete.
