//! Biquad filters (RBJ cookbook) applied forward and backward for zero phase.
//!
//! Offline analysis can afford zero-phase filtering, which keeps the
//! timing of each tick's sub-events where it really is.

use std::f64::consts::PI;

#[derive(Debug, Clone, Copy)]
pub struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Biquad {
    fn normalized(b: [f64; 3], a: [f64; 3]) -> Self {
        Biquad {
            b0: b[0] / a[0],
            b1: b[1] / a[0],
            b2: b[2] / a[0],
            a1: a[1] / a[0],
            a2: a[2] / a[0],
        }
    }

    pub fn highpass(fs: f64, f0: f64, q: f64) -> Self {
        let w0 = 2.0 * PI * f0 / fs;
        let (s, c) = w0.sin_cos();
        let alpha = s / (2.0 * q);
        Self::normalized(
            [(1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0],
            [1.0 + alpha, -2.0 * c, 1.0 - alpha],
        )
    }

    pub fn notch(fs: f64, f0: f64, q: f64) -> Self {
        let w0 = 2.0 * PI * f0 / fs;
        let (s, c) = w0.sin_cos();
        let alpha = s / (2.0 * q);
        Self::normalized([1.0, -2.0 * c, 1.0], [1.0 + alpha, -2.0 * c, 1.0 - alpha])
    }

    fn run<'a>(&self, x: impl Iterator<Item = &'a mut f32>) {
        let (mut x1, mut x2, mut y1, mut y2) = (0.0f64, 0.0, 0.0, 0.0);
        for v in x {
            let x0 = *v as f64;
            let y0 = self.b0 * x0 + self.b1 * x1 + self.b2 * x2 - self.a1 * y1 - self.a2 * y2;
            x2 = x1;
            x1 = x0;
            y2 = y1;
            y1 = y0;
            *v = y0 as f32;
        }
    }

    /// Filter in place forward, then backward (zero phase, squared magnitude).
    pub fn filtfilt(&self, x: &mut [f32]) {
        self.run(x.iter_mut());
        self.run(x.iter_mut().rev());
    }
}

/// Q values of the two sections of a 4th-order Butterworth filter.
pub const BUTTERWORTH4_Q: [f64; 2] = [0.541_196_1, 1.306_563];

/// 4th-order Butterworth high-pass, zero phase.
pub fn highpass4(x: &mut [f32], fs: f64, f0: f64) {
    for q in BUTTERWORTH4_Q {
        Biquad::highpass(fs, f0, q).filtfilt(x);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FS: f64 = 48_000.0;

    fn sine(f: f64, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * PI * f * i as f64 / FS).sin() as f32)
            .collect()
    }

    /// The rms of the middle half, away from the ends' start-up.
    fn rms_mid(x: &[f32]) -> f64 {
        let m = &x[x.len() / 4..3 * x.len() / 4];
        (m.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / m.len() as f64).sqrt()
    }

    /// The 4th-order Butterworth high-pass, run forward and back, is down
    /// 6 dB at its corner (twice the 3 dB of one pass), keeps what is well
    /// above it and takes out what is well below: 8th order in all.
    #[test]
    fn the_highpass_keeps_whats_above_its_corner_and_takes_out_whats_below() {
        let f0 = 1000.0;
        let gain = |f: f64| {
            let mut x = sine(f, 48_000);
            highpass4(&mut x, FS, f0);
            rms_mid(&x) / std::f64::consts::FRAC_1_SQRT_2
        };
        assert!((gain(f0) - 0.5).abs() < 0.01, "{}", gain(f0));
        assert!((gain(8.0 * f0) - 1.0).abs() < 0.001, "{}", gain(8.0 * f0));
        // An octave below, 8th order: 48 dB down, 1/256 in amplitude.
        assert!(gain(f0 / 2.0) < 1.0 / 200.0, "{}", gain(f0 / 2.0));
        assert!(gain(50.0) < 1e-6, "{}", gain(50.0));
    }

    /// Forward then back, nothing is moved in time: a click's peak stays
    /// on its sample, as a tick's sub-events must.
    #[test]
    fn the_filters_move_nothing_in_time() {
        let mut x = vec![0.0f32; 4801];
        x[2400] = 1.0;
        highpass4(&mut x, FS, 1000.0);
        let peak = (0..x.len())
            .max_by(|&a, &b| x[a].abs().total_cmp(&x[b].abs()))
            .unwrap();
        assert_eq!(peak, 2400);
        // Symmetric about it, as only a zero-phase filter leaves it.
        for k in 1..200 {
            assert!((x[2400 - k] - x[2400 + k]).abs() < 1e-6, "{k}");
        }
    }

    /// The notch takes out its frequency, mains hum, and leaves the rest.
    #[test]
    fn the_notch_takes_out_its_frequency_only() {
        let gain = |f: f64| {
            let mut x = sine(f, 8 * 48_000);
            Biquad::notch(FS, 50.0, 30.0).filtfilt(&mut x);
            rms_mid(&x) / std::f64::consts::FRAC_1_SQRT_2
        };
        assert!(gain(50.0) < 1e-3, "{}", gain(50.0));
        assert!((gain(5000.0) - 1.0).abs() < 1e-3, "{}", gain(5000.0));
        assert!((gain(25.0) - 1.0).abs() < 0.01, "{}", gain(25.0));
    }
}
