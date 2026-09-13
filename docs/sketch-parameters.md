# Sketch parameters

Press `s` in normal mode to edit the active sketch's named numeric parameters.
Enter `speed=1.5` to set a value or `-speed` to remove it, then press Enter.
Escape closes the panel without applying the input.

Parameters are author-defined: adding a value only changes the image when the
sketch reads that value. In p5 use `ctx.params.speed`; in native ASCII use
`context.params.speed`. Provide a fallback for absent values, for example
`const speed = ctx.params.speed ?? 1` in a p5 draw hook.

Names are ASCII identifiers of at most 64 bytes. Reserved prototype names are
rejected. There can be at most 64 values, each finite and between -1,000,000
and 1,000,000. The JavaScript parameter snapshot is read-only.

Before applying a changed parameter map, Grain validates the source with those
values in a fresh bounded runtime. Successful changes create a history version
containing the source, engine, seed and parameters. Restarting Grain or rolling
back restores that version's parameter map. Older history records without a map
load with no parameters. Validation is a sample frame, not a guarantee that every
future frame will succeed.

Changing parameters retains healthy preview session state and requests a new
frame, including while paused. A failed session can be recreated when its
parameters change. Press `R` in normal mode to explicitly reset sketch state at
the current playback position; this does not rewind the audio.

Parameters do not make engines semantically identical. p5 still produces raster
pixels, while native ASCII produces characters and colors directly. Parameter
names and their artistic meaning belong to each sketch, not a universal effect
catalogue.
