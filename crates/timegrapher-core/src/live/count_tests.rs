//! The beats a live reading counts, against those its stretch should
//! hold.

use super::tests::feed;
use super::*;
use crate::synth::{generate, SynthConfig};

/// The beats a live reading counts are those of its own stretch, last
/// less first (#1417): a watch that loses about one beat in ten (0.1 s
/// of sound gone every 0.625 s) is named for its missing beats, with
/// the share found, and the same watch heard whole is not.
#[test]
fn a_reading_names_the_beats_its_stretch_is_missing() {
    let audio = generate(
        &SynthConfig {
            duration_s: 40.0,
            ..Default::default()
        },
        |_| 280.0,
        |_| 0.0,
    );
    let fs = audio.sample_rate as usize;
    let read = |x: &[f32]| {
        let mut a = LiveAnalyzer::new(audio.sample_rate, LiveConfig::default());
        feed(&mut a, x);
        a.reading(10.0).caveats
    };
    let missing = |c: &[Caveat]| {
        c.iter().find_map(|c| match c {
            Caveat::BeatsMissing { fraction } => Some(*fraction),
            _ => None,
        })
    };
    assert_eq!(missing(&read(&audio.samples)), None);
    let mut lossy = audio.samples.clone();
    for k in 0..59 {
        let i = ((2.0 + 0.625 * k as f64) * fs as f64) as usize;
        lossy[i..i + fs / 10].fill(0.0);
    }
    let found = missing(&read(&lossy)).expect("beats missing");
    assert!((0.85..0.95).contains(&found), "{found}");
}
