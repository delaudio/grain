#![cfg(windows)]

use grain::audio::AudioPlayer;

#[test]
fn audio_owner_survives_sequential_caller_threads() {
    for iteration in 0..4 {
        std::thread::spawn(move || {
            eprintln!("audio lifecycle {iteration}: create");
            let mut player = AudioPlayer::new();
            // A valid fixture exercises the owner even on a runner without an
            // endpoint; owner failure must be an error, not a passing no-op.
            player
                .load(std::path::Path::new("fixtures/demo.wav"))
                .unwrap();
            player.pause();
            assert!(!player.is_playing());
            eprintln!("audio lifecycle {iteration}: drop");
            drop(player);
            eprintln!("audio lifecycle {iteration}: dropped");
        })
        .join()
        .unwrap();
    }
}

#[test]
fn audio_proxy_can_move_between_threads() {
    let player = std::thread::spawn(AudioPlayer::new).join().unwrap();
    std::thread::spawn(move || {
        let mut player = player;
        player
            .load(std::path::Path::new("fixtures/demo.wav"))
            .unwrap();
        player.pause();
        assert!(!player.is_playing());
    })
    .join()
    .unwrap();
}
