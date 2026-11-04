//! A figure as text, the one rule every surface writes it by: its sign,
//! its ± and its decimals (math guide section 11; #806, #815, #510 K3).
//! Core keeps full precision and JSON is never rounded; only text is, once,
//! here.

/// A signed figure as the app shows it, to `digits` decimals: `+1.2`,
/// `-1.2`, and a figure that rounds to zero takes no sign (`0.0`, never
/// `-0.0` or `+0.0`). The app sets the minus as a true minus sign.
pub fn signed_text(x: f64, digits: usize) -> String {
    let s = format!("{:.digits$}", x.abs());
    if s.bytes().all(|b| b == b'0' || b == b'.') {
        s
    } else if x < 0.0 {
        format!("-{s}")
    } else {
        format!("+{s}")
    }
}

/// A figure to `digits` decimals with its sign as it is, but one that
/// rounds to zero written without a minus (`0.0`, never `-0.0`): the one
/// rule for that (#815). [`signed_text`] also adds a plus.
pub fn fixed_text(x: f64, digits: usize) -> String {
    let text = format!("{x:.digits$}");
    match text.strip_prefix('-') {
        Some(rest) if rest.bytes().all(|b| b == b'0' || b == b'.') => rest.to_string(),
        _ => text,
    }
}

/// The most decimals a figure or its ± is written to: below 0.001 a ± is
/// far inside any clock or lift-angle accuracy (#510 K3).
pub const MAX_DIGITS: usize = 3;

/// Each figure's base decimals, with no ± to follow (the math guide's
/// section 11): what [`figure_digits`] starts from.
pub mod base_digits {
    /// Rate, s/d.
    pub const RATE: usize = 1;
    /// Rate p05 to p95, s/d.
    pub const RATE_RANGE: usize = 0;
    /// Amplitude, degrees.
    pub const AMPLITUDE: usize = 0;
    /// Beat error, ms.
    pub const BEAT_ERROR: usize = 2;
    /// Jitter, µs.
    pub const JITTER: usize = 0;
    /// Clock, ppm.
    pub const CLOCK: usize = 1;
}

/// How many decimals a figure is written to (#510 K3, "decimals follow
/// the ±"): with a ± (`se` positive and finite), the ±'s own decimals as
/// [`pm_text`] writes them, so value and ± always carry the same; with
/// none, the figure's `base` (the math guide's section 11: rate 1, rate
/// p05 to p95 0, amplitude 0, beat error 2, jitter 0, clock 1). Never fewer
/// than `base`, however wide the ±, and no more than [`MAX_DIGITS`] beyond
/// that.
pub fn figure_digits(se: Option<f64>, base: usize) -> usize {
    let Some(se) = se.filter(|se| se.is_finite() && *se > 0.0) else {
        return base;
    };
    let mut d = base;
    while d < MAX_DIGITS && se * 10f64.powi(d as i32) < 0.95 {
        d += 1;
    }
    d
}

/// A standard error as every surface writes it after "±": to
/// [`figure_digits`] decimals, at least `digits` and more, up to
/// [`MAX_DIGITS`], until its first figure that isn't zero shows (#806). So
/// with `digits` 0 an amplitude's 0.4° reads `0.4` and 1.4° reads `1`;
/// with 1, a rate's 0.04 s/d reads `0.04`. One that still rounds to zero
/// at the cap reads `<0.001`, never `0.000`. `None` when it isn't a
/// positive, finite number.
pub fn pm_text(se: f64, digits: usize) -> Option<String> {
    if !(se.is_finite() && se > 0.0) {
        return None;
    }
    let d = figure_digits(Some(se), digits);
    let text = format!("{se:.d$}");
    if text.bytes().all(|b| b == b'0' || b == b'.') {
        return Some(format!("<{:.d$}", 10f64.powi(-(d as i32))));
    }
    Some(text)
}

/// A figure as text to the decimals its ± calls for ([`figure_digits`],
/// #510 K3: decimals follow the ±), from the figure's `base` decimals
/// with no ± ([`base_digits`]). `signed` writes a rate's or clock's sign,
/// none on a zero ([`signed_text`]); else a zero takes no minus
/// ([`fixed_text`]). `se` is the figure's standard error, if it has one.
pub fn figure(v: f64, se: Option<f64>, base: usize, signed: bool) -> String {
    let d = figure_digits(se, base);
    if signed {
        signed_text(v, d)
    } else {
        fixed_text(v, d)
    }
}

/// A figure and, where it has one, its ± after it, each to the same
/// decimals ([`figure`], [`pm_text`]): "+1.23 ± 0.04".
pub fn figure_with_pm(v: f64, se: Option<f64>, base: usize, signed: bool) -> String {
    let pm = se
        .and_then(|e| pm_text(e, base))
        .map_or_else(String::new, |t| format!(" ± {t}"));
    format!("{}{pm}", figure(v, se, base, signed))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One rule for the ± on every surface (#806): the case the MCP view
    /// got wrong is 0.4° of amplitude, which reads 0.4, not 0 or 1.
    #[test]
    fn a_standard_error_shows_its_first_figure() {
        assert_eq!(pm_text(0.4, 0).as_deref(), Some("0.4"));
        assert_eq!(pm_text(0.94, 0).as_deref(), Some("0.9"));
        assert_eq!(pm_text(0.96, 0).as_deref(), Some("1"));
        assert_eq!(pm_text(1.4, 0).as_deref(), Some("1"));
        assert_eq!(pm_text(0.04, 1).as_deref(), Some("0.04"));
        assert_eq!(pm_text(0.3, 1).as_deref(), Some("0.3"));
        assert_eq!(pm_text(0.012, 2).as_deref(), Some("0.01"));
        assert_eq!(pm_text(0.004, 2).as_deref(), Some("0.004"));
        // At the cap, a ± that still rounds to zero is under the last
        // decimal, never 0.000 (#510 K3).
        assert_eq!(pm_text(0.0001, 0).as_deref(), Some("<0.001"));
        assert_eq!(pm_text(0.0006, 0).as_deref(), Some("0.001"));
        assert_eq!(pm_text(0.0, 1), None);
        assert_eq!(pm_text(f64::NAN, 1), None);
    }

    /// Decimals follow the ± (#510 K3): value and ± carry the same
    /// decimals, never fewer than the figure's base, at most three.
    #[test]
    fn a_figure_takes_its_plus_minus_decimals() {
        // Rate, base 1.
        assert_eq!(figure_digits(Some(0.007), 1), 3);
        assert_eq!(figure_digits(Some(0.04), 1), 2);
        assert_eq!(figure_digits(Some(0.3), 1), 1);
        assert_eq!(figure_digits(Some(25.0), 1), 1, "never fewer than the base");
        // Amplitude, base 0.
        assert_eq!(figure_digits(Some(0.4), 0), 1);
        assert_eq!(figure_digits(Some(1.4), 0), 0);
        // At most three, however small the ±.
        assert_eq!(figure_digits(Some(1e-6), 0), 3);
        // No ±: the base.
        for se in [
            None,
            Some(0.0),
            Some(-1.0),
            Some(f64::NAN),
            Some(f64::INFINITY),
        ] {
            assert_eq!(figure_digits(se, 2), 2);
        }
        // The value and its ± carry the same decimals.
        for (se, base) in [(0.007, 1), (0.4, 0), (25.0, 1), (0.012, 2), (0.0001, 0)] {
            let d = figure_digits(Some(se), base);
            let pm = pm_text(se, base).unwrap();
            let pm_d = pm.split_once('.').map_or(0, |(_, f)| f.len());
            assert_eq!(pm_d, d, "{se} at base {base}: ± {pm}");
        }
        assert_eq!(signed_text(5.0, figure_digits(Some(0.007), 1)), "+5.000");
    }

    /// A figure carries its ±'s decimals, from its base where there is no
    /// ±; a signed zero takes no sign, an unsigned one no minus.
    #[test]
    fn a_figure_is_written_to_its_plus_minus_decimals() {
        use base_digits as base;
        assert_eq!(figure(1.234, None, base::RATE, true), "+1.2");
        assert_eq!(figure(1.234, Some(0.3), base::RATE, true), "+1.2");
        assert_eq!(figure(1.234, Some(0.04), base::RATE, true), "+1.23");
        assert_eq!(figure(-0.04, Some(0.3), base::RATE, true), "0.0");
        assert_eq!(
            figure_with_pm(1.234, Some(0.04), base::RATE, true),
            "+1.23 ± 0.04"
        );
        assert_eq!(figure(285.37, Some(1.4), base::AMPLITUDE, false), "285");
        assert_eq!(figure(285.37, Some(0.4), base::AMPLITUDE, false), "285.4");
        assert_eq!(
            figure_with_pm(285.37, Some(0.4), base::AMPLITUDE, false),
            "285.4 ± 0.4"
        );
        assert_eq!(figure(0.1234, None, base::BEAT_ERROR, false), "0.12");
        assert_eq!(
            figure(0.1234, Some(0.004), base::BEAT_ERROR, false),
            "0.123"
        );
        assert_eq!(
            figure_with_pm(0.1234, Some(0.0001), base::BEAT_ERROR, false),
            "0.123 ± <0.001"
        );
        assert_eq!(figure(-0.2, None, base::AMPLITUDE, false), "0");
        assert_eq!(figure_with_pm(3.0, None, base::RATE, true), "+3.0");
    }
}
