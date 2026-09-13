use grain::audio::AudioFeatures;
use grain::runtime::{GrainContext, RasterFrame, evaluate_frame};

fn raster(body: &str) -> RasterFrame {
    let context = GrainContext {
        params: Default::default(),
        width: 16,
        height: 16,
        frame: 0,
        time: 0.0,
        seed: 42,
        audio: AudioFeatures::default(),
    };
    evaluate_frame(&format!("function draw(p) {{ {body} }}"), &context, 8, 4)
        .unwrap()
        .raster
        .unwrap()
}

#[test]
fn opaque_color_round_trips_through_fill_background_and_stroke() {
    for (direct, parsed) in [
        (
            "p.noStroke(); p.fill('#ff0000'); p.rect(0, 0, 16, 16);",
            "p.noStroke(); p.fill(p.color('#ff0000')); p.rect(0, 0, 16, 16);",
        ),
        (
            "p.background('#ff0000');",
            "p.background(p.color('#ff0000'));",
        ),
        (
            "p.stroke('#ff0000'); p.strokeWeight(4); p.line(0, 8, 16, 8);",
            "p.stroke(p.color('#ff0000')); p.strokeWeight(4); p.line(0, 8, 16, 8);",
        ),
    ] {
        let expected = raster(direct);
        assert!(
            expected
                .rgba
                .chunks_exact(4)
                .any(|pixel| pixel == [255, 0, 0, 255])
        );
        assert_eq!(raster(parsed), expected);
    }
}

#[test]
fn parsed_alpha_is_preserved_but_raw_arrays_use_the_current_range() {
    let expected =
        raster("p.background(0); p.noStroke(); p.fill(255, 0, 0, 128); p.rect(0, 0, 16, 16);");
    assert_eq!(&expected.rgba[..4], &[128, 0, 0, 255]);
    assert_eq!(
        raster(
            "p.background(0); p.noStroke(); p.fill(p.color(255, 0, 0, 128)); p.rect(0, 0, 16, 16);"
        ),
        expected
    );
    assert_eq!(
        raster("p.background(0); p.noStroke(); p.fill([255, 0, 0, 128]); p.rect(0, 0, 16, 16);"),
        expected
    );
}

#[test]
fn parsed_colors_survive_color_mode_changes_and_color_accessors() {
    let expected = raster("p.background(255, 0, 0);");
    let actual = raster(
        "const c = p.color('#ff0000'); p.colorMode(p.HSB); p.background(c); if (p.red(c) !== 255 || p.green(c) !== 0 || p.blue(c) !== 0 || p.alpha(c) !== 1) throw new Error('color accessor regression');",
    );
    assert_eq!(actual, expected);
}
