//! Sending a watch's positions to the ERP: sign each, attach the operator's
//! sign-in, chain them under one ERP session, then complete it.

use std::time::SystemTime;

use serde_json::{json, Value};

use crate::canonical;
use crate::client::{Client, CompleteReply, RecordingReply, SubmitReply};
use crate::operator::{self, OperatorSession};
use crate::payload::{
    Context, Enhanced, PositionResult, CAPABILITY_ENHANCED, CAPABILITY_RECORDING,
};
use crate::signing::Signer;

/// What happened to one position.
#[derive(Debug)]
pub struct Sent {
    pub position: &'static str,
    pub outcome: Result<SubmitReply, String>,
    /// The enhanced block went with this result and the ERP kept it.
    pub enhanced_kept: bool,
    /// The recording's size when the ERP took it, or why not; none when this
    /// position sent no recording.
    pub recording: Option<Result<usize, String>>,
}

#[derive(Debug, Default)]
pub struct Report {
    pub sent: Vec<Sent>,
    /// The ERP session the positions were chained under.
    pub session: Option<String>,
    /// The ERP's answer to completing the session; none if nothing was sent.
    pub completed: Option<Result<CompleteReply, String>>,
    /// The ERP did not accept the operator's sign-in on some result; it is
    /// dropped and the operator must sign in again.
    pub signin_rejected: bool,
    /// An operator was signed in but their sign-in was past the station's
    /// limits, so no result carried it.
    pub signin_stale: bool,
    /// Enhanced data was asked for and the ERP cannot take it (or all of
    /// it): what was sent instead, in words.
    pub enhanced_declined: Option<String>,
}

impl Report {
    pub fn accepted(&self) -> usize {
        self.sent.iter().filter(|s| s.outcome.is_ok()).count()
    }
}

/// The request body: the signed fields, then what travels unsigned.
/// Written with the same number formatting as the signed bytes, so what the
/// ERP parses from it gives back the bytes that were signed.
pub fn wire_body(
    signed: &Value,
    signature: Option<Value>,
    session: Option<&str>,
    operator_token: Option<&str>,
    enhanced: Option<&Value>,
) -> Vec<u8> {
    let mut body = signed.clone();
    if let Some(block) = enhanced {
        body["enhanced"] = block.clone();
    }
    if let Some(entry) = signature {
        body["signatures"] = json!([entry]);
    }
    if let Some(session) = session {
        body["session"] = json!(session);
    }
    if let Some(token) = operator_token {
        body["operator_session"] = json!(token);
    }
    canonical::to_bytes(&body)
}

/// The station's signature over the signed fields.
pub fn signature_entry(signer: &Signer, signed: &Value) -> Value {
    json!({
        "key_id": signer.key_id(),
        "signer_type": "Workstation",
        "signature_b64": signer.sign_b64(&canonical::to_bytes(signed)),
    })
}

/// Sends every position, one result each, and completes the session.
///
/// A position that fails does not stop the others. `operator` is dropped if
/// the ERP rejects it. `clock` is the time, passed in so the sign-in's limits
/// can be tested.
pub fn send_session(
    client: &Client,
    signer: Option<&Signer>,
    ctx: &Context,
    operator: &mut Option<OperatorSession>,
    results: &[PositionResult],
    clock: impl Fn() -> SystemTime,
) -> Report {
    let mut report = Report::default();
    let (enhanced, upload) = enhanced_allowed(client, results, &mut report);
    for result in results {
        let now = clock();
        let token = usable_token(operator, now, &mut report);
        let signed = result.signed_fields_with(ctx, enhanced);
        let signature = signer.map(|s| signature_entry(s, &signed));
        let block = result
            .enhanced
            .as_ref()
            .filter(|_| enhanced)
            .map(|e| &e.block);
        let body = wire_body(
            &signed,
            signature,
            report.session.as_deref(),
            token.as_deref(),
            block,
        );
        let outcome = client
            .submit_timing_result(&body)
            .map_err(|e| e.to_string());
        if let Ok(reply) = &outcome {
            note_reply(reply, token.is_some(), operator, now, &mut report);
        }
        let recording = match (&outcome, result.enhanced.as_ref()) {
            (Ok(reply), Some(e)) if enhanced && upload => send_recording(client, reply, e),
            _ => None,
        };
        report.sent.push(Sent {
            position: result.position,
            enhanced_kept: matches!(&outcome, Ok(r) if enhanced && r.enhanced_kept == Some(true)),
            outcome,
            recording,
        });
    }
    report.completed = report
        .session
        .clone()
        .filter(|_| report.accepted() > 0)
        .map(|session| client.complete_session(&session).map_err(|e| e.to_string()));
    report
}

/// Whether the results go as enhanced, and whether their recordings are
/// sent: only if the ERP says it takes them (`ping` lists what it can). An
/// ERP that does not is sent the basic results, which it can verify, and the
/// report says so.
fn enhanced_allowed(
    client: &Client,
    results: &[PositionResult],
    report: &mut Report,
) -> (bool, bool) {
    if results.iter().all(|r| r.enhanced.is_none()) {
        return (false, false);
    }
    let can = client.capabilities().unwrap_or_default();
    let has = |c: &str| can.iter().any(|x| x == c);
    if !has(CAPABILITY_ENHANCED) {
        report.enhanced_declined = Some(
            "This ERP does not take enhanced data yet, so the basic results were sent.".into(),
        );
        return (false, false);
    }
    let any_recording = results
        .iter()
        .any(|r| r.enhanced.as_ref().is_some_and(|e| e.recording.is_some()));
    if any_recording && !has(CAPABILITY_RECORDING) {
        report.enhanced_declined = Some(
            "This ERP does not take recordings yet: the enhanced data was sent without them."
                .into(),
        );
    }
    (true, has(CAPABILITY_RECORDING))
}

/// Sends a position's recording if the ERP asked for the one the result
/// announced.
fn send_recording(
    client: &Client,
    reply: &SubmitReply,
    enhanced: &Enhanced,
) -> Option<Result<usize, String>> {
    let recording = enhanced.recording.as_ref()?;
    if reply.recording_expected != Some(true) {
        return Some(Err(
            "the ERP did not ask for the recording (it did not accept the result as enhanced)"
                .into(),
        ));
    }
    let id = reply.result_id.as_deref().unwrap_or("");
    let sent = client
        .submit_recording(id, &recording.name, &recording.flac)
        .map_err(|e| e.to_string())
        .and_then(|r: RecordingReply| {
            if r.ok {
                Ok(recording.flac.len())
            } else {
                Err(r.reason.unwrap_or_else(|| "the ERP refused it".into()))
            }
        });
    Some(sent)
}

/// The operator's token if their sign-in is still within the limits.
fn usable_token(
    operator: &Option<OperatorSession>,
    now: SystemTime,
    report: &mut Report,
) -> Option<String> {
    let session = operator.as_ref()?;
    if session.is_usable(now) {
        Some(session.token().to_string())
    } else {
        report.signin_stale = true;
        None
    }
}

fn note_reply(
    reply: &SubmitReply,
    sent_token: bool,
    operator: &mut Option<OperatorSession>,
    now: SystemTime,
    report: &mut Report,
) {
    if report.session.is_none() {
        report.session = reply.session.clone();
    }
    if !sent_token {
        return;
    }
    let message = reply.verification_message.as_deref().unwrap_or("");
    if operator::message_rejects_signin(message) {
        *operator = None;
        report.signin_rejected = true;
    } else if let Some(session) = operator {
        session.used(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(session: Option<&str>, message: &str) -> SubmitReply {
        SubmitReply {
            session: session.map(str::to_string),
            verification_message: Some(message.to_string()),
            ..SubmitReply::default()
        }
    }

    fn operator(now: SystemTime) -> Option<OperatorSession> {
        Some(OperatorSession::new(
            "tok".into(),
            "OP1".into(),
            "Op".into(),
            now,
        ))
    }

    #[test]
    fn the_first_answer_gives_the_session_the_rest_are_chained_under() {
        let mut report = Report::default();
        let mut op = None;
        let now = SystemTime::UNIX_EPOCH;
        note_reply(&reply(Some("S-1"), "x"), false, &mut op, now, &mut report);
        note_reply(&reply(Some("S-2"), "x"), false, &mut op, now, &mut report);
        assert_eq!(report.session.as_deref(), Some("S-1"));
    }

    #[test]
    fn a_rejected_sign_in_is_dropped_and_noted() {
        let now = SystemTime::UNIX_EPOCH;
        let mut op = operator(now);
        let mut report = Report::default();
        note_reply(
            &reply(
                Some("S"),
                "Operator sign-in: The operator sign-in has expired. Sign in again.",
            ),
            true,
            &mut op,
            now,
            &mut report,
        );
        assert!(op.is_none());
        assert!(report.signin_rejected);
    }

    #[test]
    fn an_accepted_sign_in_counts_as_used() {
        let start = SystemTime::UNIX_EPOCH;
        let later = start + std::time::Duration::from_secs(20 * 60);
        let mut op = operator(start);
        let mut report = Report::default();
        note_reply(
            &reply(Some("S"), "attested"),
            true,
            &mut op,
            later,
            &mut report,
        );
        assert!(!report.signin_rejected);
        // used at 20 min, so still usable at 40 min though 40 > the 25 min idle limit
        let session = op.unwrap();
        assert!(session.is_usable(start + std::time::Duration::from_secs(40 * 60)));
    }

    #[test]
    fn a_stale_sign_in_is_not_sent_and_is_noted() {
        let start = SystemTime::UNIX_EPOCH;
        let op = operator(start);
        let mut report = Report::default();
        let late = start + std::time::Duration::from_secs(26 * 60);
        assert!(usable_token(&op, late, &mut report).is_none());
        assert!(report.signin_stale);
        assert_eq!(
            usable_token(&op, start, &mut Report::default()).as_deref(),
            Some("tok")
        );
    }

    #[test]
    fn the_wire_body_reads_back_to_the_bytes_that_were_signed() {
        let signed = json!({"b": 1, "a": {"x": 12.345679, "y": 20.0, "z": 1e-5}});
        let body = wire_body(
            &signed,
            Some(json!({"key_id": "k"})),
            Some("S-1"),
            Some("tok"),
            None,
        );
        let mut parsed: Value = serde_json::from_slice(&body).unwrap();
        for key in ["signatures", "session", "operator_session"] {
            assert!(
                parsed.as_object_mut().unwrap().remove(key).is_some(),
                "{key}"
            );
        }
        assert_eq!(canonical::to_bytes(&parsed), canonical::to_bytes(&signed));
    }
}
