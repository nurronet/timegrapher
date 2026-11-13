//! A lift change while listening reads each window again from its edges
//! at the new lift and gates it there, both ways, as a kept take does
//! (#1333): a side the old lift culled comes back, and one the new lift
//! puts out of range goes.

use timegrapher_core::live::{LiveAnalyzer, LiveConfig};
use timegrapher_core::synth::{generate, SynthConfig};

#[test]
fn a_live_lift_change_brings_culled_sides_back_and_culls_again() {
    // 395° at the synth's 52°: over the 380° gate, so every side is
    // culled; about 342° at 45°.
    let cfg = SynthConfig {
        duration_s: 30.0,
        ..Default::default()
    };
    let audio = generate(&cfg, |_| 395.0, |_| 0.0);
    let mut l = LiveAnalyzer::new(cfg.sample_rate, LiveConfig::default());
    for c in audio.samples.chunks(4800) {
        l.push(c);
    }
    let read = |l: &LiveAnalyzer| l.reading(20.0).amplitude_deg;
    assert!(!l.amplitude_windows().is_empty());
    assert_eq!(read(&l), None, "culled at 52°");
    l.set_lift(45.0);
    let at45 = read(&l).expect("back at 45°");
    assert!((at45 - 395.0 * 45.0 / 52.0).abs() < 5.0, "{at45}");
    l.set_lift(52.0);
    assert_eq!(read(&l), None, "culled again at 52°");
}
