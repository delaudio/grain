# iTerm2 preview integration

Status: implementation in progress under #26. Real-terminal visual and sustained
playback acceptance is not yet complete. No minimum **tested** iTerm2 version or
measured 30 FPS claim is established by the automated tests.

`GRAIN_PREVIEW_BACKEND` selects `auto` (default), `iterm2`, or `half-block`.
For example, run `GRAIN_PREVIEW_BACKEND=half-block grain` to force portable cells.
Auto enables images only for a TTY reporting `TERM_PROGRAM=iTerm.app` and a
parseable `TERM_PROGRAM_VERSION` of at least 3.2. This is a capability heuristic,
not a claim that every such version has been tested. The explicit iTerm2 override
bypasses that heuristic, but neither mode enables images for redirected output,
tmux, screen, or a dumb terminal. Native ASCII always remains native cells.

The protocol uses cell-sized bounds and preserves image aspect ratio. iTerm2
documents corrected Retina behavior starting with 3.2.0 in its
[inline image protocol reference](https://iterm2.com/documentation-images.html).

Image resampling, PNG encoding and OSC packet assembly run on the preview worker.
Presentation images are limited to 160,000 pixels, PNG payloads to 750,000 bytes,
and complete image packets to 1 MiB. The sketch canvas remains unchanged. RGBA is
composited onto black before area-average downsampling; transparent pixels cannot
reveal a previous frame. One pending request and one result replace stale work.

The terminal-owning loop serializes image writes with Ratatui draws. It explicitly
replaces the previous image region before presenting another frame, and clears
the terminal/buffer when leaving image presentation or changing its rectangle.
Modals and prompt editing use cell preview. Entering the external editor disables
image presentation before leaving the alternate screen; returning requests a new
frame even while paused. Encoding failures fall back to cells.

Remaining acceptance work includes real iTerm2 verification for trails, overlays,
resize, Retina geometry, editor transitions and scrollback; measured resolution,
CPU, frame rate and input latency; and adaptive performance under load. Terminal
writes are currently synchronous, so bounded packet size alone does not prove
responsive input on a slow terminal or transport.

## Adaptive presentation

Image requests are capped at 30 FPS independently of the audio/sketch clock.
The load controller observes worker preparation and terminal transmission time,
using a moving average. Under load it steps through presentation budgets of
160,000, 80,000, 40,000, 20,000 and 10,000 pixels, with nominal rates of 30, 24,
15, 10 and 5 FPS. Observed costs can extend the request interval up to one second.
Quality recovers one step only after 60 sufficiently inexpensive observations.

Changing the presentation budget invalidates obsolete queued image results,
without resizing or restarting the engine canvas. Current images can remain
visible until replacements arrive. This feedback reduces sustained pressure;
it cannot preempt an individual blocking terminal write and is not a substitute
for the real-terminal latency measurements above.
