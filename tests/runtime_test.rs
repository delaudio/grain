use grain::audio::AudioFeatures;
use grain::runtime::{DEFAULT_SKETCH_TEMPLATE, GrainContext, evaluate_frame};

#[test]
fn test_runtime_renders_fixture_sketch() {
    let source = include_str!("../fixtures/example_sketch.js");
    let ctx = GrainContext {
        width: 800,
        height: 600,
        frame: 0,
        time: 0.0,
        seed: 42,
        audio: AudioFeatures {
            amplitude: 0.8,
            low: 0.7,
            mid: 0.5,
            high: 0.3,
        },
    };

    let result = evaluate_frame(source, &ctx, 40, 10).expect("evaluation failed");
    assert_eq!(result.frame, 0);
    assert_eq!(result.width, 800);
    assert_eq!(result.height, 600);
    assert!(result.ascii_art.is_some());
    assert!(result.draw_commands_count > 0);
}

#[test]
fn test_runtime_renders_default_template() {
    let ctx = GrainContext {
        width: 800,
        height: 600,
        frame: 10,
        time: 0.166,
        seed: 12345,
        audio: AudioFeatures {
            amplitude: 0.5,
            low: 0.9,
            mid: 0.4,
            high: 0.8,
        },
    };

    let result = evaluate_frame(DEFAULT_SKETCH_TEMPLATE, &ctx, 50, 12).expect("evaluation failed");
    assert_eq!(result.frame, 10);
    assert!(!result.ascii_art.unwrap().is_empty());
}

#[test]
fn test_runtime_captures_syntax_and_runtime_errors() {
    let broken_source = "function draw(p, ctx) { p.invalidMethodName(); }";
    let ctx = GrainContext {
        width: 800,
        height: 600,
        frame: 0,
        time: 0.0,
        seed: 42,
        audio: AudioFeatures::default(),
    };

    let err = evaluate_frame(broken_source, &ctx, 40, 10).unwrap_err();
    assert!(err.message.contains("not a function"), "{err}");
    assert!(
        err.stack.is_some(),
        "Runtime errors should retain their stack"
    );
}

#[test]
fn test_runtime_determinism() {
    let source = include_str!("../examples/wave_particles.js");
    let ctx = GrainContext {
        width: 800,
        height: 600,
        frame: 45,
        time: 0.75,
        seed: 999,
        audio: AudioFeatures {
            amplitude: 0.65,
            low: 0.82,
            mid: 0.45,
            high: 0.30,
        },
    };

    let res1 = evaluate_frame(source, &ctx, 40, 10).unwrap();
    let res2 = evaluate_frame(source, &ctx, 40, 10).unwrap();

    assert_eq!(res1, res2);
}

#[test]
fn test_runtime_reports_sketch_source_location() {
    let source =
        "function draw(p, ctx) {\n  const value = 1;\n  throw new Error('location probe');\n}";
    let ctx = GrainContext {
        width: 800,
        height: 600,
        frame: 0,
        time: 0.0,
        seed: 42,
        audio: AudioFeatures::default(),
    };
    let error = evaluate_frame(source, &ctx, 40, 10).unwrap_err();
    assert_eq!(error.line, Some(3), "{error:?}");
    assert_eq!(error.column, Some(13), "{error:?}");

    let first_line = evaluate_frame("throw new Error('first line');", &ctx, 40, 10).unwrap_err();
    assert_eq!(first_line.line, Some(1), "{first_line:?}");
    assert_eq!(first_line.column, Some(11), "{first_line:?}");

    let missing_draw = evaluate_frame("const value = 1;", &ctx, 40, 10).unwrap_err();
    assert_eq!(missing_draw.line, None, "{missing_draw:?}");
    assert_eq!(missing_draw.column, None, "{missing_draw:?}");
}
