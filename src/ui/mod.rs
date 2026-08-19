use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, Paragraph, Wrap,
    },
    Frame,
};

use crate::state::{AudioStatus, GenerationStatus, GrainState, InputMode};

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
        InputMode::SelectModel => render_model_selector_modal(frame, area, state),
        InputMode::Tuning => render_tuning_modal(frame, area, state),
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
        InputMode::SelectModel => "AI ENGINE SELECTOR",
        InputMode::Tuning => "AUDIO DSP TUNING",
    };

    let mode_color = match state.mode {
        InputMode::Normal => Color::Cyan,
        InputMode::EditingPrompt => Color::Yellow,
        InputMode::OpeningAudio => Color::Magenta,
        InputMode::Help => Color::Green,
        InputMode::Versions => Color::Blue,
        InputMode::SelectModel => Color::LightCyan,
        InputMode::Tuning => Color::Rgb(255, 180, 0),
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
    let engine = crate::preview::PreviewEngine::new();

    let preview_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Magenta))
        .title(format!(
            " Visual Preview ({}) — {} [Seed: {}] ",
            engine.backend_name(),
            state.preview.sketch_name,
            state.preview.seed
        ));

    let inner = preview_block.inner(area);
    frame.render_widget(preview_block, area);

    if inner.height < 4 || inner.width < 10 {
        return;
    }

    let features = if state.audio.analysis.is_some() {
        state.live_audio_features
    } else {
        let phase = (((state.preview.current_frame as f64 * 0.1).sin() + 1.0) / 2.0) as f32;
        crate::audio::AudioFeatures {
            amplitude: phase,
            low: phase * 0.8,
            mid: phase * 0.6,
            high: phase * 0.4,
        }
    };

    // Split inner area into visual canvas (flexible) and reactivity meter (1 line at bottom)
    let preview_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(inner);

    match engine.render_frame(
        &state.preview.sketch_source,
        state.preview.current_frame,
        state.preview.fps,
        state.preview.seed,
        features,
        preview_layout[0],
    ) {
        Ok((_res, visual_lines)) => {
            let visual_p = Paragraph::new(visual_lines)
                .alignment(Alignment::Center)
                .block(Block::default());
            frame.render_widget(visual_p, preview_layout[0]);
        }
        Err(err) => {
            let err_lines = vec![
                Line::from(Span::styled("⚠️ Visual Runtime Diagnostic:", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(format!("{}", err), Style::default().fg(Color::LightRed))),
            ];
            let err_p = Paragraph::new(err_lines)
                .alignment(Alignment::Center)
                .block(Block::default());
            frame.render_widget(err_p, preview_layout[0]);
        }
    }

    // Reactivity bottom bar
    let make_bar = |val: f32, len: usize| -> String {
        let filled = ((val.clamp(0.0, 1.0) * len as f32).round() as usize).min(len);
        "█".repeat(filled) + &"░".repeat(len - filled)
    };

    let meter_line = Line::from(vec![
        Span::styled("Reactivity: ", Style::default().fg(Color::DarkGray)),
        Span::styled("AMP ", Style::default().fg(Color::White)),
        Span::styled(format!("[{}] ", make_bar(features.amplitude, 6)), Style::default().fg(Color::Magenta)),
        Span::styled("LOW ", Style::default().fg(Color::Cyan)),
        Span::styled(format!("[{}] ", make_bar(features.low, 6)), Style::default().fg(Color::Cyan)),
        Span::styled("MID ", Style::default().fg(Color::Yellow)),
        Span::styled(format!("[{}] ", make_bar(features.mid, 6)), Style::default().fg(Color::Yellow)),
        Span::styled("HI ", Style::default().fg(Color::Green)),
        Span::styled(format!("[{}]", make_bar(features.high, 6)), Style::default().fg(Color::Green)),
    ]);

    let meter_p = Paragraph::new(meter_line)
        .alignment(Alignment::Center)
        .block(Block::default());
    frame.render_widget(meter_p, preview_layout[1]);
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

    let engine_badge = Span::styled(
        format!(" [{}] ", state.engine.active_label()),
        Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD),
    );

    let prompt_line = Line::from(vec![
        Span::styled("Prompt: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(content, Style::default().fg(Color::White)),
        engine_badge,
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
            Span::styled("e", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Edit  "),
            Span::styled("t", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Tuning  "),
            Span::styled("m", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Engine  "),
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
        InputMode::SelectModel => vec![
            Span::styled("↑/↓ / j/k", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Select Engine  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Activate  "),
            Span::styled("Esc / m", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Close Overlay"),
        ],
        InputMode::Tuning => vec![
            Span::styled("↑/↓ / j/k", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Select Param  "),
            Span::styled("←/→ / +/-", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Adjust  "),
            Span::styled("r", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Reset  "),
            Span::styled("Esc / t", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Close Overlay"),
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
            Span::styled("  e         ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("Edit active sketch directly in your code editor ($EDITOR)"),
        ]),
        Line::from(vec![
            Span::styled("  t / a     ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("Open Audio DSP reactivity fine-tuning modal"),
        ]),
        Line::from(vec![
            Span::styled("  m         ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("Select AI Engine / Model (Claude, Codex, OpenAI, Mock)"),
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

fn render_model_selector_modal(frame: &mut Frame, area: Rect, state: &GrainState) {
    let popup_area = centered_rect(75, 65, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::LightCyan))
        .title(" AI Engine & Model Selector ");

    let mut text = vec![
        Line::from(vec![
            Span::styled("Select AI Generation Engine / Model", Style::default().add_modifier(Modifier::BOLD).fg(Color::LightCyan)),
            Span::raw("  "),
            Span::styled("(↑/↓ Navigate • Enter Activate Engine • Esc Close)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
    ];

    for (idx, opt) in state.engine.options.iter().enumerate() {
        let is_selected = idx == state.engine.selected_index;
        let is_active = idx == state.engine.active_index;

        let cursor = if is_selected { "▶ " } else { "  " };
        let active_badge = if is_active { " [ACTIVE]" } else { "" };

        let status_indicator = if opt.is_available {
            Span::styled(" ● Available ", Style::default().fg(Color::Green))
        } else {
            Span::styled(" ○ Missing CLI/Key ", Style::default().fg(Color::DarkGray))
        };

        let label_style = if is_selected {
            Style::default().fg(Color::Black).bg(Color::LightCyan).add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else if opt.is_available {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let active_span = if is_active {
            Span::styled(active_badge, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        } else {
            Span::raw("")
        };

        text.push(Line::from(vec![
            Span::styled(format!("{}{}", cursor, opt.label), label_style),
            active_span,
            Span::raw(" "),
            status_indicator,
            Span::styled(format!("— {}", opt.detail), Style::default().fg(Color::DarkGray)),
        ]));
    }

    text.push(Line::from(""));
    text.push(Line::from(Span::styled("Tip: Environment variables (.env) configure API keys and default provider.", Style::default().fg(Color::DarkGray))));
    text.push(Line::from(Span::styled("Press 'Enter' to activate selected engine, 'Esc' or 'm' to close", Style::default().fg(Color::DarkGray))));

    let p = Paragraph::new(text).block(block);
    frame.render_widget(p, popup_area);
}

fn render_versions_modal(frame: &mut Frame, area: Rect, state: &GrainState) {
    let popup_area = centered_rect(70, 60, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Blue))
        .title(" Sketch Version History (Instant Rollback) ");

    let mut text = vec![
        Line::from(vec![
            Span::styled("Available Visual Generations", Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)),
            Span::raw("  "),
            Span::styled("(↑/↓ Navigate • Enter Instant Rollback • Esc Close)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
    ];

    if state.versions.history.versions.is_empty() {
        text.push(Line::from(Span::styled("  No generated versions yet. Press 'g' to generate a sketch.", Style::default().fg(Color::DarkGray))));
    } else {
        for (idx, v_meta) in state.versions.history.versions.iter().enumerate() {
            let is_selected = idx == state.versions.selected_index;
            let is_active = v_meta.version == state.prompt.current_version;

            let cursor = if is_selected { "▶ " } else { "  " };
            let active_badge = if is_active { " [ACTIVE]" } else { "" };

            let base_style = if is_selected {
                Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let prompt_preview = if v_meta.prompt.len() > 40 {
                format!("{}...", &v_meta.prompt[..37])
            } else {
                v_meta.prompt.clone()
            };

            text.push(Line::from(vec![
                Span::styled(
                    format!(
                        "{}{:03} • sketch_v{} — \"{}\" (Seed: {}){}",
                        cursor, v_meta.version, v_meta.version, prompt_preview, v_meta.seed, active_badge
                    ),
                    base_style,
                ),
            ]));
        }
    }

    text.push(Line::from(""));
    text.push(Line::from(Span::styled("Press 'Enter' to rollback to selected version, 'Esc' or 'v' to exit", Style::default().fg(Color::DarkGray))));

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

fn render_tuning_modal(frame: &mut Frame, area: Rect, state: &GrainState) {
    let popup_area = centered_rect(75, 75, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Rgb(255, 180, 0)))
        .title(" 🎛️ Audio Reactivity & DSP Fine-Tuning ");

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if inner.height < 12 || inner.width < 30 {
        return;
    }

    let make_slider = |val: f32, min: f32, max: f32, width: usize| -> String {
        let norm = ((val - min) / (max - min)).clamp(0.0, 1.0);
        let pos = ((norm * (width - 1) as f32).round() as usize).min(width - 1);
        let mut s = String::with_capacity(width + 2);
        s.push('[');
        for i in 0..width {
            if i == pos {
                s.push('|');
            } else if i < pos {
                s.push('-');
            } else {
                s.push('-');
            }
        }
        s.push(']');
        s
    };

    let make_vu = |val: f32, width: usize| -> String {
        let filled = ((val.clamp(0.0, 1.0) * width as f32).round() as usize).min(width);
        "█".repeat(filled) + &"░".repeat(width - filled)
    };

    let mut text = vec![
        Line::from(vec![
            Span::styled("Calibrate Dynamic Range, Sensitivities & Smoothing", Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow)),
            Span::raw("  "),
            Span::styled("(↑/↓ Select • ←/→ Adjust • 'r' Reset • Esc Close)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
    ];

    let params: [(&str, String, &str); 7] = [
        (
            "Master Gain",
            format!("{} {:.1}x", make_slider(state.dsp.master_gain, 0.1, 4.0, 16), state.dsp.master_gain),
            "Overall audio energy multiplier across all visual channels",
        ),
        (
            "Low / Bass Gain",
            format!("{} {:.1}x", make_slider(state.dsp.low_gain, 0.1, 4.0, 16), state.dsp.low_gain),
            "Sensitivity for kick drums, sub-bass, and radial punch",
        ),
        (
            "Mid Gain",
            format!("{} {:.1}x", make_slider(state.dsp.mid_gain, 0.1, 4.0, 16), state.dsp.mid_gain),
            "Sensitivity for snares, synths, vocals, and shape morphing",
        ),
        (
            "High / Treble Gain",
            format!("{} {:.1}x", make_slider(state.dsp.high_gain, 0.1, 4.0, 16), state.dsp.high_gain),
            "Sensitivity for hi-hats, cymbals, shimmer, and spark bursts",
        ),
        (
            "Noise Gate / Cutoff",
            format!("{} {:.2}", make_slider(state.dsp.threshold, 0.0, 0.5, 16), state.dsp.threshold),
            "Threshold floor to eliminate quiet noise and idle visual jitter",
        ),
        (
            "Transient Smoothing",
            format!("{} {:.2}", make_slider(state.dsp.attack_decay, 0.0, 0.95, 16), state.dsp.attack_decay),
            "Attack/decay envelope (0.0 = snappy instant, 0.8 = smooth analog)",
        ),
        (
            "Auto Dynamic Range (AGC)",
            if state.dsp.auto_gain {
                "[ ON  / off ] Auto-scaled to song peak".to_string()
            } else {
                "[ on /  OFF ] Raw fixed scaling".to_string()
            },
            "Normalizes quiet or overly loud audio tracks automatically",
        ),
    ];

    for (idx, (name, val_str, desc)) in params.iter().enumerate() {
        let is_selected = idx == state.tuning_selected_param;
        let cursor = if is_selected { "▶ " } else { "  " };

        let name_style = if is_selected {
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        };

        let val_style = if is_selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        };

        text.push(Line::from(vec![
            Span::styled(format!("{}{:<24}", cursor, name), name_style),
            Span::raw(" "),
            Span::styled(format!("{:<28}", val_str), val_style),
            Span::styled(format!("— {}", desc), Style::default().fg(Color::DarkGray)),
        ]));
    }

    text.push(Line::from(""));
    text.push(Line::from(Span::styled("── Real-Time Calibrated VU Meters ────────────────────────────", Style::default().fg(Color::DarkGray))));

    let f = state.live_audio_features;
    text.push(Line::from(vec![
        Span::styled("  BASS [LOW] ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(format!("[{}] {:.2}", make_vu(f.low, 18), f.low), Style::default().fg(Color::Cyan)),
        Span::styled("   MID [SYNTH] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(format!("[{}] {:.2}", make_vu(f.mid, 18), f.mid), Style::default().fg(Color::Yellow)),
    ]));
    text.push(Line::from(vec![
        Span::styled("  TREB [HIGH]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::styled(format!("[{}] {:.2}", make_vu(f.high, 18), f.high), Style::default().fg(Color::Green)),
        Span::styled("   AMP [TOTAL] ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        Span::styled(format!("[{}] {:.2}", make_vu(f.amplitude, 18), f.amplitude), Style::default().fg(Color::Magenta)),
    ]));

    let p = Paragraph::new(text).block(Block::default());
    frame.render_widget(p, inner);
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

    #[test]
    fn test_ui_renders_model_selector_modal() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = GrainState::default();
        state.mode = InputMode::SelectModel;

        terminal.draw(|f| render(f, &state)).unwrap();
    }

    #[test]
    fn test_ui_renders_tuning_modal() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = GrainState::default();
        state.mode = InputMode::Tuning;

        terminal.draw(|f| render(f, &state)).unwrap();
    }
}
