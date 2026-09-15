use crate::runtime::engine::EngineId;

pub const ASCII_CONTRACT: &str = r#"Write a native ASCII creative-coding sketch for Grain, not p5.js.
Implement main(coord, context, cursor, buffer, data). Optional synchronous hooks:
boot(context, buffer, data), pre(context, cursor, buffer, data), post(context, cursor, buffer, data).
coord has x/y/index. context has cols/rows, frame, time IN SECONDS, seed, metrics.aspect
(cell width/height), and audio {amplitude,low,mid,high}. Use host time, not Date.
Return a single printable one-column Unicode scalar or {char, color, backgroundColor}.
Colors are RGB integer arrays [0..255,0..255,0..255] or #rrggbb. No CSS names, alpha,
fontWeight, wide/combining characters, DOM, external imports, async hooks, or settings.
Use module variables/data for persistence; buffer is flat and cleared on grid resize.
Math.random is seeded. Cursor is unavailable. Named ES-module exports are supported.
Make deliberate typography, geometric interference or symbol fields, not raster-to-text.
Output only executable JavaScript, optionally in one javascript code fence."#;

pub trait SketchGenerator: Send + Sync {
    /// Legacy providers get boundary checks; interruptible providers override this.
    fn generate_controlled(
        &self,
        engine: EngineId,
        prompt: &str,
        seed: u64,
        control: &super::control::GenerationControl,
    ) -> Result<String, String> {
        control.check()?;
        let result = self.generate_for_engine(engine, prompt, seed);
        control.check()?;
        result
    }

    fn revise_controlled(
        &self,
        engine: EngineId,
        prompt: &str,
        current: &str,
        seed: u64,
        control: &super::control::GenerationControl,
    ) -> Result<String, String> {
        control.check()?;
        let result = self.revise_for_engine(engine, prompt, current, seed);
        control.check()?;
        result
    }

    /// Generate a brand new p5.js audio-reactive sketch from a prompt.
    fn generate(&self, prompt: &str, seed: u64) -> Result<String, String>;

    /// Revise an existing sketch based on a follow-up instruction.
    fn revise(&self, prompt: &str, current_sketch: &str, seed: u64) -> Result<String, String>;

    fn generate_for_engine(
        &self,
        engine: EngineId,
        prompt: &str,
        seed: u64,
    ) -> Result<String, String> {
        match engine {
            EngineId::P5 => self.generate(prompt, seed),
            EngineId::Ascii => Err("This provider does not support native ASCII generation".into()),
        }
    }

    fn revise_for_engine(
        &self,
        engine: EngineId,
        prompt: &str,
        current: &str,
        seed: u64,
    ) -> Result<String, String> {
        match engine {
            EngineId::P5 => self.revise(prompt, current, seed),
            EngineId::Ascii => Err("This provider does not support native ASCII revision".into()),
        }
    }
}
