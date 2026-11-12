//! A stand-in LN ERP, over real HTTP on this computer, for tests: this
//! crate's own and the desktop app's. Built only with the `testing` feature.
//!
//! It checks what the real ERP checks: the API key header, the operator's
//! PIN and sign-in token, and the station's Ed25519 signature over the bytes
//! it rebuilds from the body it receives (the signed fields, less the four
//! that travel unsigned). A signature that does not survive the trip
//! through the wire fails here as it would there.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use crate::canonical;
use crate::payload::sha256_hex;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use ring::signature::{UnparsedPublicKey, ED25519};
use serde_json::{json, Value};

pub const KEY: &str = "KEY123";
pub const SECRET: &str = "SECRET456";
pub const PIN: &str = "483920";
pub const TOKEN: &str = "TOKEN-abc-123";
const WIRE_ONLY: [&str; 5] = [
    "signatures",
    "raw_samples",
    "session",
    "operator_session",
    "enhanced",
];

#[derive(Default)]
pub struct State {
    pub keys: HashMap<String, Vec<u8>>,
    /// (method name, Authorization header, body) of every request.
    pub log: Vec<(String, String, String)>,
    /// The token the stand-in treats as ended.
    pub ended_token: bool,
    /// Where `ping` sends the caller on, when set.
    pub redirect_ping_to: Option<String>,
    pub sessions: u32,
    /// An ERP from before enhanced data: its `ping` lists no capabilities.
    pub old_erp: bool,
    /// An ERP that reads enhanced results but not yet recordings.
    pub no_recordings: bool,
    /// Every result body received, as parsed (less the sign-in token).
    pub results: Vec<Value>,
    /// The enhanced blocks kept, by result id.
    pub enhanced: Vec<(String, Value)>,
    /// The recordings kept, by result id.
    pub recordings: Vec<(String, Vec<u8>)>,
}

pub struct Erp {
    pub url: String,
    pub state: Arc<Mutex<State>>,
}

fn header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

fn read_request(stream: &mut TcpStream) -> Option<(String, String, String)> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let end = loop {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(end) = header_end(&buf) {
            break end;
        }
    };
    let head = String::from_utf8_lossy(&buf[..end]).to_string();
    let length = head
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().parse::<usize>().unwrap_or(0))
        })
        .unwrap_or(0);
    while buf.len() < end + length {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let path = head.lines().next()?.split_whitespace().nth(1)?.to_string();
    let auth = head
        .lines()
        .find_map(|l| {
            l.strip_prefix("Authorization: ")
                .or_else(|| l.strip_prefix("authorization: "))
        })
        .unwrap_or("")
        .to_string();
    let body = String::from_utf8_lossy(&buf[end..end + length.min(buf.len() - end)]).to_string();
    Some((path, auth, body))
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let text = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(text.as_bytes());
}

/// Does the stand-in accept this signed body? Returns (status, message).
fn assess(state: &State, body: &Value) -> (&'static str, String) {
    let mut payload = body.clone();
    let token = payload.as_object_mut().unwrap().remove("operator_session");
    let signatures = payload
        .get("signatures")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if signatures.is_empty() {
        return ("Unsigned", String::new());
    }
    let mut signed = payload.clone();
    for key in WIRE_ONLY {
        signed.as_object_mut().unwrap().remove(key);
    }
    // the bytes the ERP rebuilds from what it parsed
    let bytes = canonical::to_bytes(&signed);
    let entry = &signatures[0];
    let Some(public) = state.keys.get(entry["key_id"].as_str().unwrap_or("")) else {
        return ("Invalid", "Workstation: unknown signing key".into());
    };
    let signature = STANDARD
        .decode(entry["signature_b64"].as_str().unwrap_or(""))
        .unwrap_or_default();
    if UnparsedPublicKey::new(&ED25519, public)
        .verify(&bytes, &signature)
        .is_err()
    {
        return ("Invalid", "Workstation: Invalid Ed25519 signature".into());
    }
    match token.as_ref().and_then(Value::as_str) {
        None => (
            "Invalid",
            "One or more required signatures failed verification".into(),
        ),
        Some(_) if state.ended_token => (
            "Invalid",
            "Operator sign-in: The operator sign-in has expired. Sign in again.".into(),
        ),
        Some(t) if t == TOKEN && body["operator_id"] == "OP1" => (
            "Attested",
            "Station signature verified; operator OP1 attested by the ERP after PIN sign-in".into(),
        ),
        Some(_) => (
            "Invalid",
            "Operator sign-in: The operator sign-in was not found. Sign in again.".into(),
        ),
    }
}

fn answer(state: &mut State, name: &str, auth: &str, body: &str) -> (&'static str, Value) {
    if auth != format!("token {KEY}:{SECRET}") {
        return ("403 Forbidden", json!({"exc_type": "AuthenticationError"}));
    }
    let parsed: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let message = match name {
        "ping" if state.old_erp => {
            json!({"ok": true, "user": "station@example.test", "server_time": "t", "api_version": "1.4.1"})
        }
        "ping" => {
            let mut can = vec!["enhanced_results_v1"];
            if !state.no_recordings {
                can.push("recording_upload_v1");
            }
            json!({"ok": true, "user": "station@example.test", "server_time": "t", "api_version": "1.5.0",
                   "capabilities": can})
        }
        "register_key" => {
            let id = parsed["key_id"].as_str().unwrap_or("").to_string();
            if state.keys.contains_key(&id) {
                return (
                    "417 Expectation Failed",
                    json!({"exception": "frappe.exceptions.ValidationError: Key ID already exists"}),
                );
            }
            state.keys.insert(
                id.clone(),
                STANDARD
                    .decode(parsed["public_key_b64"].as_str().unwrap_or(""))
                    .unwrap(),
            );
            json!({"key_id": id, "status": "Active"})
        }
        "operator_login" => login(&parsed),
        "operator_logout" => json!({"ok": true}),
        "submit_timing_result" => submit(state, parsed),
        "submit_recording" => recording(state, &parsed),
        "complete_measurement_session" => json!({"session": parsed["session"],
            "suggested_functional_grade": "B - Serviceable", "suggested_grade_reason": "Rates within limits"}),
        _ => return ("404 Not Found", json!({})),
    };
    ("200 OK", json!({ "message": message }))
}

/// A result: judged as the real ERP does, its enhanced block checked against
/// the digest the station signed, and the block kept only if it matches.
fn submit(state: &mut State, parsed: Value) -> Value {
    let (mut status, mut message) = assess(state, &parsed);
    let id = format!("R{}", state.results.len() + 1);
    let mut kept = false;
    if let Some(block) = parsed.get("enhanced") {
        let digest = sha256_hex(&canonical::to_bytes(block));
        if parsed["enhanced_sha256"].as_str() == Some(digest.as_str()) {
            state.enhanced.push((id.clone(), block.clone()));
            kept = true;
        } else {
            status = "Invalid";
            message = "Enhanced data: does not match the digest the station signed".into();
        }
    }
    let session = parsed["session"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| {
            state.sessions += 1;
            format!("SESSION-{}", state.sessions)
        });
    let expected = parsed["recording"]["sha256"].is_string();
    let mut stored = parsed;
    stored.as_object_mut().unwrap().remove("operator_session");
    state.results.push(stored);
    json!({"ok": true, "session": session, "result_id": id, "pass_fail": "Pass",
           "verification_status": status, "verification_message": message,
           "enhanced_kept": kept, "recording_expected": expected})
}

/// A recording: kept only if it is the size and SHA-256 the result signed.
fn recording(state: &mut State, request: &Value) -> Value {
    let id = request["result_id"].as_str().unwrap_or("");
    let Some(result) = state
        .results
        .iter()
        .enumerate()
        .find_map(|(i, r)| (format!("R{}", i + 1) == id).then_some(r))
    else {
        return json!({"ok": false, "reason": "No such result"});
    };
    let expected = result["recording"].clone();
    let bytes = STANDARD
        .decode(request["content_b64"].as_str().unwrap_or(""))
        .unwrap_or_default();
    if expected["sha256"].as_str() != Some(sha256_hex(&bytes).as_str())
        || expected["bytes"].as_u64() != Some(bytes.len() as u64)
    {
        return json!({"ok": false, "reason": "The recording does not match the size and digest the station signed"});
    }
    state.recordings.push((id.to_string(), bytes));
    json!({"ok": true, "already": false})
}

fn login(request: &Value) -> Value {
    if request["operator_id"] == "OP1"
        && request["pin"] == PIN
        && request["station_id"] == "STATION-01"
    {
        json!({"ok": true, "operator_id": "OP1", "operator_name": "Op One", "session": TOKEN, "expires_at": "x"})
    } else {
        json!({"ok": false, "reason": "The Operator ID or PIN is wrong."})
    }
}

pub fn start_erp() -> Erp {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let state = Arc::new(Mutex::new(State::default()));
    let shared = Arc::clone(&state);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let Some((path, auth, body)) = read_request(&mut stream) else {
                continue;
            };
            let (route, query) = path.split_once('?').unwrap_or((&path, ""));
            let name = route.rsplit('.').next().unwrap_or("").to_string();
            let mut state = shared.lock().unwrap();
            state
                .log
                .push((name.clone(), auth.clone(), format!("{body}{query}")));
            if name == "ping" {
                if let Some(to) = state.redirect_ping_to.clone() {
                    let text = format!("HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    let _ = stream.write_all(text.as_bytes());
                    continue;
                }
            }
            // a GET carries its question in the path, so lookup is answered here
            let (status, mut reply) = if name == "search_identities" {
                let items = if query.contains("query=ID") {
                    json!([{"identity_id": "ID-1", "display": "ID-1  |  ETA 2824-2  |  Movement",
                            "identity_type": "Movement", "ln_part": "ETA 2824-2"}])
                } else {
                    json!([])
                };
                ("200 OK", json!({"message": {"items": items}}))
            } else if name == "lookup_identity" {
                (
                    "200 OK",
                    json!({"message": if query.contains("identity_id=ID-1") {
                        json!({"found": true, "identity_id": "ID-1", "ln_part": "ETA 2824-2",
                               "recommended_measurement_type": "Follow-up Certification",
                               "recommended_profile": "Workshop"})
                    } else {
                        json!({"found": false, "identity_id": "X", "suggestions": [{"display": "ID-1  |  ETA 2824-2"}]})
                    }}),
                )
            } else {
                answer(&mut state, &name, &auth, &body)
            };
            if reply.is_null() {
                reply = json!({});
            }
            respond(&mut stream, status, &reply.to_string());
        }
    });
    Erp { url, state }
}
