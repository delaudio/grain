use grain::audio::AudioFeatures;
use grain::runtime::GrainContext;
use grain::runtime::ascii::AsciiSession;
use grain::runtime::engine::{EngineAdapter, FrameOutput, OutputKind};
use std::time::{Duration, Instant};

fn context() -> GrainContext {
    GrainContext {
        width: 80,
        height: 40,
        frame: 0,
        time: 1.5,
        seed: 42,
        audio: AudioFeatures {
            amplitude: 0.5,
            low: 0.25,
            mid: 0.75,
            high: 1.0,
        },
    }
}

#[test]
fn native_cells_receive_coordinates_clock_audio_and_metrics() {
    let ctx = context();
    let source = r##"export function main(c, ctx, cursor) {
        if (ctx.time !== 1.5 || ctx.frame !== 0 || ctx.seed !== 42 || ctx.cols !== 2 || ctx.rows !== 2 || ctx.metrics.aspect !== 0.5 || cursor.available !== false) throw Error('context mismatch');
        return { char: String.fromCharCode(65 + c.index), color: [ctx.audio.amplitude * 200, c.x, c.y], backgroundColor: '#102030' };
    }"##;
    let mut session = AsciiSession::new(source, &ctx).unwrap();
    assert_eq!(session.capabilities().output, OutputKind::Cells);
    let FrameOutput::Cells(frame) = session.render(&ctx, 2, 2).unwrap() else {
        panic!("native output was rasterized")
    };
    assert_eq!(frame.cells[1][1].symbol, "D");
    assert_eq!(frame.cells[1][1].r, 100);
    assert_eq!(frame.cells[1][1].g, 1);
    assert_eq!(frame.cells[1][1].b, 1);
    assert_eq!(frame.cells[1][1].background, Some([16, 32, 48]));
}

#[test]
fn hooks_state_and_buffer_persist_but_grid_resize_does_not_reboot() {
    let mut ctx = context();
    let source = r#"
        function boot(ctx, buffer, data) { data.boots = 1; data.count = 0; buffer[0] = 'B'; }
        function pre(ctx, cursor, buffer, data) { data.count++; }
        function main(c, ctx, cursor, buffer, data) { if (c.index) return String(data.boots); }
        function post(ctx, cursor, buffer, data) { buffer[buffer.length - 1] = String(data.count); }
    "#;
    let mut session = AsciiSession::new(source, &ctx).unwrap();
    let first = session.render_cells(&ctx, 3, 1, 0.5).unwrap();
    assert_eq!(first.cells[0][0].symbol, "B");
    assert_eq!(session.render_cells(&ctx, 3, 1, 0.5).unwrap(), first);
    ctx.frame += 1;
    ctx.time += 1.0;
    let second = session.render_cells(&ctx, 3, 1, 0.5).unwrap();
    assert_eq!(second.cells[0][0].symbol, "B");
    assert_eq!(second.cells[0][2].symbol, "2");
    let resized = session.render_cells(&ctx, 4, 1, 0.5).unwrap();
    assert_eq!(resized.cells[0][0].symbol, " ");
    assert_eq!(resized.cells[0][1].symbol, "1");
    assert_eq!(resized.cells[0][3].symbol, "3");
}

#[test]
fn seeded_random_is_repeatable_and_uses_all_seed_digits() {
    let source = "const initial = Math.random(); export function main() { return String.fromCharCode(33 + Math.floor((initial + Math.random()) * 40)); }";
    let mut ctx = context();
    ctx.seed = u64::MAX;
    let first = AsciiSession::new(source, &ctx)
        .unwrap()
        .render_cells(&ctx, 20, 1, 0.5)
        .unwrap();
    assert_eq!(
        AsciiSession::new(source, &ctx)
            .unwrap()
            .render_cells(&ctx, 20, 1, 0.5)
            .unwrap(),
        first
    );
    ctx.seed -= 1;
    assert_ne!(
        AsciiSession::new(source, &ctx)
            .unwrap()
            .render_cells(&ctx, 20, 1, 0.5)
            .unwrap(),
        first
    );
}

#[test]
fn paused_audio_refreshes_and_invalid_viewports_do_not_poison() {
    let mut ctx = context();
    let mut session = AsciiSession::new(
        "function main(c, ctx) { return ctx.audio.low > 0.5 ? 'H' : 'L'; }",
        &ctx,
    )
    .unwrap();
    assert!(session.render_cells(&ctx, 0, 1, 0.5).is_err());
    assert!(session.render_cells(&ctx, 200, 200, 0.5).is_err());
    assert!(session.render_cells(&ctx, 1, 1, f64::NAN).is_err());
    assert_eq!(
        session.render_cells(&ctx, 1, 1, 0.5).unwrap().cells[0][0].symbol,
        "L"
    );
    ctx.audio.low = 1.0;
    assert_eq!(
        session.render_cells(&ctx, 1, 1, 0.5).unwrap().cells[0][0].symbol,
        "H"
    );
    ctx.time -= 1.0;
    assert!(
        session
            .render_cells(&ctx, 1, 1, 0.5)
            .unwrap_err()
            .message
            .contains("new session")
    );
}

#[test]
fn sandbox_has_no_host_capabilities_or_external_modules() {
    let ctx = context();
    let source = r#"function main() {
        const global = Function('return globalThis')();
        for (const name of ['process', 'require', 'fetch', 'document', 'window', 'std', 'os', 'Date']) {
            if (typeof global[name] !== 'undefined') throw Error('leaked ' + name);
        }
        return 'S';
    }"#;
    assert!(
        AsciiSession::new(source, &ctx)
            .unwrap()
            .render_cells(&ctx, 1, 1, 0.5)
            .is_ok()
    );
    assert!(
        AsciiSession::new(
            "import x from 'node:fs'; export function main() { return 'x'; }",
            &ctx
        )
        .is_err()
    );
    assert!(
        AsciiSession::new(
            "await new Promise(() => {}); export function main() { return 'x'; }",
            &ctx
        )
        .is_err()
    );
}

#[test]
fn unsupported_features_and_invalid_cells_are_explicit_errors() {
    let ctx = context();
    assert!(AsciiSession::new("export const settings = { fontFamily: 'serif' }; export function main() { return 'x'; }", &ctx).is_err());
    for source in [
        "async function boot() {} function main() { return 'x'; }",
        "async function main() { return 'x'; }",
        "function main() { return { char: 'x', fontWeight: 700 }; }",
        "function main() { return { char: 'x', color: 'red' }; }",
        "function main() { return { char: 'x', color: [256, 0, 0] }; }",
        "function main() { return '\\u4e00'; }",
        "function main() { return '\\x1b'; }",
        "function main() { return 'e\\u0301'; }",
    ] {
        assert!(
            AsciiSession::new(source, &ctx)
                .unwrap()
                .render_cells(&ctx, 1, 1, 0.5)
                .is_err(),
            "{source}"
        );
    }
}

#[test]
fn timed_out_sessions_are_poisoned_and_fresh_sessions_recover() {
    let ctx = context();
    let start = Instant::now();
    assert!(AsciiSession::new("while (true) {}", &ctx).is_err());
    for source in [
        "function main() { while (true) {} }",
        "function main() { throw { get message() { while (true) {} } }; }",
        "function main() { return 'x'; } function post() { while (true) {} }",
        "function main() { const a = []; while (true) a.push(new Uint8Array(1000000)); }",
    ] {
        let mut session = AsciiSession::new(source, &ctx).unwrap();
        assert!(session.render_cells(&ctx, 1, 1, 0.5).is_err());
        assert!(
            session
                .render_cells(&ctx, 1, 1, 0.5)
                .unwrap_err()
                .message
                .contains("new session")
        );
    }
    assert!(start.elapsed() < Duration::from_secs(5));
    assert!(
        AsciiSession::new("function main() { return 'R'; }", &ctx)
            .unwrap()
            .render_cells(&ctx, 1, 1, 0.5)
            .is_ok()
    );
}
