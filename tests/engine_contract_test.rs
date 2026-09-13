use grain::history::HistoryManager;
use grain::history::record::VersionMetadata;
use grain::runtime::engine::{CONTRACT_VERSION, CellFrame, EngineId, FrameOutput};
use grain::runtime::{RasterFrame, TerminalCell};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

fn cell_frame(symbol: &str) -> FrameOutput {
    FrameOutput::Cells(CellFrame {
        cols: 1,
        rows: 1,
        cells: vec![vec![TerminalCell {
            symbol: symbol.into(),
            r: 10,
            g: 20,
            b: 30,
            background: Some([40, 50, 60]),
        }]],
    })
}

#[test]
fn cells_reject_controls_wide_and_multiscalar_symbols() {
    for symbol in ["A", " ", "\u{2580}"] {
        assert!(cell_frame(symbol).validate().is_ok(), "{symbol:?}");
    }
    for symbol in [
        "",
        "ab",
        "\n",
        "\x1b",
        "\u{7f}",
        "\u{301}",
        "e\u{301}",
        "\u{4e00}",
        "\u{1f600}",
    ] {
        assert!(cell_frame(symbol).validate().is_err(), "{symbol:?}");
    }
}

#[test]
fn outputs_validate_shape_and_preserve_native_cells() {
    let original = cell_frame("#");
    let encoded = serde_json::to_string(&original).unwrap();
    assert_eq!(
        serde_json::from_str::<FrameOutput>(&encoded).unwrap(),
        original
    );
    let FrameOutput::Cells(mut grid) = original else {
        unreachable!()
    };
    grid.cols = 2;
    assert!(FrameOutput::Cells(grid).validate().is_err());
    let raster = RasterFrame {
        width: 1,
        height: 1,
        rgba: vec![1, 2, 3, 255],
    };
    assert!(FrameOutput::Raster(raster.clone()).validate().is_ok());
    let mut invalid = raster;
    invalid.width = 4097;
    assert!(FrameOutput::Raster(invalid).validate().is_err());
}

#[test]
fn legacy_metadata_migrates_to_p5_without_changing_source_reference() {
    let legacy = r#"{"version":3,"timestamp":0,"prompt":"old","seed":42,"provider":"mock","runtime_contract":"grain-p5-v1","audio_source_hash":null,"sketch_file":"003.js"}"#;
    let metadata: VersionMetadata = serde_json::from_str(legacy).unwrap();
    assert_eq!(metadata.engine, EngineId::P5);
    assert_eq!(metadata.contract_version, CONTRACT_VERSION);
    assert_eq!(metadata.sketch_file, "003.js");
    assert_eq!(metadata.seed, 42);
}

struct TempHistory(std::path::PathBuf);
impl TempHistory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "grain-engine-history-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        Self(dir)
    }
    fn manager(&self) -> HistoryManager {
        HistoryManager::new(self.0.clone())
    }
}
impl Drop for TempHistory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn engine_metadata_roundtrips_and_sparse_versions_do_not_collide() {
    let temp = TempHistory::new();
    let manager = temp.manager();
    let first = manager
        .record_new_version_for_engine(
            "cells",
            "function main() {}",
            7,
            "mock",
            None,
            EngineId::Ascii,
        )
        .unwrap();
    assert_eq!(first.engine, EngineId::Ascii);
    assert_eq!(first.runtime_contract, "grain-ascii-v1");
    assert_eq!(manager.load_history().unwrap().versions[0], first);
    let mut history = manager.load_history().unwrap();
    history.versions[0].version = 5;
    history.active_version = 5;
    manager.save_history(&history).unwrap();
    assert_eq!(
        manager
            .record_new_version("p5", "source", 8, "mock", None)
            .unwrap()
            .version,
        6
    );
}

#[test]
fn corrupt_history_and_orphan_source_are_never_overwritten() {
    let temp = TempHistory::new();
    let manager = temp.manager();
    manager.init_dirs().unwrap();
    let history = temp.0.join("generations.json");
    fs::write(&history, "{broken").unwrap();
    assert!(
        manager
            .record_new_version("test", "new", 0, "mock", None)
            .is_err()
    );
    assert_eq!(fs::read_to_string(&history).unwrap(), "{broken");
    assert!(manager.get_active_sketch_path().is_err());
    fs::remove_file(history).unwrap();
    let orphan = temp.0.join("sketches/001.js");
    fs::write(&orphan, "original").unwrap();
    let recovered = manager
        .record_new_version("test", "new", 0, "mock", None)
        .unwrap();
    assert_eq!(recovered.version, 2);
    assert_eq!(manager.load_history().unwrap().active_version, 2);
    assert_eq!(
        manager.load_sketch_content(&recovered.sketch_file).unwrap(),
        "new"
    );
    assert_eq!(fs::read_to_string(orphan).unwrap(), "original");
}
