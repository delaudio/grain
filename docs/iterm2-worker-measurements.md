# iTerm2 worker baseline

Measured on 2026-09-13, Apple M5, 16 GiB RAM, optimized Rust build.
Installed iTerm2: 3.6.11. This benchmark did **not** send packets to iTerm2.
It does not establish a tested terminal version, visible FPS, visual fidelity,
scrollback behavior or keyboard latency.

Reproduce from the repository root:

```sh
cargo build --offline --release --example iterm2_benchmark
/usr/bin/time -lp target/release/examples/iterm2_benchmark
```

The workload renders 64 colored rectangles and a moving circle to an 800 x 600
p5 canvas, then downsamples and encodes iTerm2 packets. The presentation budget
is 160,000 pixels. It submits 90 frames sequentially, as quickly as possible,
with a 15-second deadline. This is a simple baseline, not a worst-case sketch.

| Measurement | Result |
| --- | --- |
| Worker elapsed time | 0.4700 s |
| Worker throughput | 191.47 frames/s |
| Request-to-completion median | 5.09 ms |
| Request-to-completion p95 | 5.44 ms |
| Mean OSC packet | 9,158 bytes |
| Process user CPU time | 0.44 s |
| Process system CPU time | 0.01 s |
| Maximum resident set size | 19,202,048 bytes |

The initial sandboxed run produced approximately 175.65 frames/s, but the `time`
wrapper could not read a required system counter. The table records the completed
host-approved run instead. CPU time describes this saturated benchmark process,
not Grain plus iTerm2 during 30 FPS playback. Terminal presentation and real input
latency remain separate acceptance measurements under #26.
