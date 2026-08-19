use std::io::Write;
use std::process::{Command, Stdio};
use serde::{Deserialize, Serialize};
use crate::runtime::contract::{FrameRenderResult, GrainContext, RuntimeDiagnostic};

#[allow(dead_code)]
const RUNNER_JS: &str = include_str!("js/runner.js");

#[derive(Serialize)]
struct RunnerRequest<'a> {
    source: &'a str,
    context: &'a GrainContext,
    #[serde(rename = "termCols")]
    term_cols: u16,
    #[serde(rename = "termRows")]
    term_rows: u16,
}

#[derive(Deserialize)]
struct RunnerResponse {
    success: bool,
    frame: Option<usize>,
    width: Option<u32>,
    height: Option<u32>,
    ascii_art: Option<String>,
    draw_commands_count: Option<usize>,
    error: Option<RunnerErrorResponse>,
}

#[derive(Deserialize)]
struct RunnerErrorResponse {
    message: String,
    line: Option<usize>,
    column: Option<usize>,
    stack: Option<String>,
}

pub fn evaluate_frame(
    source: &str,
    context: &GrainContext,
    term_cols: u16,
    term_rows: u16,
) -> Result<FrameRenderResult, RuntimeDiagnostic> {
    // Check if node is available to run full p5.js sandbox
    let mut child = match Command::new("node")
        .arg("-e")
        .arg(RUNNER_JS)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            // If node is not found, fallback to pure deterministic Rust evaluation
            return fallback_evaluate_frame(source, context, term_cols, term_rows, e);
        }
    };

    let req = RunnerRequest {
        source,
        context,
        term_cols,
        term_rows,
    };

    let input_json = serde_json::to_string(&req).map_err(|e| RuntimeDiagnostic {
        message: format!("Serialization error: {}", e),
        line: None,
        column: None,
        stack: None,
    })?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input_json.as_bytes());
    }

    let output = child.wait_with_output().map_err(|e| RuntimeDiagnostic {
        message: format!("Process execution error: {}", e),
        line: None,
        column: None,
        stack: None,
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(RuntimeDiagnostic {
            message: if stderr.is_empty() {
                "Runtime execution failed".to_string()
            } else {
                stderr
            },
            line: None,
            column: None,
            stack: None,
        });
    }

    let res: RunnerResponse = serde_json::from_slice(&output.stdout).map_err(|e| RuntimeDiagnostic {
        message: format!("Failed to parse runner output: {}", e),
        line: None,
        column: None,
        stack: None,
    })?;

    if res.success {
        Ok(FrameRenderResult {
            frame: res.frame.unwrap_or(context.frame),
            width: res.width.unwrap_or(context.width),
            height: res.height.unwrap_or(context.height),
            ascii_art: res.ascii_art,
            draw_commands_count: res.draw_commands_count.unwrap_or(0),
        })
    } else if let Some(err) = res.error {
        Err(RuntimeDiagnostic {
            message: err.message,
            line: err.line,
            column: err.column,
            stack: err.stack,
        })
    } else {
        Err(RuntimeDiagnostic {
            message: "Unknown runtime error".to_string(),
            line: None,
            column: None,
            stack: None,
        })
    }
}

fn fallback_evaluate_frame(
    _source: &str,
    context: &GrainContext,
    term_cols: u16,
    term_rows: u16,
    _err: std::io::Error,
) -> Result<FrameRenderResult, RuntimeDiagnostic> {
    // Pure Rust deterministic fallback preview
    let cols = term_cols as usize;
    let rows = term_rows as usize;
    let mut grid = vec![vec![' '; cols]; rows];

    let cx = cols as f64 / 2.0;
    let cy = rows as f64 / 2.0;
    let radius_cols = (cols as f64 * 0.25 * (0.5 + context.audio.amplitude as f64 * 0.5)).max(2.0);
    let radius_rows = radius_cols * 0.5;

    let char_ramp = [' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];

    for r in 0..rows {
        for c in 0..cols {
            let dx = (c as f64 - cx) / radius_cols;
            let dy = (r as f64 - cy) / radius_rows;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= 1.0 {
                let intensity = 1.0 - dist_sq * 0.5;
                let char_idx = ((intensity * (char_ramp.len() - 1) as f64).round() as usize).min(char_ramp.len() - 1);
                grid[r][c] = char_ramp[char_idx];
            }
        }
    }

    let ascii_art = grid.iter().map(|row| row.iter().collect::<String>()).collect::<Vec<_>>().join("\n");

    Ok(FrameRenderResult {
        frame: context.frame,
        width: context.width,
        height: context.height,
        ascii_art: Some(ascii_art),
        draw_commands_count: 1,
    })
}
