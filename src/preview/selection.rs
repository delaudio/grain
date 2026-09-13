//! Conservative startup policy, without injecting terminal queries into input.
pub fn iterm2_enabled(
    requested: Option<&str>,
    program: Option<&str>,
    version: Option<&str>,
    term: Option<&str>,
    multiplexer: bool,
    tty: bool,
) -> Result<bool, &'static str> {
    let requested = requested.unwrap_or("auto");
    if !matches!(requested, "auto" | "iterm2" | "half-block") {
        return Err("GRAIN_PREVIEW_BACKEND must be auto, iterm2 or half-block");
    }
    if requested == "half-block"
        || !tty
        || multiplexer
        || term.is_some_and(|term| {
            term.starts_with("screen") || term.starts_with("tmux") || term == "dumb"
        })
    {
        return Ok(false);
    }
    if requested == "iterm2" {
        return Ok(true);
    }
    let mut parts = version.unwrap_or("").split('.');
    let major = parts.next().and_then(|v| v.parse::<u32>().ok());
    let minor = parts.next().and_then(|v| v.parse::<u32>().ok());
    Ok(program == Some("iTerm.app")
        && matches!((major, minor), (Some(major), Some(minor)) if major > 3 || (major == 3 && minor >= 2)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auto_requires_a_known_terminal_and_retina_capable_version() {
        assert!(
            iterm2_enabled(
                None,
                Some("iTerm.app"),
                Some("3.5.0"),
                Some("xterm-256color"),
                false,
                true
            )
            .unwrap()
        );
        for (program, version) in [
            (Some("Other"), Some("3.5.0")),
            (Some("iTerm.app"), Some("3.1.9")),
            (Some("iTerm.app"), None),
        ] {
            assert!(!iterm2_enabled(None, program, version, Some("xterm"), false, true).unwrap());
        }
    }
    #[test]
    fn explicit_override_never_bypasses_mux_or_redirected_output() {
        assert!(iterm2_enabled(Some("iterm2"), None, None, Some("xterm"), false, true).unwrap());
        for (term, mux, tty) in [
            ("screen-256color", false, true),
            ("tmux", false, true),
            ("xterm", true, true),
            ("xterm", false, false),
            ("dumb", false, true),
        ] {
            assert!(!iterm2_enabled(Some("iterm2"), None, None, Some(term), mux, tty).unwrap());
        }
        assert!(
            !iterm2_enabled(
                Some("half-block"),
                Some("iTerm.app"),
                Some("3.5.0"),
                None,
                false,
                true
            )
            .unwrap()
        );
        assert!(iterm2_enabled(Some("typo"), None, None, None, false, true).is_err());
    }
}
