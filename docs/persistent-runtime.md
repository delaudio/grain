# Persistent preview lifecycle

The terminal preview uses one background worker. The worker owns a QuickJS
context and the RGBA canvas; `setup(p, ctx)` runs once when a source/seed/canvas
configuration is activated. `draw(p, ctx)` runs for each requested position.
JavaScript variables, seeded PRNG state, drawing styles and canvas pixels persist.
Transforms reset before each draw. Both lifecycle callbacks must be synchronous.
Generation validation still uses an isolated one-shot evaluator.

The mailbox has one pending request and one completed result. New requests
replace queued work instead of creating a backlog. Completed frames from old
source/viewport/seek revisions are rejected. Within the current revision, the
newest completed frame remains visible while a later one is being rendered;
otherwise a slow renderer would never display anything. Native drawing and
cell conversion do not run inside the input or ratatui paint handlers.

A replacement session is provisional until its first frame succeeds. On error,
the previous visible frame is retained and the diagnostic is shown in the status
bar. A session that fails mid-frame is not reused, because its user state may
have been partially mutated. Failed source/seed/canvas identities are not retried
on every tick; edit the source or change that configuration to retry.

Forward jumps skip intermediate draws. Backward seeks and loop restarts create
a fresh session at the requested position, rather than replaying every skipped
frame. This bounds latency but means stateful sketches are not seek-invariant.
A paused resize resamples the existing canvas without advancing sketch state.
Each result carries the coordinator's source/viewport/seek revision and the
authoritative requested frame number; sketch code cannot supply these values.

The visual clock samples rodio's playback position when an audio sink is loaded.
Without a sink, a monotonic clock tracks elapsed playback time. Pause/resume do
not accrue paused time. UI ticks are scheduled independently of keyboard events;
missed ticks are skipped. Position quantization is less than one configured frame
(16.7 ms at 60 fps), excluding device latency and rendering/presentation latency.
This is not a claim that a slow sketch displays within that tolerance: the last
completed frame may lag playback while a newer request is processed.

The worker is joined on shutdown after the bounded current operation completes.
JavaScript retains its 32 MiB heap, 256 KiB stack and 250 ms interrupt budget per
initialization/frame. Native raster operations have a separate soft budget as
described in `raster-preview.md`, not a preemptive hard real-time guarantee.
No Node process, visible browser or external graphics service is required.
