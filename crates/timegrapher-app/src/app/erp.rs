//! The LN ERP in the app: Settings > ERP (the connection, the station's
//! signing key and the operator's sign-in) and the ERP view (search the ERP,
//! see the identity, write a note, send the Positions Test).
//!
//! Every call to the ERP runs on a thread of its own, one at a time, and
//! the window is drawn again when it answers, as the update check does. The
//! figures sent are the Positions Test table's, read by core; the calls,
//! the signing and the sign-in rules are `timegrapher-erp`'s, the same the
//! `timegrapher-erp` program uses. The operator's sign-in token lives here
//! in memory only, and the PIN is wiped as soon as it has been sent.

use super::erp_enhanced::{self, EnhancedInput};
use super::*;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::{Duration, Instant, SystemTime};
use timegrapher_erp::client::{Client, Identity, LoginOutcome};
use timegrapher_erp::config::{self, Config};
use timegrapher_erp::operator::{self, OperatorSession, Pin};
use timegrapher_erp::payload::{Context, PositionResult};
use timegrapher_erp::register::{self, Registration};
use timegrapher_erp::signing::Signer;
use timegrapher_erp::submit::{self, Report};
use zeroize::Zeroize;

/// How long a check that the ERP answers stays good before it is made again.
const RECHECK: Duration = Duration::from_secs(120);
/// How long the search field must rest before the ERP is asked.
const SEARCH_PAUSE: Duration = Duration::from_millis(350);
/// The fewest characters that are searched for.
const SEARCH_MIN_CHARS: usize = 2;
/// How wide Settings > ERP's text fields are, and the PIN's. A row's control
/// is laid out at the width it took last frame, so a field is given its
/// size, not a width to ask for.
pub(super) const FIELD_W: f32 = 250.0;
const PIN_W: f32 = 110.0;
/// How many matches the ERP is asked for.
const SEARCH_LIMIT: u32 = 12;

/// The names the ERP knows these by: the QA standards its certification
/// profiles carry, the measurement types and the temperature conditions.
pub(super) const QA_STANDARDS: [&str; 4] = ["Workshop", "Precision", "Signature", "Observatory"];
pub(super) const MEASUREMENT_TYPES: [&str; 5] = [
    "Initial Certification",
    "Follow-up Certification",
    "Service Check",
    "Regulation Check",
    "Qualification",
];
pub(super) const TEMPERATURES: [&str; 4] = ["Room / 23C", "Cold / 8C", "Warm / 38C", "Custom"];

pub(super) const ERP_HELP: &[&str] = &[
    "The LN ERP keeps your inventory, work orders and certificates. Here the station says where \
     the ERP is and who it is, and signs the operator in, so a result sent from the ERP view is \
     recorded against the right watch and the right person.",
    "The API key and secret belong to this station's account in the ERP, which an administrator \
     makes. The Station ID must match the station's record there. The secret is kept in a file \
     that only your account should read.",
    "The station signs each result with its own key, made once and registered with the ERP. \
     Sign in with the Operator ID and PIN the ERP gives you: the ERP then vouches for you, and \
     the results are recorded as Attested. Without a sign-in they are recorded as Invalid.",
    "The settings are shared with the timegrapher-erp command line program.",
];

const ERP_VIEW_HELP: &[&str] = &[
    "Search the ERP for a movement or watch by its serial, label, part or work order. Pick one to \
     see what the ERP knows about it.",
    "A note typed here goes with the next results sent, then clears. Send puts the Positions \
     Test on the Bench into the ERP, one result for each position read.",
];

/// A line of status: what happened, and whether it went wrong.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Note {
    pub(super) text: String,
    pub(super) ok: bool,
}

impl Note {
    fn ok(text: impl Into<String>) -> Option<Note> {
        Some(Note {
            text: text.into(),
            ok: true,
        })
    }

    fn bad(text: impl Into<String>) -> Option<Note> {
        Some(Note {
            text: text.into(),
            ok: false,
        })
    }
}

/// What a call to the ERP answered, with what was asked of it.
pub(super) enum Reply {
    /// Whether the saved ERP answered a check.
    Checked(bool),
    Tested(Result<String, String>),
    Key(Result<String, String>),
    SignedIn(Result<LoginOutcome, String>),
    Searched {
        query: String,
        found: Result<Vec<Identity>, String>,
    },
    Looked(Box<Result<Identity, String>>),
    Sent(Box<Sent>),
}

/// The results sent, and the operator's sign-in handed back: it travels
/// with the job and returns whatever happened.
pub(super) struct Sent {
    pub(super) report: Result<Report, String>,
    pub(super) operator: Option<OperatorSession>,
    /// What enhanced data was left out before sending, and why.
    pub(super) notes: Vec<String>,
}

/// The ERP's state in the window: the settings, the operator, the search
/// and the note, and the call under way.
#[derive(Default)]
pub(super) struct ErpState {
    /// Where the settings and key are kept; the default folder when none.
    pub(super) dir: Option<PathBuf>,
    loaded: bool,
    /// The settings as saved, and as typed in Settings.
    pub(super) config: Config,
    pub(super) form: Config,
    pub(super) status: Option<Note>,
    pub(super) key_note: Option<Note>,
    pub(super) operator_id: String,
    pub(super) pin: Pin,
    pub(super) operator: Option<OperatorSession>,
    pub(super) operator_note: Option<Note>,
    pub(super) query: String,
    pub(super) typed_at: Option<Instant>,
    pub(super) searched: String,
    pub(super) results: Vec<Identity>,
    pub(super) picked: Option<Identity>,
    pub(super) note: String,
    pub(super) qa_standard: String,
    pub(super) measurement_type: String,
    pub(super) temperature: String,
    pub(super) view_note: Option<Note>,
    pub(super) sent: Vec<String>,
    /// The Bench's ERP card is folded to its one row.
    pub(super) card_closed: bool,
    /// The Bench's quick search shows its matches under the card.
    pub(super) search_open: bool,
    /// The saved ERP answered its last check, which is made when the
    /// settings change and every [`RECHECK`] after.
    pub(super) active: bool,
    pub(super) checked_at: Option<Instant>,
    job: Option<Receiver<Reply>>,
}

impl ErpState {
    fn folder(&self) -> PathBuf {
        self.dir.clone().unwrap_or_else(config::dir)
    }

    /// Reads the saved settings, once, when the ERP is first used.
    pub(super) fn ensure_loaded(&mut self) {
        if std::mem::replace(&mut self.loaded, true) {
            return;
        }
        match Config::load(&self.folder()) {
            Ok(c) => {
                self.form = c.clone();
                self.config = c;
            }
            Err(e) => self.status = Note::bad(e),
        }
    }

    /// Whether the saved settings are complete and safe to use.
    pub(super) fn configured(&self) -> bool {
        self.config.check().is_ok()
    }

    /// Whether the ERP is defined and answering: what the Bench's card
    /// waits for.
    pub(super) fn live(&self) -> bool {
        self.configured() && self.active
    }

    pub(super) fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// Asks the saved ERP whether it answers, when that has not been done
    /// lately and no other call is under way; and has the window drawn again
    /// when the next check is due.
    pub(super) fn check_when_due(&mut self, ctx: &egui::Context) {
        if !self.configured() {
            self.active = false;
            return;
        }
        if let Some(at) = self.checked_at {
            if at.elapsed() < RECHECK {
                ctx.request_repaint_after(RECHECK - at.elapsed());
                return;
            }
        }
        if self.busy() {
            return;
        }
        self.checked_at = Some(Instant::now());
        let config = self.config.clone();
        self.start(ctx, move || {
            Reply::Checked(Client::new(&config).and_then(|c| c.ping()).is_ok())
        });
    }

    /// Whether the operator is signed in and still within the limits.
    pub(super) fn signed_in(&self) -> bool {
        self.operator
            .as_ref()
            .is_some_and(|o| o.is_usable(SystemTime::now()))
    }

    fn start(&mut self, ctx: &egui::Context, work: impl FnOnce() -> Reply + Send + 'static) {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(work());
            ctx.request_repaint();
        });
        self.job = Some(rx);
    }

    /// Takes in a finished call.
    pub(super) fn poll(&mut self) {
        let Some(rx) = &self.job else { return };
        let reply = match rx.try_recv() {
            Ok(reply) => reply,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                self.job = None;
                self.status = Note::bad("The call to the ERP stopped without an answer.");
                return;
            }
        };
        self.job = None;
        self.apply(reply);
    }

    pub(super) fn apply(&mut self, reply: Reply) {
        match reply {
            Reply::Checked(answered) => self.active = answered,
            Reply::Tested(r) => self.status = outcome(r),
            Reply::Key(r) => self.key_note = outcome(r),
            Reply::SignedIn(r) => self.signed_in_reply(r),
            Reply::Searched { query, found } => {
                self.searched = query;
                match found {
                    Ok(v) => {
                        self.view_note = v.is_empty().then(|| Note {
                            text: "No match in the ERP.".into(),
                            ok: true,
                        });
                        self.results = v;
                    }
                    Err(e) => {
                        self.results.clear();
                        self.view_note = Note::bad(e);
                    }
                }
            }
            Reply::Looked(found) => match *found {
                Ok(id) if id.found => self.pick(id),
                Ok(id) => {
                    let name = id.identity_id.unwrap_or_default();
                    self.view_note = Note::bad(format!("\"{name}\" is not known to the ERP."));
                }
                Err(e) => self.view_note = Note::bad(e),
            },
            Reply::Sent(done) => self.sent_reply(*done),
        }
    }

    fn signed_in_reply(&mut self, r: Result<LoginOutcome, String>) {
        match r {
            Ok(LoginOutcome::SignedIn(s)) => {
                self.operator_note = Note::ok(format!(
                    "Signed in as {}.",
                    if s.operator_name.is_empty() {
                        &s.operator_id
                    } else {
                        &s.operator_name
                    }
                ));
                self.operator = Some(s);
            }
            Ok(LoginOutcome::Refused(why)) => self.operator_note = Note::bad(why),
            Err(e) => self.operator_note = Note::bad(e),
        }
    }

    /// The identity picked, with what the ERP recommends for it.
    fn pick(&mut self, id: Identity) {
        self.measurement_type = id
            .recommended_measurement_type
            .clone()
            .unwrap_or_else(|| MEASUREMENT_TYPES[0].to_string());
        self.qa_standard = id
            .recommended_profile
            .clone()
            .unwrap_or_else(|| QA_STANDARDS[0].to_string());
        if self.temperature.is_empty() {
            self.temperature = TEMPERATURES[0].to_string();
        }
        self.view_note = None;
        self.sent.clear();
        self.search_open = false;
        self.picked = Some(id);
    }

    fn sent_reply(&mut self, done: Sent) {
        self.operator = done.operator;
        match done.report {
            Ok(report) => {
                self.sent = report_lines(&report);
                self.sent.extend(done.notes);
                let n = report.accepted();
                self.view_note = if n == report.sent.len() {
                    Note::ok(format!("Sent {n} of {} positions.", report.sent.len()))
                } else {
                    Note::bad(format!(
                        "Sent {n} of {} positions: see below.",
                        report.sent.len()
                    ))
                };
                // The note went with these results.
                if n > 0 {
                    self.note.clear();
                }
            }
            Err(e) => {
                self.sent.clear();
                self.view_note = Note::bad(e);
            }
        }
    }

    /// Asks the ERP for matches once the search field has rested.
    pub(super) fn search_when_rested(&mut self, ctx: &egui::Context) {
        let query = self.query.trim().to_string();
        if query.chars().count() < SEARCH_MIN_CHARS {
            if !self.results.is_empty() && self.job.is_none() {
                self.results.clear();
                self.searched.clear();
            }
            return;
        }
        if !self.configured() || self.busy() || query == self.searched {
            return;
        }
        let Some(typed) = self.typed_at else {
            return;
        };
        if typed.elapsed() < SEARCH_PAUSE {
            ctx.request_repaint_after(SEARCH_PAUSE - typed.elapsed());
            return;
        }
        let config = self.config.clone();
        self.start(ctx, move || {
            let found = Client::new(&config)
                .and_then(|c| c.search(&query, SEARCH_LIMIT))
                .map_err(|e| e.to_string());
            Reply::Searched { query, found }
        });
    }

    pub(super) fn look_up(&mut self, ctx: &egui::Context, identity_id: String) {
        let config = self.config.clone();
        self.start(ctx, move || {
            Reply::Looked(Box::new(
                Client::new(&config)
                    .and_then(|c| c.lookup_identity(&identity_id))
                    .map_err(|e| e.to_string()),
            ))
        });
    }

    pub(super) fn test_connection(&mut self, ctx: &egui::Context) {
        let config = Config {
            base_url: self.form.base_url.trim().trim_end_matches('/').to_string(),
            ..self.form.clone()
        };
        if let Err(why) = config.check() {
            self.status = Note::bad(why);
            return;
        }
        self.status = None;
        self.start(ctx, move || {
            Reply::Tested(
                Client::new(&config)
                    .and_then(|c| c.ping())
                    .map(|p| {
                        format!(
                            "Connected to {}: ERP user {}, API version {}.",
                            config.base_url,
                            p["user"].as_str().unwrap_or("?"),
                            p["api_version"].as_str().unwrap_or("?")
                        )
                    })
                    .map_err(|e| e.to_string()),
            )
        });
    }

    pub(super) fn save_settings(&mut self) {
        self.form.base_url = self.form.base_url.trim().trim_end_matches('/').to_string();
        if let Err(why) = self.form.check() {
            self.status = Note::bad(why);
            return;
        }
        match self.form.save(&self.folder()) {
            Ok(()) => {
                // Only a changed connection needs to be checked again.
                let connection_kept = Config {
                    enhanced: self.config.enhanced,
                    ..self.form.clone()
                } == self.config;
                if !connection_kept {
                    self.active = false;
                    self.checked_at = None;
                }
                self.config = self.form.clone();
                self.status = Note::ok("Saved.");
            }
            Err(e) => self.status = Note::bad(e),
        }
    }

    /// Makes the station's signing key if there is none, and registers it.
    pub(super) fn make_key(&mut self, ctx: &egui::Context) {
        let (config, dir) = (self.config.clone(), self.folder());
        self.key_note = None;
        self.start(ctx, move || Reply::Key(make_and_register(&config, &dir)));
    }

    pub(super) fn sign_in(&mut self, ctx: &egui::Context) {
        let (config, id) = (
            self.config.clone(),
            operator::clean_operator_id(&self.operator_id).to_string(),
        );
        // The PIN goes to the call and is wiped when it is done with.
        let pin = std::mem::take(&mut self.pin);
        self.operator_note = None;
        self.start(ctx, move || {
            Reply::SignedIn(
                Client::new(&config)
                    .and_then(|c| c.operator_login(&id, &pin, SystemTime::now()))
                    .map_err(|e| e.to_string()),
            )
        });
    }

    pub(super) fn sign_out(&mut self, ctx: &egui::Context) {
        let Some(session) = self.operator.take() else {
            return;
        };
        self.operator_note = Note::ok("Signed out.");
        self.pin.zeroize();
        let config = self.config.clone();
        // Best effort: the ERP ends the sign-in either now or by itself.
        std::thread::spawn(move || {
            if let Ok(c) = Client::new(&config) {
                c.operator_logout(&session);
            }
        });
        ctx.request_repaint();
    }

    /// Sends `results` signed, with the operator's sign-in if there is one,
    /// and no enhanced data: the form the tests use.
    #[cfg(test)]
    pub(super) fn send(&mut self, ctx: &egui::Context, results: Vec<PositionResult>) {
        self.send_with(ctx, results, None);
    }

    /// [`Self::send`], with each position's enhanced data when `enhanced`
    /// is given.
    pub(super) fn send_with(
        &mut self,
        ctx: &egui::Context,
        mut results: Vec<PositionResult>,
        enhanced: Option<EnhancedInput>,
    ) {
        let Some(identity) = self.picked.as_ref().and_then(|i| i.identity_id.clone()) else {
            return;
        };
        let (config, dir) = (self.config.clone(), self.folder());
        let mut operator = self.operator.take();
        let context = Context {
            station_id: config.station_id.clone(),
            station_version: env!("CARGO_PKG_VERSION").to_string(),
            operator_id: operator
                .as_ref()
                .map_or_else(String::new, |o| o.operator_id.clone()),
            identity_id: identity,
            work_order: self
                .picked
                .as_ref()
                .and_then(|i| i.current_work_order.clone())
                .unwrap_or_default(),
            qa_standard: self.qa_standard.clone(),
            temperature_condition: self.temperature.clone(),
            measurement_type: self.measurement_type.clone(),
            timestamp_iso: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        };
        self.view_note = None;
        self.start(ctx, move || {
            // The recordings are cut here, not in the window's frame.
            let notes = enhanced
                .map(|input| erp_enhanced::attach(&mut results, &input))
                .unwrap_or_default();
            let report = send_all(&config, &dir, &context, &mut operator, &results);
            Reply::Sent(Box::new(Sent {
                report,
                operator,
                notes,
            }))
        });
    }
}

fn outcome(r: Result<String, String>) -> Option<Note> {
    match r {
        Ok(text) => Note::ok(text),
        Err(text) => Note::bad(text),
    }
}

/// Makes the key and registers it, and says what happened.
fn make_and_register(config: &Config, dir: &Path) -> Result<String, String> {
    let client = Client::new(config).map_err(|e| e.to_string())?;
    let (signer, created) = Signer::load_or_create(dir, &Signer::key_id_for(&config.station_id))?;
    Ok(match register::ensure(&client, &signer, dir, created)? {
        Registration::Known => "The key is already registered.".to_string(),
        Registration::HeldAlready => "The key is already registered with the ERP.".to_string(),
        Registration::Registered { status } if status == "Pending" => {
            "Registered, and waiting for an administrator to activate it.".to_string()
        }
        Registration::Registered { .. } => "The key is made and registered.".to_string(),
    })
}

/// Signs and sends every position, registering the key first if need be.
fn send_all(
    config: &Config,
    dir: &Path,
    context: &Context,
    operator: &mut Option<OperatorSession>,
    results: &[PositionResult],
) -> Result<Report, String> {
    let client = Client::new(config).map_err(|e| e.to_string())?;
    let (signer, created) = Signer::load_or_create(dir, &Signer::key_id_for(&config.station_id))?;
    register::ensure(&client, &signer, dir, created)?;
    Ok(submit::send_session(
        &client,
        Some(&signer),
        context,
        operator,
        results,
        SystemTime::now,
    ))
}

/// What enhanced data the ERP kept with a position, in words.
fn kept_words(s: &submit::Sent) -> String {
    let mut words = String::new();
    if s.enhanced_kept {
        words.push_str(", with its whole measurement");
    }
    if let Some(Ok(bytes)) = &s.recording {
        words.push_str(&format!(" and recording ({:.1} MB)", *bytes as f64 / 1e6));
    }
    words
}

/// What happened to each position, one line each, and what to do about a
/// sign-in that did not count.
pub(super) fn report_lines(report: &Report) -> Vec<String> {
    let mut lines: Vec<String> = report
        .sent
        .iter()
        .map(|s| match &s.outcome {
            Ok(r) => format!(
                "{}: sent, {}{}",
                s.position,
                r.verification_status.as_deref().unwrap_or("recorded"),
                kept_words(s)
            ),
            Err(why) => format!("{}: not sent. {why}", s.position),
        })
        .collect();
    for s in &report.sent {
        if let Some(Err(why)) = &s.recording {
            lines.push(format!("{}: the recording was not kept. {why}", s.position));
        }
    }
    if let Some(why) = &report.enhanced_declined {
        lines.push(why.clone());
    }
    if report.signin_rejected || report.signin_stale {
        lines.push("The operator's sign-in did not count: sign in again in Settings.".into());
    }
    if let Some(Ok(done)) = &report.completed {
        if let Some(grade) = &done.suggested_functional_grade {
            lines.push(format!("Suggested grade: {grade}. Confirm it in the ERP."));
        }
    }
    lines
}

/// The Positions Test table's positions that have a reading, as results for
/// the ERP: each position's name, its rate, beat error and amplitude, the
/// beat rate and lift angle, and the seconds marked. A note typed goes first
/// in each result's notes.
pub(super) fn results_from_table(
    table: &guided::FreeTable,
    bph: Option<u32>,
    lift: Option<f64>,
    note: &str,
) -> Vec<PositionResult> {
    table
        .rows
        .iter()
        .filter_map(|(label, seconds, cell)| {
            let guided::FreeCell::Read(rate, amplitude, beat_error) = cell else {
                return None;
            };
            let position = core_session::Position::ALL
                .iter()
                .find(|p| guided::position_label(**p) == *label)?;
            let auto = format!(
                "Measured by timegrapher {}: {seconds:.0} s in {}",
                env!("CARGO_PKG_VERSION"),
                position.name()
            );
            let note = note.trim();
            Some(PositionResult {
                position: position.name(),
                rate_s_per_day: *rate,
                beat_error_ms: *beat_error,
                amplitude_deg: *amplitude,
                beat_frequency_bph: bph.map(f64::from),
                lift_angle_deg: lift,
                duration_seconds: Some(*seconds),
                notes: if note.is_empty() {
                    auto
                } else {
                    format!("{note}\n{auto}")
                },
                enhanced: None,
            })
        })
        .collect()
}

/// A status line: a warning icon when it went wrong, small grey words when
/// not.
pub(super) fn show_note(ui: &mut egui::Ui, note: &Option<Note>) {
    let Some(n) = note else { return };
    if n.ok {
        let p = theme::pal(ui);
        ui.label(RichText::new(&n.text).small().color(p.text_secondary));
    } else {
        theme::alert_label(ui, theme::Alert::Warning, &n.text, true);
    }
}

/// What a settings row's caption says about a note: its words, or an alert.
fn caption<'a>(note: &'a Option<Note>, plain: &'a str) -> theme::Caption<'a> {
    match note {
        Some(n) if !n.ok => theme::Caption::Alert(theme::Alert::Warning, &n.text),
        Some(n) => theme::Caption::Text(&n.text),
        None => theme::Caption::Text(plain),
    }
}

/// What Settings > ERP's buttons ask for.
#[derive(Clone, Copy)]
enum Act {
    Save,
    Test,
    MakeKey,
    SignIn,
    SignOut,
}

impl TimegrapherApp {
    /// Takes in a call to the ERP that has finished, and asks for a search
    /// once its field has rested; every frame, whichever view is shown.
    pub(super) fn erp_poll(&mut self, ctx: &egui::Context) {
        self.erp.poll();
        self.erp.check_when_due(ctx);
        self.erp.search_when_rested(ctx);
    }

    /// The Positions Test table of the take on show as results for the ERP,
    /// with the note typed so far: what Send would send now.
    pub(super) fn erp_results_now(&mut self, ctx: &egui::Context) -> Vec<PositionResult> {
        let Some(table) = self.free_table_shown(ctx) else {
            return Vec::new();
        };
        let lift = self.lift_known().then_some(self.lift_deg);
        results_from_table(&table, self.bph, lift, &self.erp.note)
    }

    /// Settings > ERP.
    pub(super) fn erp_settings(&mut self, ui: &mut egui::Ui) {
        self.erp.ensure_loaded();
        let mut act = None;
        self.erp_connection_group(ui, &mut act);
        self.erp_key_group(ui, &mut act);
        self.erp_operator_group(ui, &mut act);
        let ctx = ui.ctx().clone();
        match act {
            Some(Act::Save) => self.erp.save_settings(),
            Some(Act::Test) => self.erp.test_connection(&ctx),
            Some(Act::MakeKey) => self.erp.make_key(&ctx),
            Some(Act::SignIn) => self.erp.sign_in(&ctx),
            Some(Act::SignOut) => self.erp.sign_out(&ctx),
            None => {}
        }
    }

    fn erp_connection_group(&mut self, ui: &mut egui::Ui, act: &mut Option<Act>) {
        let busy = self.erp.busy();
        let unsaved = self.erp.form != self.erp.config;
        let status = self.erp.status.clone();
        let form = &mut self.erp.form;
        theme::group(ui, "Connection", Some(ERP_HELP), |g| {
            let text = |g: &mut theme::Group,
                        label: &str,
                        caption: &str,
                        hint: &str,
                        s: &mut String,
                        secret: bool| {
                g.row(label, theme::Caption::Text(caption), hint, |ui| {
                    ui.add_sized(
                        [FIELD_W, theme::CONTROL_H],
                        egui::TextEdit::singleline(s).password(secret),
                    )
                    .on_hover_text(hint);
                });
            };
            text(
                g,
                "ERP Address",
                "Starts with https://",
                "The ERP's address, e.g. https://erp.example.com",
                &mut form.base_url,
                false,
            );
            text(
                g,
                "API Key",
                "From the station's account in the ERP",
                "The API key of the account this station connects with",
                &mut form.api_key,
                false,
            );
            text(
                g,
                "API Secret",
                "Kept as plain text in your settings folder",
                "The API secret that goes with the key",
                &mut form.api_secret,
                true,
            );
            text(
                g,
                "Station ID",
                "As in the station's ERP record",
                "Must match the Station ID of this station's LN Workstation in the ERP",
                &mut form.station_id,
                false,
            );
            let hint = "Check the ERP's certificate against the system's list. Turn off only for a test server with its own certificate.";
            g.row(
                "Check Certificate",
                theme::Caption::Text("Leave on for a real ERP."),
                hint,
                |ui| theme::switch(ui, &mut form.verify_tls).on_hover_text(hint),
            );
            let hint = "Send each position's whole measurement and a recording of its sound as well as its figures. An ERP that cannot take them is sent the figures alone.";
            g.row(
                "Send Enhanced Data",
                theme::Caption::Text("The whole measurement and the sound."),
                hint,
                |ui| theme::switch(ui, &mut form.enhanced).on_hover_text(hint),
            );
            g.whole(|ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(unsaved, theme::primary(ui, "Save"))
                        .on_hover_text("Keep these settings")
                        .on_disabled_hover_text("Nothing has changed")
                        .clicked()
                    {
                        *act = Some(Act::Save);
                    }
                    if ui
                        .add_enabled(!busy, theme::secondary(ui, "Test Connection"))
                        .on_hover_text("Ask the ERP who this station is, with what is typed here")
                        .clicked()
                    {
                        *act = Some(Act::Test);
                    }
                    if busy {
                        ui.spinner();
                    }
                });
                if unsaved && status.is_none() {
                    ui.label(RichText::new("Not saved yet.").small().weak());
                }
                show_note(ui, &status);
            });
        });
    }

    fn erp_key_group(&mut self, ui: &mut egui::Ui, act: &mut Option<Act>) {
        let dir = self.erp.folder();
        let station = self.erp.config.station_id.clone();
        let key_id = Signer::key_id_for(&station);
        let made = Signer::path(&dir).exists();
        let registered = register::is_registered_as(&dir, &key_id);
        let ready = self.erp.configured() && !self.erp.busy();
        let note = self.erp.key_note.clone();
        theme::group(ui, "Station Key", None, |g| {
            let state = match (made, registered) {
                (_, true) => "Made and registered with the ERP.",
                (true, false) => "Made, not registered with the ERP yet.",
                (false, _) => "Not made yet.",
            };
            let label = if made {
                "Register Key"
            } else {
                "Make and Register Key"
            };
            g.row(
                "Signing Key",
                caption(&note, state),
                "The station signs each result with its own key. The ERP checks the signature \
                 against the public half it was given.",
                |ui| {
                    if !registered
                        && ui
                            .add_enabled(ready, theme::secondary(ui, label))
                            .on_hover_text("Make the key (once) and tell the ERP its public half")
                            .on_disabled_hover_text("Save the connection first")
                            .clicked()
                    {
                        *act = Some(Act::MakeKey);
                    }
                },
            );
            if !key_id.starts_with('-') {
                g.row(
                    "Key ID",
                    theme::Caption::None,
                    "How the ERP names this station's key",
                    |ui| {
                        ui.label(RichText::new(&key_id).small().weak());
                    },
                );
            }
        });
    }

    fn erp_operator_group(&mut self, ui: &mut egui::Ui, act: &mut Option<Act>) {
        let signed_in = self.erp.signed_in();
        let expired = self.erp.operator.is_some() && !signed_in;
        let name = self.erp.operator.as_ref().map(|o| {
            if o.operator_name.is_empty() {
                o.operator_id.clone()
            } else {
                o.operator_name.clone()
            }
        });
        let ready = self.erp.configured() && !self.erp.busy();
        let pin_ok = operator::pin_format_ok(&self.erp.pin);
        let id_ok = !operator::clean_operator_id(&self.erp.operator_id).is_empty();
        let note = self.erp.operator_note.clone();
        let (id, pin) = (&mut self.erp.operator_id, &mut *self.erp.pin);
        theme::group(ui, "Operator", None, |g| {
            if let (true, Some(name)) = (signed_in, name.as_deref()) {
                g.row(
                    "Signed In",
                    theme::Caption::Text("Results sent are attested by the ERP for this operator."),
                    "Who the ERP vouches for on the results sent",
                    |ui| {
                        ui.label(name);
                        if ui
                            .add(theme::secondary(ui, "Sign Out"))
                            .on_hover_text("End the sign-in")
                            .clicked()
                        {
                            *act = Some(Act::SignOut);
                        }
                    },
                );
                return;
            }
            let plain = if expired {
                "The sign-in has ended. Sign in again."
            } else {
                "Not signed in: results are recorded as Invalid."
            };
            g.row(
                "Operator ID",
                caption(&note, plain),
                "The Operator ID the ERP gave you",
                |ui| {
                    ui.add_sized([FIELD_W, theme::CONTROL_H], egui::TextEdit::singleline(id));
                },
            );
            g.row(
                "PIN",
                theme::Caption::Text("6 to 8 digits. Never kept."),
                "Your PIN. It is sent once, to sign in, and wiped.",
                |ui| {
                    ui.add_sized(
                        [PIN_W, theme::CONTROL_H],
                        egui::TextEdit::singleline(pin).password(true),
                    );
                    let why = if !ready {
                        Some("Save the connection first")
                    } else if !id_ok {
                        Some("Type the Operator ID")
                    } else if !pin_ok {
                        Some("The PIN is 6 to 8 digits")
                    } else {
                        None
                    };
                    if ui
                        .add_enabled(why.is_none(), theme::primary(ui, "Sign In"))
                        .on_hover_text("Sign in so the ERP can attest your results")
                        .on_disabled_hover_text(why.unwrap_or(""))
                        .clicked()
                    {
                        *act = Some(Act::SignIn);
                    }
                },
            );
        });
    }

    /// The ERP view: the card, its toolbar, and what the ERP says.
    pub(super) fn erp_view(&mut self, ui: &mut egui::Ui) {
        self.erp.ensure_loaded();
        let pal = theme::pal(ui);
        let rect = ui.max_rect();
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(theme::CARD_RADIUS), pal.card);
        let inner = rect.shrink2(Vec2::new(8.0, 6.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            self.erp_tools(ui);
            egui::ScrollArea::vertical()
                .id_salt("erp view")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_max_width(ui.available_width() - 8.0);
                    ui.spacing_mut().item_spacing.y = 8.0;
                    self.erp_body(ui);
                });
        });
    }

    fn erp_tools(&mut self, ui: &mut egui::Ui) {
        let configured = self.erp.configured();
        let mut typed = false;
        let mut settings = false;
        theme::pane_toolbar(ui, "ERP", ERP_VIEW_HELP, |ui| {
            theme::keep_next(ui);
            theme::tool_search(
                ui,
                "Find a movement or watch in the ERP by serial, label, part or work order",
                |ui| {
                    let w = (ui.available_width() - 48.0).clamp(96.0, 220.0);
                    let field = ui
                        .add_enabled(
                            configured,
                            egui::TextEdit::singleline(&mut self.erp.query)
                                .hint_text("Search the ERP")
                                .desired_width(w),
                        )
                        .on_hover_text(
                            "Find a movement or watch by serial, label, part or work order",
                        );
                    typed |= field.changed();
                },
            );
            theme::bar_gap(ui);
            if theme::tool_word_button(
                ui,
                Icon::Settings,
                "ERP Settings…",
                "The ERP's address, the station's key and the operator's sign-in",
                false,
                None,
            ) {
                settings = true;
            }
        });
        if typed {
            self.erp.typed_at = Some(Instant::now());
        }
        if settings {
            self.open_settings(Tab::Erp);
        }
    }

    fn erp_body(&mut self, ui: &mut egui::Ui) {
        let pal = theme::pal(ui);
        if !self.erp.configured() {
            watches::panel(ui, |ui| {
                ui.label("The ERP is not set up yet.");
                ui.label(
                    RichText::new(
                        "Settings > ERP takes its address, this station's API key and Station ID.",
                    )
                    .small()
                    .color(pal.text_secondary),
                );
                if ui.add(theme::primary(ui, "Open ERP Settings")).clicked() {
                    self.open_settings(Tab::Erp);
                }
            });
            return;
        }
        if !self.erp.active && !self.erp.busy() && self.erp.checked_at.is_some() {
            theme::alert_label(
                ui,
                theme::Alert::Warning,
                "The ERP is not answering now. Check the address in Settings > ERP, or the network.",
                true,
            );
        }
        show_note(ui, &self.erp.view_note.clone());
        self.erp_results(ui);
        self.erp_identity(ui);
        self.erp_note(ui);
        self.erp_send(ui);
    }

    fn erp_results(&mut self, ui: &mut egui::Ui) {
        let pal = theme::pal(ui);
        let shown = !self.erp.results.is_empty();
        watches::title_row(
            ui,
            "Matches",
            shown.then_some(self.erp.results.len()),
            |ui| {
                if self.erp.busy() {
                    ui.spinner();
                }
            },
        );
        let ctx = ui.ctx().clone();
        let mut pick = None;
        watches::panel(ui, |ui| {
            if !shown {
                let words = if self.erp.query.trim().chars().count() < SEARCH_MIN_CHARS {
                    "Type at least 2 characters in the search to look in the ERP."
                } else {
                    "No matches yet."
                };
                ui.label(RichText::new(words).small().color(pal.text_secondary));
            }
            for item in &self.erp.results {
                let id = item.identity_id.clone().unwrap_or_default();
                let on = self
                    .erp
                    .picked
                    .as_ref()
                    .and_then(|p| p.identity_id.as_deref())
                    == Some(id.as_str());
                let text = item.display.clone().unwrap_or_else(|| id.clone());
                if ui.selectable_label(on, text).clicked() {
                    pick = Some(id);
                }
            }
        });
        if let Some(id) = pick.filter(|_| !self.erp.busy()) {
            self.erp.look_up(&ctx, id);
        }
    }

    fn erp_identity(&mut self, ui: &mut egui::Ui) {
        let pal = theme::pal(ui);
        watches::title_row(ui, "Identity", None, |_| {});
        let ctx = ui.ctx().clone();
        let base = self.erp.config.base_url.clone();
        let Some(id) = self.erp.picked.clone() else {
            watches::panel(ui, |ui| {
                ui.label(
                    RichText::new("Pick a match to see what the ERP knows about it.")
                        .small()
                        .color(pal.text_secondary),
                );
            });
            return;
        };
        let status = [&id.lifecycle_status, &id.functional_grade, &id.qa_status]
            .iter()
            .filter_map(|s| s.as_deref())
            .collect::<Vec<_>>()
            .join(" · ");
        let part = id.ln_part.clone().or_else(|| id.item.clone());
        let rows = [
            ("Identity", id.identity_id.clone()),
            ("Movement S/N", id.movement_serial.clone()),
            ("Part", part),
            ("Watch Unit", id.current_watch_unit.clone()),
            ("Work Order", id.current_work_order.clone()),
            ("Status", (!status.is_empty()).then_some(status)),
            ("Recommended", id.recommended_measurement_type.clone()),
        ];
        watches::panel(ui, |ui| {
            for (label, value) in rows {
                let Some(value) = value.filter(|v| !v.is_empty()) else {
                    continue;
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label).color(pal.text_secondary));
                    ui.label(value);
                });
            }
            // A watch unit is not an identity record: no page to open.
            let link = id
                .identity_id
                .as_deref()
                .filter(|_| id.identity_type.as_deref() != Some("Watch"));
            if let Some(identity) = link {
                if ui
                    .add(theme::secondary(ui, "Open in ERP"))
                    .on_hover_text("Open this identity's record in your browser")
                    .clicked()
                {
                    ctx.open_url(egui::OpenUrl::new_tab(format!(
                        "{base}/desk/ln-inventory-identity/{identity}"
                    )));
                }
            }
        });
    }

    fn erp_note(&mut self, ui: &mut egui::Ui) {
        watches::title_row(ui, "Note", None, |_| {});
        watches::panel(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut self.erp.note)
                    .hint_text("Anything about this watch's measurement")
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            )
            .on_hover_text("Sent with the next results, then cleared");
            let pal = theme::pal(ui);
            ui.label(
                RichText::new("Goes with the next results sent, then clears.")
                    .small()
                    .color(pal.text_secondary),
            );
        });
    }

    /// Why the Positions Test can't be sent now.
    pub(super) fn erp_send_blocker(&self, results: &[PositionResult]) -> Option<&'static str> {
        if self.erp.picked.is_none() {
            Some("Pick the watch the Positions Test belongs to first")
        } else if self.running() {
            Some("Stop the take on the Bench first")
        } else if results.is_empty() {
            Some("No position has a reading yet: do a Positions Test on the Bench")
        } else if self.erp.busy() {
            Some("The ERP is answering")
        } else {
            None
        }
    }

    fn erp_send(&mut self, ui: &mut egui::Ui) {
        let pal = theme::pal(ui);
        watches::title_row(ui, "Send to ERP", None, |_| {});
        let ctx = ui.ctx().clone();
        let results = self.erp_results_now(&ctx);
        let blocker = self.erp_send_blocker(&results);
        let signed_in = self.erp.signed_in();
        let mut go = false;
        watches::panel(ui, |ui| {
            let erp = &mut self.erp;
            for (label, current, choices) in [
                (
                    "Measurement",
                    &mut erp.measurement_type,
                    &MEASUREMENT_TYPES[..],
                ),
                ("QA Standard", &mut erp.qa_standard, &QA_STANDARDS[..]),
                ("Temperature", &mut erp.temperature, &TEMPERATURES[..]),
            ] {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label).color(pal.text_secondary));
                    egui::ComboBox::from_id_salt(label)
                        .selected_text(current.as_str())
                        .show_ui(ui, |ui| {
                            for c in choices {
                                ui.selectable_value(current, (*c).to_string(), *c);
                            }
                        });
                });
            }
            ui.label(
                RichText::new(if signed_in {
                    "Signed in: the ERP will attest these results."
                } else {
                    "Not signed in: the ERP will record these as Invalid. Sign in in Settings > ERP."
                })
                .small()
                .color(pal.text_secondary),
            );
            let text = format!(
                "Send {} Position{}",
                results.len(),
                if results.len() == 1 { "" } else { "s" }
            );
            if ui
                .add_enabled(blocker.is_none(), theme::primary(ui, &text))
                .on_hover_text(
                    "Sign each position and send it to the ERP, then complete the session",
                )
                .on_disabled_hover_text(blocker.unwrap_or(""))
                .clicked()
            {
                go = true;
            }
            for line in &self.erp.sent {
                ui.label(RichText::new(line).small());
            }
        });
        if go {
            self.erp_send_now(&ctx, results);
        }
    }
}
