use grain::audio::AudioFeatures;
use grain::runtime::GrainContext;
use grain::runtime::builtin::BuiltinEngineFactory;
use grain::runtime::engine::{EngineFactory, EngineId, FrameOutput, ResetReason};
use grain::runtime::parameters::{MAX_PARAMETERS, Parameters};

#[test]
fn validated_parameters_reject_invalid_names_values_and_excess_entries() {
    let mut params = Parameters::default();
    for name in [
        "",
        "1speed",
        "a b",
        "x.y",
        "__proto__",
        "constructor",
        "prototype",
        "\u{e9}",
    ] {
        assert!(params.set(name, 1.0).is_err(), "{name}");
    }
    assert!(params.set(&"x".repeat(65), 1.0).is_err());
    for value in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        1_000_001.0,
        -1_000_001.0,
    ] {
        assert!(params.set("speed", value).is_err());
    }
    for index in 0..MAX_PARAMETERS {
        params.set(&format!("p{index}"), index as f64).unwrap();
    }
    assert!(params.set("overflow", 0.0).is_err());
    params.set("p0", -1_000_000.0).unwrap();
    assert_eq!(params.get("p0"), Some(-1_000_000.0));
    params.remove("p0");
    params.set("replacement", 1_000_000.0).unwrap();
    assert_eq!(params.iter().count(), MAX_PARAMETERS);
}

#[test]
fn serialization_is_stable_and_deserialization_enforces_the_same_contract() {
    let mut params = Parameters::default();
    params.set("z", 2.0).unwrap();
    params.set("a", 1.0).unwrap();
    let json = serde_json::to_string(&params).unwrap();
    assert_eq!(json, r#"{"a":1.0,"z":2.0}"#);
    assert_eq!(serde_json::from_str::<Parameters>(&json).unwrap(), params);
    for invalid in [
        r#"{"a":1,"a":2}"#,
        r#"{"a":null}"#,
        r#"{"a":1000001}"#,
        r#"{"__proto__":1}"#,
        r#"{"a":"1"}"#,
    ] {
        assert!(
            serde_json::from_str::<Parameters>(invalid).is_err(),
            "{invalid}"
        );
    }
    let oversized = format!(
        "{{{}}}",
        (0..65)
            .map(|n| format!("\"p{n}\":1"))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert!(serde_json::from_str::<Parameters>(&oversized).is_err());
    let context: GrainContext = serde_json::from_str(r#"{"width":4,"height":4,"frame":0,"time":0,"seed":42,"audio":{"amplitude":0,"low":0,"mid":0,"high":0}}"#).unwrap();
    assert_eq!(context.params, Parameters::default());
}

#[test]
fn both_engines_refresh_changed_parameters_while_paused_without_resetting_state() {
    for engine in [EngineId::P5, EngineId::Ascii] {
        let mut context = GrainContext {
            params: Parameters::default(),
            width: 4,
            height: 4,
            frame: 0,
            time: 0.0,
            seed: 42,
            audio: AudioFeatures::default(),
        };
        context.params.set("level", 10.0).unwrap();
        let source = match engine {
            EngineId::P5 => {
                "let count = 0; function setup(p, ctx) { if (ctx.params.level !== 10) throw Error('setup params'); } function draw(p, ctx) { if (!Object.isFrozen(ctx.params)) throw Error('mutable params'); count++; p.background(ctx.params.level, count, 0); }"
            }
            EngineId::Ascii => {
                "let count = 0; export function boot(ctx) { if (ctx.params.level !== 10) throw Error('boot params'); } export function pre() { count++; } export function main(c, ctx) { if (!Object.isFrozen(ctx.params)) throw Error('mutable params'); return {char: 'X', color: [ctx.params.level, count, 0]}; }"
            }
        };
        let mut adapter = BuiltinEngineFactory
            .create(engine, source, &context, ResetReason::SourceChanged)
            .unwrap();
        let first = adapter.render(&context, 1, 1).unwrap();
        if let FrameOutput::Raster(expected) = &first {
            let isolated = grain::runtime::evaluate_frame(source, &context, 1, 1).unwrap();
            assert_eq!(isolated.raster.as_ref(), Some(expected));
        }
        assert_eq!(adapter.render(&context, 1, 1).unwrap(), first);
        context.params.set("level", 20.0).unwrap();
        let changed = adapter.render(&context, 1, 1).unwrap();
        match changed {
            FrameOutput::Raster(raster) => assert_eq!(&raster.rgba[..4], &[20, 2, 0, 255]),
            FrameOutput::Cells(grid) => {
                assert_eq!(grid.cells[0][0].r, 20);
                assert_eq!(grid.cells[0][0].g, 2);
            }
        }
    }
}
