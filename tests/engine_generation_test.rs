use grain::generator::{GenerationService, SketchGenerator};
use grain::runtime::engine::EngineId;
use grain::runtime::template::{DEFAULT_ASCII_SKETCH_TEMPLATE, DEFAULT_SKETCH_TEMPLATE};
use std::sync::{Arc, Mutex};

struct TrackingProvider {
    engines: Arc<Mutex<Vec<EngineId>>>,
    wrong_contract: bool,
}

impl SketchGenerator for TrackingProvider {
    fn generate(&self, _: &str, _: u64) -> Result<String, String> {
        Err("legacy entry point".into())
    }
    fn revise(&self, _: &str, _: &str, _: u64) -> Result<String, String> {
        Err("legacy entry point".into())
    }
    fn generate_for_engine(&self, engine: EngineId, _: &str, _: u64) -> Result<String, String> {
        self.engines.lock().unwrap().push(engine);
        Ok(if engine == EngineId::Ascii && !self.wrong_contract {
            DEFAULT_ASCII_SKETCH_TEMPLATE
        } else {
            DEFAULT_SKETCH_TEMPLATE
        }
        .into())
    }
    fn revise_for_engine(
        &self,
        engine: EngineId,
        prompt: &str,
        _: &str,
        seed: u64,
    ) -> Result<String, String> {
        self.generate_for_engine(engine, prompt, seed)
    }
}

#[test]
fn generation_and_revision_dispatch_and_validate_the_selected_engine() {
    let engines = Arc::new(Mutex::new(Vec::new()));
    let service = GenerationService::new(Arc::new(TrackingProvider {
        engines: engines.clone(),
        wrong_contract: false,
    }));
    let code = service
        .generate_for_engine(EngineId::Ascii, "glyphs", 42)
        .unwrap();
    assert!(code.contains("function main"));
    service
        .revise_for_engine(EngineId::Ascii, "change", &code, 43)
        .unwrap();
    service
        .generate_for_engine(EngineId::P5, "shapes", 42)
        .unwrap();
    assert_eq!(
        *engines.lock().unwrap(),
        [EngineId::Ascii, EngineId::Ascii, EngineId::P5]
    );
}

#[test]
fn p5_output_is_rejected_when_ascii_was_requested() {
    let service = GenerationService::new(Arc::new(TrackingProvider {
        engines: Arc::new(Mutex::new(Vec::new())),
        wrong_contract: true,
    }));
    assert!(
        service
            .generate_for_engine(EngineId::Ascii, "glyphs", 42)
            .unwrap_err()
            .contains("validation")
    );
}
