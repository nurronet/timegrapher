//! Enhanced data for the ERP: beside each position's figures, core's whole
//! measurement of it (every caveat, the beat error from both events, the
//! windows' spread, the two states, the clock) with the stretches read and
//! the Positions Test's values, and a lossless recording of the position's
//! sound. Switched on in Settings > ERP.
//!
//! Nothing here measures anything: the block is core's `Measurement` and
//! `StateIndices` as JSON, and the recording is the take's own samples cut
//! with `excerpt::cut` and written as FLAC, as the bug report writes its
//! excerpt. `timegrapher-erp` signs the block's digest and the recording's,
//! and sends them.

use super::*;
use serde_json::{json, Value};
use timegrapher_core::excerpt;
use timegrapher_erp::client::MAX_RECORDING_BYTES;
use timegrapher_erp::payload::{Enhanced, PositionResult, Recording};

/// Version 1 of the block this writes (`enhanced_version`).
const BLOCK_VERSION: u64 = 1;

/// What the enhanced data is made from: the Positions Test as it stands, the
/// take's sound files, and the settings the readings were made with.
pub(super) struct EnhancedInput {
    pub(super) table: guided::FreeTable,
    pub(super) files: Vec<PathBuf>,
    pub(super) about: Value,
}

/// Gives each result in `results` its enhanced data, and says what had to be
/// left out: a position without its detail, or a recording that could not be
/// cut or is over what the ERP takes. A result keeps its figures whatever
/// happens here.
pub(super) fn attach(results: &mut [PositionResult], input: &EnhancedInput) -> Vec<String> {
    let mut notes = Vec::new();
    for result in results.iter_mut() {
        let Some((position, row, detail)) = row_of(&input.table, result.position) else {
            notes.push(format!(
                "{}: no enhanced data, the table holds none for it",
                result.position
            ));
            continue;
        };
        let recording = if input.files.is_empty() {
            notes.push(format!(
                "{}: no recording, the take has no sound file to cut from",
                result.position
            ));
            None
        } else {
            match cut_recording(&input.files, position, &detail.spans) {
                Ok(r) => Some(r),
                Err(why) => {
                    notes.push(format!("{}: no recording. {why}", result.position));
                    None
                }
            }
        };
        result.enhanced = Some(Enhanced {
            block: block(input, position, row, detail),
            recording,
        });
    }
    notes
}

/// The table's row, its detail and core's position for the ERP's name of one.
fn row_of<'a>(
    table: &'a guided::FreeTable,
    name: &str,
) -> Option<(core_session::Position, f64, &'a guided::RowDetail)> {
    let position = core_session::Position::ALL
        .iter()
        .copied()
        .find(|p| p.name() == name)?;
    let label = guided::position_label(position);
    let at = table.rows.iter().position(|r| r.0 == label)?;
    Some((position, table.rows[at].1, table.details.get(at)?))
}

/// The block for one position.
fn block(
    input: &EnhancedInput,
    position: core_session::Position,
    duration_s: f64,
    detail: &guided::RowDetail,
) -> Value {
    json!({
        "version": BLOCK_VERSION,
        "software": {"name": "timegrapher", "version": env!("CARGO_PKG_VERSION")},
        "position": {
            "code": position.code(),
            "name": position.name(),
            "duration_s": duration_s,
            "spans": detail.spans.iter().map(|s| json!([s.0, s.1])).collect::<Vec<_>>(),
        },
        "measurement": detail.measurement,
        "settings": input.about,
        "positions_test": {"positions_read": input.table.read, "indices": input.table.indices},
    })
}

/// The sound from the start of the position's first stretch to the end of
/// its last, cut from the take's files without changing a sample.
fn cut_recording(
    files: &[PathBuf],
    position: core_session::Position,
    spans: &[(f64, f64)],
) -> Result<Recording, String> {
    let from = spans.iter().map(|s| s.0).fold(f64::INFINITY, f64::min);
    let to = spans.iter().map(|s| s.1).fold(f64::NEG_INFINITY, f64::max);
    if !(from.is_finite() && to.is_finite() && to > from) {
        return Err("No time is marked in this position.".into());
    }
    let pcm = excerpt::cut(files, from, to - from).map_err(|e| e.to_string())?;
    let flac = excerpt::encode_flac(&pcm).map_err(|e| e.to_string())?;
    if flac.len() > MAX_RECORDING_BYTES {
        return Err(format!(
            "It is {} MB, over the {} MB the ERP takes.",
            flac.len() / (1024 * 1024),
            MAX_RECORDING_BYTES / (1024 * 1024)
        ));
    }
    Ok(Recording {
        flac,
        name: format!("{}.flac", position.name()),
        from_s: from,
        duration_s: pcm.duration_s(),
        sample_rate: pcm.sample_rate,
        channels: pcm.channels,
        bits_per_sample: pcm.bits_per_sample,
    })
}

impl TimegrapherApp {
    /// What enhanced data would be made from now, when it is switched on
    /// and there is a Positions Test to take it from.
    pub(super) fn erp_enhanced_input(&mut self, ctx: &egui::Context) -> Option<EnhancedInput> {
        if !self.erp.config.enhanced {
            return None;
        }
        let table = self.free_table_shown(ctx)?;
        let files = self.report_source().map(|s| s.files).unwrap_or_default();
        let about = json!({
            "bph": self.bph,
            "lift_deg": self.lift_known().then_some(self.lift_deg),
            "calibre": self.calibre,
        });
        Some(EnhancedInput {
            table,
            files,
            about,
        })
    }

    /// Sends the Positions Test now, with its enhanced data if that is
    /// switched on.
    pub(super) fn erp_send_now(&mut self, ctx: &egui::Context, results: Vec<PositionResult>) {
        let enhanced = self.erp_enhanced_input(ctx);
        self.erp.send_with(ctx, results, enhanced);
    }
}
