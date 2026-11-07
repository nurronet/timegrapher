//! Source-aware position and session aggregates, sharing the legacy estimator.
use super::*;

/// Position, rate, amplitude, unsigned beat error, and its known source.
pub type SourcedFigures = (
    Position,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<BeatErrorSource>,
);

/// Position averages and state indices, choosing one known beat-error source.
/// Values with unknown provenance remain available to callers as individual rows,
/// but never enter a pooled beat-error figure.
pub fn sourced_indices(wind_h: f64, figures: &[SourcedFigures]) -> StateIndices {
    let positions: Vec<PositionValue> = Position::ALL
        .iter()
        .filter_map(|&p| {
            let rs: Vec<&SourcedFigures> = figures.iter().filter(|f| f.0 == p).collect();
            if rs.is_empty() {
                return None;
            }
            let selected = select_beat_errors(rs.iter().map(|r| r.3.zip(r.4)));
            let values = selected.values.iter().map(|(_, value)| *value);
            Some(PositionValue {
                position: p,
                rate_s_per_day: mean_of(rs.iter().filter_map(|f| f.1)),
                amplitude_deg: mean_of(rs.iter().filter_map(|f| f.2)),
                beat_error_ms: mean_of(values),
                beat_error_source: selected.beat_error_source,
                beat_error_sources: selected.beat_error_sources,
                caveats: source_caveats(&selected, rs.iter().copied()),
                readings: rs.len(),
            })
        })
        .collect();
    let rate = |pred: &dyn Fn(Position) -> bool| -> Vec<f64> {
        positions
            .iter()
            .filter(|v| pred(v.position))
            .filter_map(|v| v.rate_s_per_day)
            .collect()
    };
    let amp = |pred: &dyn Fn(Position) -> bool| -> Vec<f64> {
        positions
            .iter()
            .filter(|v| pred(v.position))
            .filter_map(|v| v.amplitude_deg)
            .collect()
    };
    let all = |_: Position| true;
    let vert = |p: Position| p.is_vertical();
    let hor = |p: Position| !p.is_vertical();
    let xh = mean_of(rate(&hor).into_iter());
    let xv = mean_of(rate(&vert).into_iter());
    let ah = mean_of(amp(&hor).into_iter());
    let av = mean_of(amp(&vert).into_iter());
    let get = |p: Position| {
        positions
            .iter()
            .find(|v| v.position == p)
            .and_then(|v| v.rate_s_per_day)
    };
    let selected = select_beat_errors(
        positions
            .iter()
            .map(|p| p.beat_error_ms.zip(p.beat_error_source)),
    );
    let beat_errors: Vec<_> = selected.values.iter().map(|(_, value)| *value).collect();
    StateIndices {
        wind_h,
        x: mean_of(rate(&all).into_iter()),
        xh,
        xv,
        d_rate: spread(rate(&all).into_iter()),
        d_amplitude: spread(amp(&all).into_iter()),
        dv_rate: spread(rate(&vert).into_iter()),
        dh_rate: spread(rate(&hor).into_iter()),
        dvh_rate: xv.zip(xh).map(|(v, h)| v - h),
        dvh_amplitude: av.zip(ah).map(|(v, h)| v - h),
        di: get(Position::H6).zip(get(Position::CH)).map(|(a, b)| a - b),
        amplitude_mean: mean_of(amp(&all).into_iter()),
        beat_error_mean: mean_of(beat_errors.iter().copied()),
        d_beat_error: spread(beat_errors.iter().copied()),
        beat_error_source: selected.beat_error_source,
        beat_error_sources: selected.beat_error_sources,
        caveats: source_caveats(&selected, figures.iter()),
        positions,
    }
}

impl StateIndices {
    /// Words for a position-table mean, including the actual source count and
    /// why legacy unknown-source readings do not contribute.
    pub fn beat_error_summary(&self) -> String {
        let mut text = match self.beat_error_source {
            Some(source) => {
                let used = match source {
                    BeatErrorSource::Unlock => self.beat_error_sources.unlock,
                    BeatErrorSource::Drop => self.beat_error_sources.drop,
                };
                format!(
                    "{}: mean of {used} of {} positions.",
                    beat_error_source_label(Some(source)),
                    self.positions.len()
                )
            }
            None => format!("{}: no mean.", beat_error_source_label(None)),
        };
        for caveat in &self.caveats {
            if matches!(
                caveat,
                crate::measure::Caveat::BeatErrorUnknownSource { .. }
            ) {
                if let Some(note) = caveat.long(None) {
                    text.push(' ');
                    text.push_str(&note);
                }
            }
        }
        text
    }
}

fn source_caveats<'a>(
    selected: &BeatErrorSelection,
    figures: impl Iterator<Item = &'a SourcedFigures>,
) -> Vec<crate::measure::Caveat> {
    let mut caveats = selected.caveats();
    let readings = figures
        .filter(|f| f.4.is_none() && f.3.is_some_and(f64::is_finite))
        .count();
    if readings > 0 {
        caveats.push(crate::measure::Caveat::BeatErrorUnknownSource { readings });
    }
    caveats
}

#[cfg(test)]
mod tests {
    use super::*;
    fn row(p: Position, value: f64, source: BeatErrorSource) -> SourcedFigures {
        (p, Some(1.0), Some(270.0), Some(value), Some(source))
    }
    #[test]
    fn position_and_state_aggregates_never_mix_sources() {
        let s = sourced_indices(
            0.0,
            &[
                row(Position::CH, 0.2, BeatErrorSource::Unlock),
                row(Position::CH, 0.4, BeatErrorSource::Unlock),
                row(Position::CH, 8.0, BeatErrorSource::Drop),
                row(Position::CB, 9.0, BeatErrorSource::Drop),
            ],
        );
        let du = s
            .positions
            .iter()
            .find(|p| p.position == Position::CH)
            .unwrap();
        assert!((du.beat_error_ms.unwrap() - 0.3).abs() < 1e-12);
        assert_eq!(
            du.beat_error_sources,
            BeatErrorSources { unlock: 2, drop: 1 }
        );
        assert!(matches!(
            du.caveats.as_slice(),
            [crate::measure::Caveat::BeatErrorSourceReadings {
                used: 2,
                readings: 3,
                counted: crate::measure::Counted::Readings,
                ..
            }]
        ));
        assert!((s.beat_error_mean.unwrap() - 0.3).abs() < 1e-12);
        assert_eq!(s.beat_error_source, Some(BeatErrorSource::Unlock));
        assert_eq!(
            s.beat_error_sources,
            BeatErrorSources { unlock: 1, drop: 1 }
        );
        assert_eq!(
            s.d_beat_error, None,
            "one chosen-source position cannot give a spread"
        );
    }
    #[test]
    fn drop_only_position_statistics_keep_their_source() {
        let s = sourced_indices(
            0.0,
            &[
                row(Position::CH, 0.4, BeatErrorSource::Drop),
                row(Position::CB, 0.8, BeatErrorSource::Drop),
            ],
        );
        assert_eq!(s.beat_error_source, Some(BeatErrorSource::Drop));
        assert!((s.beat_error_mean.unwrap() - 0.6).abs() < 1e-12);
        assert_eq!(s.d_beat_error, Some(0.4));
        assert!(matches!(
            s.caveats.as_slice(),
            [crate::measure::Caveat::BeatErrorFromDrop]
        ));
    }
    #[test]
    fn unknown_source_rows_never_enter_beat_error_aggregates() {
        let figures = [
            (Position::CH, Some(2.0), Some(280.0), Some(0.4), None),
            (Position::CB, Some(4.0), Some(260.0), Some(0.8), None),
        ];
        let s = sourced_indices(0.0, &figures);
        assert_eq!(s.beat_error_mean, None);
        assert_eq!(s.d_beat_error, None);
        assert_eq!(s.beat_error_source, None);
        assert_eq!(s.x, Some(3.0));
        assert_eq!(s.amplitude_mean, Some(270.0));
        assert!(s.positions.iter().all(|p| p.beat_error_ms.is_none()));
        assert!(matches!(
            s.caveats.as_slice(),
            [crate::measure::Caveat::BeatErrorUnknownSource { readings: 2 }]
        ));
        let mixed = [figures[0], row(Position::CH, 0.2, BeatErrorSource::Unlock)];
        let s = sourced_indices(0.0, &mixed);
        assert_eq!(s.beat_error_mean, Some(0.2));
        assert_eq!(s.beat_error_source, Some(BeatErrorSource::Unlock));
        assert!(matches!(
            s.positions[0].caveats.as_slice(),
            [crate::measure::Caveat::BeatErrorUnknownSource { readings: 1 }]
        ));
    }
    #[test]
    fn position_mean_words_count_the_selected_source_and_explain_unknowns() {
        let known = sourced_indices(
            0.0,
            &[
                row(Position::CH, 0.2, BeatErrorSource::Unlock),
                row(Position::CB, 0.8, BeatErrorSource::Drop),
                row(Position::H3, 0.4, BeatErrorSource::Unlock),
            ],
        );
        assert_eq!(
            known.beat_error_summary(),
            "Beat Error from the Unlock: mean of 2 of 3 positions."
        );
        let unknown = sourced_indices(0.0, &[(Position::CH, None, None, Some(0.4), None)]);
        assert!(unknown.beat_error_summary().contains("source not recorded"));
        assert!(unknown.beat_error_summary().contains("no mean"));
        assert!(unknown
            .beat_error_summary()
            .contains("leaves out 1 reading whose"));
    }
}
