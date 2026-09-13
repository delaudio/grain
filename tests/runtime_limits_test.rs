use std::time::{Duration, Instant};

use grain::audio::AudioFeatures;
use grain::runtime::{GrainContext, evaluate_frame};

fn context() -> GrainContext {
    GrainContext {
        width: 800,
        height: 600,
        frame: 0,
        time: 0.0,
        seed: 42,
        audio: AudioFeatures::default(),
    }
}

fn assert_recovers() {
    assert!(
        evaluate_frame(
            "function draw(p) { p.circle(400, 300, 80); }",
            &context(),
            40,
            10
        )
        .is_ok()
    );
}

#[test]
fn host_capabilities_are_not_exposed_even_through_function_constructors() {
    let source = r#"
        function draw(p) {
            const root = p.constructor.constructor('return globalThis')();
            for (const name of ['process', 'require', 'fetch', 'std', 'os', 'Deno', 'Bun', 'console']) {
                if (typeof root[name] !== 'undefined') throw new Error('Host capability exposed: ' + name);
            }
            p.point(20, 20);
        }
    "#;
    evaluate_frame(source, &context(), 40, 10).unwrap();
    for source in [
        "function draw() { process.env.GRAIN_TEST_SENTINEL; }",
        "function draw() { require('fs').readFileSync('synthetic-sentinel'); }",
        "function draw() { fetch('https://example.invalid'); }",
    ] {
        assert!(evaluate_frame(source, &context(), 40, 10).is_err());
    }
}

#[test]
fn nonterminating_sketches_and_exception_getters_are_interrupted() {
    for source in [
        "function draw() { while (true) {} }",
        "while (true) {}",
        "function draw() { try { while(true) {} } catch(e) { while(true) {} } }",
        "function draw() { throw {get message() { while(true) {} }}; }",
    ] {
        let start = Instant::now();
        let error = evaluate_frame(source, &context(), 40, 10).unwrap_err();
        assert!(error.message.contains("time limit"), "{error}");
        assert!(start.elapsed() < Duration::from_secs(3));
        assert_recovers();
    }
}

#[test]
fn nonfinite_geometry_is_rejected_without_waiting_for_a_timeout() {
    for source in [
        "function draw(p) { p.line(0, 0, Infinity, Infinity); }",
        "function draw(p) { p.ellipse(NaN, 10, 20, 20); }",
    ] {
        let error = evaluate_frame(source, &context(), 40, 10).unwrap_err();
        assert!(error.message.contains("Invalid sketch output"), "{error}");
        assert_recovers();
    }
}

#[test]
fn memory_and_stack_exhaustion_return_errors_and_release_the_runtime() {
    for source in [
        "function draw() { globalThis.buffer = new ArrayBuffer(128 * 1024 * 1024); }",
        "function draw() { function recurse() { return recurse() + 1; } recurse(); }",
    ] {
        assert!(evaluate_frame(source, &context(), 40, 10).is_err());
        assert_recovers();
    }
}

#[test]
fn oversized_input_and_invalid_dimensions_fail_before_evaluation() {
    assert!(evaluate_frame(&" ".repeat(256 * 1024 + 1), &context(), 40, 10).is_err());
    for (cols, rows) in [(0, 10), (10, 0), (u16::MAX, u16::MAX)] {
        assert!(evaluate_frame("function draw() {}", &context(), cols, rows).is_err());
    }
    let mut invalid = context();
    invalid.time = f64::NAN;
    assert!(evaluate_frame("function draw() {}", &invalid, 40, 10).is_err());
}

#[test]
fn invalid_source_never_becomes_a_successful_placeholder() {
    for source in [
        "not javascript !!!",
        "",
        "function draw() { throw null; }",
        "async function draw() {}",
    ] {
        assert!(
            evaluate_frame(source, &context(), 40, 10).is_err(),
            "{source}"
        );
    }
    assert_recovers();
}

#[test]
fn oversized_output_is_rejected_and_next_evaluation_is_clean() {
    let source = r#"
        function draw() {
            Object.prototype.toJSON = function() { return 'x'.repeat(5 * 1024 * 1024); };
        }
    "#;
    assert!(evaluate_frame(source, &context(), 40, 10).is_err());
    assert_recovers();
}
