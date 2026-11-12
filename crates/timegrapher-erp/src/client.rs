//! The calls the station makes to the ERP.
//!
//! Every call is `<address>/api/method/ln_watch_inventory.<module>.<name>`
//! with `Authorization: token <key>:<secret>`. A Frappe method wraps what it
//! returns as `{"message": ...}`; that value is what these functions give.
//! A refusal the ERP expects (a wrong PIN, a locked operator) comes back
//! as `{"ok": false, "reason": ...}` with HTTP 200 and is not an error here.
//! Redirects are not followed, so the key and secret are never forwarded.

use std::time::{Duration, SystemTime};

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ureq::tls::{RootCerts, TlsConfig};
use zeroize::Zeroize;

use crate::config::Config;
use crate::operator::OperatorSession;

const FREEERP: &str = "ln_watch_inventory.freeerp";
const CUSTODY: &str = "ln_watch_inventory.custody";
const CONNECT: Duration = Duration::from_secs(8);
const TOTAL: Duration = Duration::from_secs(20);
/// A recording is megabytes, over whatever link the station has.
const UPLOAD_TOTAL: Duration = Duration::from_secs(180);
/// The most one position's recording may be: what the ERP takes.
pub const MAX_RECORDING_BYTES: usize = 24 * 1024 * 1024;
/// How much of an error page is kept to show.
const ERROR_BODY_CHARS: usize = 500;

#[derive(Debug)]
pub enum ErpError {
    /// The settings are not usable.
    Config(String),
    /// The ERP could not be reached.
    Network(String),
    /// The ERP answered with an error status.
    Status { code: u16, body: String },
    /// The ERP answered, but not in a way this understands.
    Reply(String),
}

impl std::fmt::Display for ErpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErpError::Config(why) => f.write_str(why),
            ErpError::Network(why) => write!(f, "could not reach the ERP: {why}"),
            ErpError::Status {
                code: 401 | 403, ..
            } => f.write_str(
                "the ERP did not accept this station's API key; an administrator needs to check it",
            ),
            ErpError::Status { code, body } => write!(f, "the ERP answered HTTP {code}: {body}"),
            ErpError::Reply(why) => write!(f, "the ERP's answer was not understood: {why}"),
        }
    }
}

impl std::error::Error for ErpError {}

/// What `lookup_identity` knows about what is on the bench.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Identity {
    #[serde(default)]
    pub found: bool,
    pub identity_id: Option<String>,
    pub movement_serial: Option<String>,
    pub ln_part: Option<String>,
    pub item: Option<String>,
    pub current_watch_unit: Option<String>,
    pub current_work_order: Option<String>,
    pub lifecycle_status: Option<String>,
    pub functional_grade: Option<String>,
    pub qa_status: Option<String>,
    pub recommended_measurement_type: Option<String>,
    pub recommended_profile: Option<String>,
    /// The ERP's own one-line label: `serial  |  part  |  type`.
    pub display: Option<String>,
    pub identity_type: Option<String>,
    /// When not found: what the query was close to.
    #[serde(default)]
    pub suggestions: Vec<Value>,
}

/// The ERP's answer to one submitted result.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SubmitReply {
    pub session: Option<String>,
    pub result_id: Option<String>,
    pub pass_fail: Option<String>,
    pub failure_reason: Option<String>,
    pub verification_status: Option<String>,
    pub verification_message: Option<String>,
    /// Enhanced results only: the ERP kept the enhanced block, and wants the
    /// recording the result announced.
    pub enhanced_kept: Option<bool>,
    pub recording_expected: Option<bool>,
}

/// The ERP's answer to a recording.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RecordingReply {
    #[serde(default)]
    pub ok: bool,
    pub reason: Option<String>,
    #[serde(default)]
    pub already: bool,
}

/// The ERP's answer to a completed session.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CompleteReply {
    pub measurement_type: Option<String>,
    pub suggested_functional_grade: Option<String>,
    pub suggested_grade_reason: Option<String>,
}

/// How an operator's sign-in went.
#[derive(Debug)]
pub enum LoginOutcome {
    SignedIn(OperatorSession),
    /// The ERP's own words, to show as they are.
    Refused(String),
}

pub struct Client {
    agent: ureq::Agent,
    base: String,
    auth: String,
    station_id: String,
}

impl Client {
    pub fn new(config: &Config) -> Result<Client, ErpError> {
        config.check().map_err(ErpError::Config)?;
        // The system's own certificate store, as the update check uses, so an
        // office network that inspects HTTPS with its own authority works.
        let tls = TlsConfig::builder()
            .root_certs(RootCerts::PlatformVerifier)
            .disable_verification(!config.verify_tls)
            .build();
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .tls_config(tls)
            .timeout_connect(Some(CONNECT))
            .timeout_global(Some(TOTAL))
            .http_status_as_error(false)
            .max_redirects(0)
            .user_agent(format!("timegrapher-erp/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        Ok(Client {
            agent,
            base: config.base_url.clone(),
            auth: format!("token {}:{}", config.api_key, config.api_secret),
            station_id: config.station_id.clone(),
        })
    }

    fn url(&self, module: &str, name: &str) -> String {
        format!("{}/api/method/{module}.{name}", self.base)
    }

    fn get(&self, name: &str, query: &[(&str, &str)]) -> Result<Value, ErpError> {
        let mut request = self
            .agent
            .get(self.url(FREEERP, name))
            .header("Authorization", &self.auth)
            .header("Accept", "application/json");
        for (key, value) in query {
            request = request.query(key, value);
        }
        finish(request.call())
    }

    fn post(&self, module: &str, name: &str, body: &[u8]) -> Result<Value, ErpError> {
        let request = self
            .agent
            .post(self.url(module, name))
            .header("Authorization", &self.auth)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json");
        finish(request.send(body))
    }

    /// Checks the address, the key and the secret.
    pub fn ping(&self) -> Result<Value, ErpError> {
        self.get(
            "ping",
            &[
                ("station_id", &self.station_id),
                ("software_version", env!("CARGO_PKG_VERSION")),
            ],
        )
    }

    /// What the ERP says it can take beyond a basic result: the
    /// `capabilities` of its `ping` (none from an ERP that predates them).
    pub fn capabilities(&self) -> Result<Vec<String>, ErpError> {
        let reply = self.ping()?;
        Ok(reply["capabilities"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|c| c.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Suggestions for what is typed or scanned: `{"items": [...]}`.
    pub fn search_identities(&self, query: &str, limit: u32) -> Result<Value, ErpError> {
        self.get(
            "search_identities",
            &[
                ("query", query),
                ("limit", &limit.to_string()),
                ("station_id", &self.station_id),
            ],
        )
    }

    /// What matches `query`, exact matches first: movements and watch units.
    /// Each has [`Identity::found`] set, and the fields a search gives.
    pub fn search(&self, query: &str, limit: u32) -> Result<Vec<Identity>, ErpError> {
        let reply = self.search_identities(query, limit)?;
        let items = reply["items"].as_array().cloned().unwrap_or_default();
        items
            .into_iter()
            .map(|mut item| {
                item["found"] = Value::Bool(true);
                serde_json::from_value(item).map_err(|e| ErpError::Reply(e.to_string()))
            })
            .collect()
    }

    /// An exact lookup. Check [`Identity::found`] before using the rest.
    pub fn lookup_identity(&self, identity_id: &str) -> Result<Identity, ErpError> {
        let reply = self.get(
            "lookup_identity",
            &[
                ("identity_id", identity_id),
                ("station_id", &self.station_id),
            ],
        )?;
        serde_json::from_value(reply).map_err(|e| ErpError::Reply(e.to_string()))
    }

    /// Tells the ERP the station's public key. The ERP refuses a second
    /// registration of a key ID, so call it only for a key just made.
    pub fn register_key(&self, key_id: &str, public_key_b64: &str) -> Result<Value, ErpError> {
        let body = serde_json::json!({
            "key_id": key_id,
            "owner_type": "Workstation",
            "public_key_b64": public_key_b64,
        });
        self.post(CUSTODY, "register_key", body.to_string().as_bytes())
    }

    /// Signs an operator in. The PIN is wiped from the request as soon as it
    /// has been sent.
    pub fn operator_login(
        &self,
        operator_id: &str,
        pin: &str,
        now: SystemTime,
    ) -> Result<LoginOutcome, ErpError> {
        #[derive(Serialize)]
        struct Request<'a> {
            operator_id: &'a str,
            pin: &'a str,
            station_id: &'a str,
        }
        let mut body = serde_json::to_vec(&Request {
            operator_id,
            pin,
            station_id: &self.station_id,
        })
        .map_err(|e| ErpError::Reply(e.to_string()))?;
        let reply = self.post(FREEERP, "operator_login", &body);
        body.zeroize();
        parse_login(&reply?, now)
    }

    /// Ends the sign-in. Best effort: the token is dropped either way.
    pub fn operator_logout(&self, session: &OperatorSession) -> bool {
        let mut body = serde_json::json!({ "session": session.token() })
            .to_string()
            .into_bytes();
        let done = self.post(FREEERP, "operator_logout", &body).is_ok();
        body.zeroize();
        done
    }

    /// Sends one result: the body is built and signed by the caller.
    pub fn submit_timing_result(&self, body: &[u8]) -> Result<SubmitReply, ErpError> {
        let reply = self.post(FREEERP, "submit_timing_result", body)?;
        serde_json::from_value(reply).map_err(|e| ErpError::Reply(e.to_string()))
    }

    /// Sends the recording of a position already sent. The ERP keeps it only
    /// if its size and SHA-256 are those the result's signature covers.
    pub fn submit_recording(
        &self,
        result_id: &str,
        filename: &str,
        flac: &[u8],
    ) -> Result<RecordingReply, ErpError> {
        if flac.len() > MAX_RECORDING_BYTES {
            return Err(ErpError::Config(format!(
                "the recording is {} MB, over the {} MB the ERP takes",
                flac.len() / (1024 * 1024),
                MAX_RECORDING_BYTES / (1024 * 1024)
            )));
        }
        let body = serde_json::json!({
            "result_id": result_id,
            "filename": filename,
            "content_b64": base64::engine::general_purpose::STANDARD.encode(flac),
        });
        let request = self
            .agent
            .post(self.url(FREEERP, "submit_recording"))
            .config()
            .timeout_global(Some(UPLOAD_TOTAL))
            .build()
            .header("Authorization", &self.auth)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json");
        let reply = finish(request.send(body.to_string().as_bytes()))?;
        serde_json::from_value(reply).map_err(|e| ErpError::Reply(e.to_string()))
    }

    /// Tells the ERP every position of a session is in.
    pub fn complete_session(&self, session: &str) -> Result<CompleteReply, ErpError> {
        let body = serde_json::json!({ "session": session });
        let reply = self.post(
            FREEERP,
            "complete_measurement_session",
            body.to_string().as_bytes(),
        )?;
        serde_json::from_value(reply).map_err(|e| ErpError::Reply(e.to_string()))
    }
}

/// Reads the ERP's answer to `operator_login`.
pub(crate) fn parse_login(reply: &Value, now: SystemTime) -> Result<LoginOutcome, ErpError> {
    let text = |key: &str| reply[key].as_str().map(str::to_string);
    match reply["ok"].as_bool() {
        Some(true) => {
            let token = text("session")
                .filter(|t| !t.is_empty())
                .ok_or_else(|| ErpError::Reply("a sign-in with no session".into()))?;
            Ok(LoginOutcome::SignedIn(OperatorSession::new(
                token,
                text("operator_id").unwrap_or_default(),
                text("operator_name").unwrap_or_default(),
                now,
            )))
        }
        Some(false) => Ok(LoginOutcome::Refused(
            text("reason").unwrap_or_else(|| "The ERP refused the sign-in.".into()),
        )),
        None => Err(ErpError::Reply("a sign-in answer with no ok".into())),
    }
}

fn finish(sent: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<Value, ErpError> {
    let mut response = sent.map_err(|e| ErpError::Network(e.to_string()))?;
    let code = response.status().as_u16();
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| ErpError::Network(e.to_string()))?;
    if !(200..300).contains(&code) {
        let body: String = text.chars().take(ERROR_BODY_CHARS).collect();
        return Err(ErpError::Status { code, body });
    }
    let doc: Value = serde_json::from_str(&text).map_err(|e| ErpError::Reply(e.to_string()))?;
    doc.get("message")
        .cloned()
        .ok_or_else(|| ErpError::Reply("no message in the answer".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000)
    }

    #[test]
    fn a_sign_in_gives_a_session_that_holds_the_token() {
        let reply = json!({"ok": true, "operator_id": "OP1", "operator_name": "Op One",
                           "session": "tok", "expires_at": "x"});
        match parse_login(&reply, now()).unwrap() {
            LoginOutcome::SignedIn(s) => {
                assert_eq!(s.token(), "tok");
                assert_eq!(s.operator_id, "OP1");
                assert_eq!(s.operator_name, "Op One");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_refusal_carries_the_erps_own_words() {
        let reply = json!({"ok": false, "reason": "The Operator ID or PIN is wrong."});
        match parse_login(&reply, now()).unwrap() {
            LoginOutcome::Refused(why) => assert_eq!(why, "The Operator ID or PIN is wrong."),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_answer_that_is_neither_is_an_error_not_a_sign_in() {
        assert!(parse_login(&json!({}), now()).is_err());
        assert!(parse_login(&json!({"ok": true}), now()).is_err());
        assert!(parse_login(&json!({"ok": true, "session": ""}), now()).is_err());
    }

    #[test]
    fn a_key_the_erp_does_not_accept_is_told_without_the_page() {
        let text = ErpError::Status {
            code: 403,
            body: "<html>secret page</html>".into(),
        }
        .to_string();
        assert!(text.contains("API key"), "{text}");
        assert!(!text.contains("html"), "{text}");
    }
}
