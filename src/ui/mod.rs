use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, Paragraph, Wrap,
    },
    Frame,
};

use crate::state::{AudioStatus, GenerationStatus, GrainState, InputMode, PreviewStatus};

pub fn render(frame: &mut Frame, state: &GrainState) {
    let area = frame.area();

    // Base layout: Header (3), Main Preview & Sidebar (Min 8), Prompt Bar (5), Footer (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(4), // Audio status panel
            Constraint::Min(8),    // Visual Preview area
            Constraint::Length(4), // Prompt bar
            Constraint::Length(1), // Help / Shortcut footer
        ])
        .split(area);

    render_header(frame, chunks[0], state);
    render_audio_panel(frame, chunks[1], state);
    render_preview_area(frame, chunks[2], state);
    render_prompt_panel(frame, chunks[3], state);
    render_footer(frame, chunks[4], state);

    // Render modals/overlays if needed
    match state.mode {
        InputMode::Help => render_help_modal(frame, area),
        InputMode::Versions => render_versions_modal(frame, area, state),
        InputMode::OpeningAudio => render_open_audio_modal(frame, area, state),
        _ => {}
    }
}

fn render_header(frame: &mut Frame, area: Rect, state: &GrainState) {
    let mode_str = match state.mode {
        InputMode::Normal => "NORMAL",
        InputMode::EditingPrompt => "EDITING PROMPT",
        InputMode::OpeningAudio => "OPEN AUDIO",
        InputMode::Help => "HELP",
        InputMode::Versions => "VERSIONS",
    };

    let mode_color = match state.mode {
        InputMode::Normal => Color::Cyan,
        InputMode::EditingPrompt => Color::Yellow,
        InputMode::OpeningAudio => Color::Magenta,
        InputMode::Help => Color::Green,
        InputMode::Versions => Color::Blue,
    };

    let header_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(16),
            Constraint::Min(20),
            Constraint::Length(22),
        ])
        .split(area);

    let title_block = Paragraph::new(Line::from(vec![
        Span::styled(" GRAIN ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled("v0.1.0", Style::default().fg(Color::DarkGray)),
    ]))
    .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));

    let status_text = state.status_message.as_deref().unwrap_or("Ready");
    let status_block = Paragraph::new(Line::from(vec![
        Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
        Span::styled(status_text, Style::default().fg(Color::White)),
    ]))
    .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));

    let mode_badge = Paragraph::new(Line::from(vec![
        Span::styled(format!(" [{}] ", mode_str), Style::default().fg(mode_color).add_modifier(Modifier::BOLD)),
    ]))
    .alignment(Alignment::Right)
    .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));

    frame.render_widget(title_block, header_layout[0]);
    frame.render_widget(status_block, header_layout[1]);
    frame.render_widget(mode_badge, header_layout[2]);
}

fn render_audio_panel(frame: &mut Frame, area: Rect, state: &GrainState) {
    let audio_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Audio & Analysis ");

    let audio_path_display = match &state.audio.path {
        Some(p) => p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| p.display().to_string()),
        None => "[No audio loaded - press 'o' to open]".to_string(),
    };

    let status_indicator = match &state.audio.status {
        AudioStatus::None => Span::styled("● IDLE", Style::default().fg(Color::DarkGray)),
        AudioStatus::Loading => Span::styled("● ANALYZING...", Style::default().fg(Color::Yellow)),
        AudioStatus::Ready => Span::styled("● READY", Style::default().fg(Color::Green)),
        AudioStatus::Error(e) => Span::styled(format!("● ERROR: {}", e), Style::default().fg(Color::Red)),
    };

    let duration_sec = state.audio.duration_ms as f64 / 1000.0;
    let line1 = Line::from(vec![
        Span::styled(" Track: ", Style::default().fg(Color::DarkGray)),
        Span::styled(audio_path_display, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::raw("   "),
        Span::styled("Duration: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:.1}s", duration_sec), Style::default().fg(Color::White)),
        Span::raw("   "),
        Span::styled("Format: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{}Hz / {}ch", state.audio.sample_rate, state.audio.channels), Style::default().fg(Color::White)),
        Span::raw("   "),
        status_indicator,
    ]);

    let playback_indicator = if state.preview.is_playing {
        Span::styled("▶ PLAYING", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("⏸ PAUSED", Style::default().fg(Color::DarkGray))
    };

    let current_sec = (state.preview.current_frame as f64 / state.preview.fps.max(1) as f64).min(duration_sec);
    let line2 = Line::from(vec![
        Span::styled(" Playback: ", Style::default().fg(Color::DarkGray)),
        playback_indicator,
        Span::raw("   "),
        Span::styled("Position: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!(
                "{:02}:{:02}.{:02} / {:02}:{:02}.{:02}",
                (current_sec / 60.0) as u32,
                (current_sec % 60.0) as u32,
                ((current_sec * 100.0) % 100.0) as u32,
                (duration_sec / 60.0) as u32,
                (duration_sec % 60.0) as u32,
                ((duration_sec * 100.0) % 100.0) as u32,
            ),
            Style::default().fg(Color::White),
        ),
        Span::raw("   "),
        Span::styled("Frame: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{}/{}", state.preview.current_frame, state.preview.total_frames), Style::default().fg(Color::White)),
    ]);

    let paragraph = Paragraph::new(vec![line1, line2]).block(audio_block);
    frame.render_widget(paragraph, area);
}

fn render_preview_area(frame: &mut Frame, area: Rect, state: &GrainState) {
    let preview_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Magenta))
        .title(format!(
            " Visual Preview (p5.js runtime) — {} [Seed: {}] ",
            state.preview.sketch_name, state.preview.seed
        ));

    let inner = preview_block.inner(area);
    frame.render_widget(preview_block, area);

    if inner.height < 4 || inner.width < 10 {
        return;
    }

    // Centered placeholder visualization
    let mut lines = Vec::new();

    let status_str = match &state.preview.status {
        PreviewStatus::Placeholder => "Placeholder preview active",
        PreviewStatus::Ready => "Runtime ready",
        PreviewStatus::Rendering => "Rendering frames...",
        PreviewStatus::Error(e) => e.as_str(),
    };

    lines.push(Line::from(vec![
        Span::styled("┌────────────────────────────────────────────────────────┐", Style::default().fg(Color::DarkGray)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("│  ", Style::default().fg(Color::DarkGray)),
        Span::styled("p5.js Creative Runtime Bridge", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled("                              │", Style::default().fg(Color::DarkGray)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("│  ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("Target Canvas: {}x{} @ {}fps", state.preview.width, state.preview.height, state.preview.fps), Style::default().fg(Color::Cyan)),
        Span::styled("                     │", Style::default().fg(Color::DarkGray)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("│  ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("Status: {}", status_str), Style::default().fg(Color::Green)),
        Span::styled("                                 │", Style::default().fg(Color::DarkGray)),
    ]));

    // Visual reaction simulation bar
    let phase = (state.preview.current_frame as f64 * 0.1).sin();
    let amp_bars = ((((phase + 1.0) / 2.0) * 20.0).round() as usize).min(20);
    let bar_repr = "█".repeat(amp_bars) + &"░".repeat(20 - amp_bars);

    lines.push(Line::from(vec![
        Span::styled("│  Audio Reactivity: [", Style::default().fg(Color::DarkGray)),
        Span::styled(bar_repr, Style::default().fg(Color::Magenta)),
        Span::styled("]         │", Style::default().fg(Color::DarkGray)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("└────────────────────────────────────────────────────────┘", Style::default().fg(Color::DarkGray)),
    ]));

    let placeholder_p = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .block(Block::default());

    let center_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(6),
            Constraint::Fill(1),
        ])
        .split(inner);

    frame.render_widget(placeholder_p, center_layout[1]);
}

fn render_prompt_panel(frame: &mut Frame, area: Rect, state: &GrainState) {
    let is_editing = state.mode == InputMode::EditingPrompt;

    let border_color = if is_editing {
        Color::Yellow
    } else {
        Color::White
    };

    let title = if is_editing {
        " Prompt Editor (Press Enter to submit, Esc to cancel) "
    } else {
        " Generation Prompt (Press 'p' to edit, 'g' to generate) "
    };

    let prompt_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(title);

    let content = if is_editing {
        state.prompt.input_buffer.as_str()
    } else {
        state.prompt.active_prompt.as_str()
    };

    let gen_status_span = match &state.prompt.generation_status {
        GenerationStatus::Idle => Span::styled(" [Ready] ", Style::default().fg(Color::DarkGray)),
        GenerationStatus::Generating => Span::styled(" [Generating...] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        GenerationStatus::Ready => Span::styled(format!(" [v{}] ", state.prompt.current_version), Style::default().fg(Color::Green)),
        GenerationStatus::Failed(e) => Span::styled(format!(" [Error: {}] ", e), Style::default().fg(Color::Red)),
    };

    let prompt_line = Line::from(vec![
        Span::styled("Prompt: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(content, Style::default().fg(Color::White)),
        gen_status_span,
    ]);

    let p = Paragraph::new(vec![prompt_line])
        .block(prompt_block)
        .wrap(Wrap { trim: false });

    frame.render_widget(p, area);

    // Show cursor when editing
    if is_editing {
        let cursor_x = area.x + 9 + (state.prompt.cursor_position as u16);
        let cursor_y = area.y + 1;
        if cursor_x < area.x + area.width - 1 && cursor_y < area.y + area.height - 1 {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }
}

fn render_footer(frame: &mut Frame, area: Rect, state: &GrainState) {
    let shortcuts = match state.mode {
        InputMode::Normal => vec![
            Span::styled(" o", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Open  "),
            Span::styled("p", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Prompt  "),
            Span::styled("g", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Generate  "),
            Span::styled("Space", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Play/Pause  "),
            Span::styled("v", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Versions  "),
            Span::styled("?", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Help  "),
            Span::styled("q", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Quit"),
        ],
        InputMode::EditingPrompt => vec![
            Span::styled("Enter", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Commit & Generate  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Cancel  "),
            Span::styled("←/→", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Move Cursor"),
        ],
        InputMode::OpeningAudio => vec![
            Span::styled("Enter", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Load Audio File  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Cancel"),
        ],
        InputMode::Help | InputMode::Versions => vec![
            Span::styled("Esc / q", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Close Overlay"),
        ],
    };

    let footer_p = Paragraph::new(Line::from(shortcuts))
        .alignment(Alignment::Center)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));

    frame.render_widget(footer_p, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn render_help_modal(frame: &mut Frame, area: Rect) {
    let popup_area = centered_rect(60, 60, area);
    frame.render_widget(Clear, popup_area);

    let help_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Green))
        .title(" Help & Keyboard Controls ");

    let text = vec![
        Line::from(Span::styled("Grain — Audio-Reactive Creative Coding Instrument", Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  o         ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("Open an audio file (WAV / MP3)"),
        ]),
        Line::from(vec![
            Span::styled("  p         ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("Edit natural language prompt"),
        ]),
        Line::from(vec![
            Span::styled("  g         ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("Generate or regenerate sketch from active prompt"),
        ]),
        Line::from(vec![
            Span::styled("  Space     ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("Play / Pause audio & preview playback"),
        ]),
        Line::from(vec![
            Span::styled("  v         ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("View sketch version history and rollback"),
        ]),
        Line::from(vec![
            Span::styled("  ?         ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("Toggle this help popup"),
        ]),
        Line::from(vec![
            Span::styled("  q         ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("Quit Grain"),
        ]),
        Line::from(""),
        Line::from(Span::styled("Press 'Esc' or '?' to close this dialog", Style::default().fg(Color::DarkGray))),
    ];

    let p = Paragraph::new(text)
        .block(help_block)
        .alignment(Alignment::Left);

    frame.render_widget(p, popup_area);
}

fn render_versions_modal(frame: &mut Frame, area: Rect, state: &GrainState) {
    let popup_area = centered_rect(60, 50, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Blue))
        .title(" Sketch Version History ");

    let mut text = vec![
        Line::from(Span::styled("Saved Versions", Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan))),
        Line::from(""),
    ];

    if state.prompt.total_versions == 0 {
        text.push(Line::from(Span::styled("  No generated versions yet. Press 'g' to generate a sketch.", Style::default().fg(Color::DarkGray))));
    } else {
        for v in 1..=state.prompt.total_versions {
            let marker = if v == state.prompt.current_version { "► " } else { "  " };
            let style = if v == state.prompt.current_version {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            text.push(Line::from(vec![
                Span::styled(format!("{}{:03} — sketch_v{} (Prompt: \"{}\")", marker, v, v, state.prompt.active_prompt), style),
            ]));
        }
    }

    text.push(Line::from(""));
    text.push(Line::from(Span::styled("Press 'Esc' or 'v' to close", Style::default().fg(Color::DarkGray))));

    let p = Paragraph::new(text).block(block);
    frame.render_widget(p, popup_area);
}

fn render_open_audio_modal(frame: &mut Frame, area: Rect, state: &GrainState) {
    let popup_area = centered_rect(60, 30, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Magenta))
        .title(" Open Audio File ");

    let text = vec![
        Line::from(Span::styled("Enter path to WAV or MP3 audio file:", Style::default().fg(Color::White))),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::styled(&state.audio_input_buffer, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("█", Style::default().fg(Color::Magenta)),
        ]),
        Line::from(""),
        Line::from(Span::styled("Press 'Enter' to load or 'Esc' to cancel", Style::default().fg(Color::DarkGray))),
    ];

    let p = Paragraph::new(text).block(block);
    frame.render_widget(p, popup_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn test_ui_renders_without_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = GrainState::default();

        terminal.draw(|f| render(f, &state)).unwrap();
    }

    #[test]
    fn test_ui_renders_small_terminal_without_panic() {
        let backend = TestBackend::new(40, 15);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = GrainState::default();

        terminal.draw(|f| render(f, &state)).unwrap();
    }

    #[test]
    fn test_ui_renders_help_modal() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = GrainState::default();
        state.mode = InputMode::Help;

        terminal.draw(|f| render(f, &state)).unwrap();
    }
}
