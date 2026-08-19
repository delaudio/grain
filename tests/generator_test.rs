use std::sync::Arc;
use grain::generator::{GenerationService, MockGenerator, SketchGenerator};

#[test]
fn test_mock_generator_generates_valid_sketch() {
    let service = GenerationService::new(Arc::new(MockGenerator::new()));
    let code = service
        .generate_and_validate("geometric audio tunnel", 42)
        .expect("Generation should succeed");

    assert!(code.contains("Geometric Audio Tunnel"));
    assert!(code.contains("setup(p)"));
    assert!(code.contains("draw(p, ctx)"));
}

#[test]
fn test_mock_generator_revises_sketch() {
    let service = GenerationService::new(Arc::new(MockGenerator::new()));
    let initial = service
        .generate_and_validate("orbital particle field", 100)
        .unwrap();

    let revised = service
        .revise_and_validate("make it red with high speed", &initial, 101)
        .expect("Revision should succeed");

    assert!(revised.contains("Red Accent Shift") || revised.contains("High Speed"));
}

struct BrokenGenerator;
impl SketchGenerator for BrokenGenerator {
    fn generate(&self, _prompt: &str, _seed: u64) -> Result<String, String> {
        Ok("function draw(p, ctx) { throw new Error('Boom'); }".to_string())
    }
    fn revise(&self, _prompt: &str, _current: &str, _seed: u64) -> Result<String, String> {
        Ok("function draw(p, ctx) { throw new Error('Boom'); }".to_string())
    }
}

#[test]
fn test_validation_rejects_broken_generated_sketch() {
    let service = GenerationService::new(Arc::new(BrokenGenerator));
    let result = service.generate_and_validate("some prompt", 42);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("runtime validation"));
}
