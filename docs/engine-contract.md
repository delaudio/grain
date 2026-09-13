# Creative engine contract, version 1

This contract separates a sketch engine from its AI code provider and its
terminal presentation backend. The initial engine identifiers are `p5` and
`ascii`. The `ascii` identifier currently describes the native-cell contract;
it does not yet enable an ASCII engine in the application. Worker dispatch,
the engine selector, and engine-aware generation are still being integrated.

## Outputs

An adapter returns exactly one `FrameOutput` variant:

- `Raster`: width, height, and straight-alpha RGBA bytes. Each axis is at most
  4096 pixels, the image is at most 4,194,304 pixels, and its byte length must
  equal width times height times four.
- `Cells`: columns, rows, and a rectangular row-major grid of terminal cells.
  The nonempty grid is at most 32,768 cells. Each cell has foreground RGB and
  optional background RGB. Cells are not images and must not be rasterized and
  then sampled back into characters by a presentation backend.

Cell symbols must contain exactly one printable Unicode scalar with narrow
Unicode display width one. Controls, escape sequences, combining characters,
multi-scalar graphemes, and wide characters are rejected. East Asian ambiguous
characters use width one; configure the terminal accordingly. This intentionally
restricted policy makes positioning predictable rather than silently truncating
text or allowing a glyph to overwrite its neighbor.

`EngineFrame` wraps output with engine identity, contract version, source
revision, frame number, and time. The host coordinator is responsible for that
header, not user JavaScript. `FrameOutput::validate` checks output shape and
symbols; deserializing a frame alone is not validation. Adapter sandbox limits
must constrain allocation and evaluation before a frame can reach this check.

## Host adapter boundary

`EngineFactory` is shareable across threads, but the adapter it creates is not
required to be `Send`. Embedded JavaScript sessions belong to the preview worker.
Adapters declare their output kind and whether their state persists across
frames. Render calls receive the host's `GrainContext`, including authoritative
audio, time, frame number, dimensions, and seed, plus terminal grid dimensions.

The coordinator selects reset reasons for source changes, engine changes, seed
changes, seeks, restarts, and canvas resizes. A factory invocation creates a new
session; terminal-only resampling must not reset a persistent raster canvas.
These interfaces describe the boundary; the existing p5 worker has not yet
been replaced by the common factory.

## History compatibility

New version records store `engine`, `contract_version`, and the existing
`runtime_contract` name (`grain-p5-v1` or `grain-ascii-v1`). Missing engine and
version fields in legacy records deserialize as `p5` and `1`. Migration does not
rewrite sketch source, seed, or version references. Unknown engine identifiers
are rejected rather than interpreted as p5.

The legacy `record_new_version` API still records p5. Engine-aware callers use
`record_new_version_for_engine`. Recording rejects malformed existing history
and refuses to overwrite an existing sketch file. Version numbering advances
from the maximum stored version, not the number of entries. Atomic multi-file
history transactions remain a separate task; these protections do not claim
crash-safe or concurrent-writer persistence.

The contract does not promise pixel-identical output across platforms, graphics
backends, or engine implementations. Fixed seed and clock inputs control sketch
inputs, not terminal fonts or external graphics implementations.
