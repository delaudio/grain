// Preserve the public API while keeping native WASAPI objects on their owner.
#[cfg(not(windows))]
pub use super::backend::AudioPlayer;
#[cfg(windows)]
pub use super::windows_player::AudioPlayer;
