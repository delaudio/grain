pub trait SketchGenerator: Send + Sync {
    /// Generate a brand new p5.js audio-reactive sketch from a prompt.
    fn generate(&self, prompt: &str, seed: u64) -> Result<String, String>;

    /// Revise an existing sketch based on a follow-up instruction.
    fn revise(&self, prompt: &str, current_sketch: &str, seed: u64) -> Result<String, String>;
}
