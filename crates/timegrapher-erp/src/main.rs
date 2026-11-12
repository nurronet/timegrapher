//! `timegrapher-erp`: sends a `timegrapher session` result to the LN ERP.
//!
//! ```text
//! timegrapher-erp setup --url https://erp.example.com --key KEY --station STATION-01
//! timegrapher-erp test
//! timegrapher-erp lookup SERIAL
//! timegrapher-erp submit recordings/watch-1/ --identity SERIAL --operator OP1
//! ```

use std::io::{BufRead, IsTerminal};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::SystemTime;

use clap::{Args, Parser, Subcommand};
use timegrapher_erp::client::{Client, Identity, LoginOutcome};
use timegrapher_erp::config::{self, Config};
use timegrapher_erp::operator::{self, OperatorSession, Pin};
use timegrapher_erp::payload::{self, Context};
use timegrapher_erp::register::{self, Registration};
use timegrapher_erp::signing::Signer;
use timegrapher_erp::submit::{self, Report};
use zeroize::Zeroizing;

#[derive(Parser)]
#[command(
    name = "timegrapher-erp",
    version,
    about = "Send a timegrapher session to the LN ERP: look up the watch, sign in the operator, send signed results"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Save the ERP's address, this station's API key and its Station ID
    Setup {
        /// The ERP's address, e.g. https://erp.example.com
        #[arg(long)]
        url: String,
        /// The station's API key (the secret is asked for, or read from TIMEGRAPHER_ERP_SECRET)
        #[arg(long)]
        key: String,
        /// The Station ID of this station's LN Workstation record
        #[arg(long)]
        station: String,
        /// Do not check the ERP's certificate (a test server only)
        #[arg(long)]
        insecure_tls: bool,
    },
    /// Check the address, the key and the secret
    Test,
    /// Find identities that match what is typed or scanned
    Search { text: String },
    /// Show what the ERP knows about one identity
    Lookup { identity: String },
    /// Register this station's signing key with the ERP
    Register,
    /// Send a session's positions to the ERP
    Submit(SubmitArgs),
}

#[derive(Args)]
struct SubmitArgs {
    /// A `session_report/summary.json`, or the session folder holding it
    summary: PathBuf,
    /// The identity measured: a movement's or a watch's serial
    #[arg(long)]
    identity: String,
    /// The Operator ID to sign in (the PIN is asked for). Without one the ERP records the results as Invalid.
    #[arg(long)]
    operator: Option<String>,
    #[arg(long, default_value = "")]
    work_order: String,
    /// The QA standard the ERP judges the figures against
    #[arg(long, default_value = "Workshop")]
    qa_standard: String,
    #[arg(long, default_value = "Room / 23C")]
    temperature: String,
    /// Default: the type the ERP recommends for the identity
    #[arg(long)]
    measurement_type: Option<String>,
    /// Show what would be signed and sent, and send nothing
    #[arg(long)]
    dry_run: bool,
}

fn main() -> ExitCode {
    match run(Cli::parse().command) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<ExitCode, String> {
    match command {
        Command::Setup {
            url,
            key,
            station,
            insecure_tls,
        } => setup(url, key, station, insecure_tls),
        Command::Test => test(),
        Command::Search { text } => search(&text),
        Command::Lookup { identity } => lookup(&identity),
        Command::Register => register(),
        Command::Submit(args) => submit_cmd(&args),
    }
}

/// A secret typed at the keyboard without an echo, or, when the input is
/// piped (a script), the first line of it. Wiped when dropped.
fn read_secret(prompt: &str) -> Result<Zeroizing<String>, String> {
    if std::io::stdin().is_terminal() {
        return rpassword::prompt_password(prompt)
            .map(Zeroizing::new)
            .map_err(|e| e.to_string());
    }
    let mut line = Zeroizing::new(String::new());
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    let kept = line.trim_end_matches(['\r', '\n']).to_string();
    Ok(Zeroizing::new(kept))
}

fn connect() -> Result<(Config, Client, PathBuf), String> {
    let dir = config::dir();
    let config = Config::load(&dir)?;
    let client = Client::new(&config).map_err(|e| e.to_string())?;
    Ok((config, client, dir))
}

fn setup(
    url: String,
    key: String,
    station: String,
    insecure_tls: bool,
) -> Result<ExitCode, String> {
    let secret = match std::env::var("TIMEGRAPHER_ERP_SECRET") {
        Ok(s) if !s.is_empty() => Zeroizing::new(s),
        _ => read_secret("API secret: ")?,
    };
    let config = Config {
        base_url: url.trim().trim_end_matches('/').to_string(),
        api_key: key,
        api_secret: secret.to_string(),
        station_id: station,
        verify_tls: !insecure_tls,
        enhanced: false,
    };
    config.check()?;
    let dir = config::dir();
    config.save(&dir)?;
    println!("Saved to {}", Config::path(&dir).display());
    println!("The API secret is stored there as plain text; keep that folder private.");
    println!("Next: timegrapher-erp test");
    Ok(ExitCode::SUCCESS)
}

fn test() -> Result<ExitCode, String> {
    let (config, client, _) = connect()?;
    let reply = client.ping().map_err(|e| e.to_string())?;
    println!("Connected to {}", config.base_url);
    for (label, key) in [
        ("ERP user", "user"),
        ("Server time", "server_time"),
        ("API version", "api_version"),
    ] {
        println!("{label}: {}", reply[key].as_str().unwrap_or("?"));
    }
    println!("Station ID: {}", config.station_id);
    Ok(ExitCode::SUCCESS)
}

fn search(text: &str) -> Result<ExitCode, String> {
    let (_, client, _) = connect()?;
    let reply = client
        .search_identities(text, 12)
        .map_err(|e| e.to_string())?;
    let items = reply["items"].as_array().cloned().unwrap_or_default();
    if items.is_empty() {
        println!("No matches for \"{text}\".");
    }
    for item in items {
        println!("{}", item["display"].as_str().unwrap_or("?"));
    }
    Ok(ExitCode::SUCCESS)
}

fn lookup(identity: &str) -> Result<ExitCode, String> {
    let (_, client, _) = connect()?;
    let found = client
        .lookup_identity(identity)
        .map_err(|e| e.to_string())?;
    if !found.found {
        println!("\"{identity}\" is not known to the ERP.");
        print_suggestions(&found);
        return Ok(ExitCode::FAILURE);
    }
    print_identity(&found);
    Ok(ExitCode::SUCCESS)
}

fn print_suggestions(identity: &Identity) {
    for item in &identity.suggestions {
        println!(
            "  did you mean: {}",
            item["display"].as_str().unwrap_or("?")
        );
    }
}

fn print_identity(identity: &Identity) {
    let part = identity.ln_part.as_ref().or(identity.item.as_ref());
    let status = [
        &identity.lifecycle_status,
        &identity.functional_grade,
        &identity.qa_status,
    ]
    .iter()
    .filter_map(|s| s.as_deref())
    .collect::<Vec<_>>()
    .join(" · ");
    for (label, value) in [
        ("Identity", identity.identity_id.as_deref()),
        ("Movement S/N", identity.movement_serial.as_deref()),
        ("Part", part.map(String::as_str)),
        ("Watch unit", identity.current_watch_unit.as_deref()),
        ("Work order", identity.current_work_order.as_deref()),
        ("Status", Some(status.as_str())),
        (
            "Recommended",
            identity.recommended_measurement_type.as_deref(),
        ),
    ] {
        if let Some(value) = value.filter(|v| !v.is_empty()) {
            println!("{label}: {value}");
        }
    }
}

fn register() -> Result<ExitCode, String> {
    let (config, client, dir) = connect()?;
    let key_id = Signer::key_id_for(&config.station_id);
    let (signer, created) = Signer::load_or_create(&dir, &key_id)?;
    if created {
        println!("Made a new signing key: {}", Signer::path(&dir).display());
    }
    ensure_registered(&client, &signer, &dir, created)?;
    Ok(ExitCode::SUCCESS)
}

/// Registers the key with the ERP unless this station already has, and says
/// what happened.
fn ensure_registered(
    client: &Client,
    signer: &Signer,
    dir: &Path,
    created: bool,
) -> Result<(), String> {
    match register::ensure(client, signer, dir, created)? {
        Registration::Known => {}
        Registration::Registered { status } => {
            println!("Registered key {}: {status}", signer.key_id());
            if status == "Pending" {
                println!("An administrator must activate it before results verify.");
            }
        }
        Registration::HeldAlready => println!(
            "Key {} is already registered with the ERP.",
            signer.key_id()
        ),
    }
    Ok(())
}

fn summary_file(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join("session_report").join("summary.json")
    } else {
        path.to_path_buf()
    }
}

fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn submit_cmd(args: &SubmitArgs) -> Result<ExitCode, String> {
    let file = summary_file(&args.summary);
    let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let figures = payload::read_summary(&text)?;
    for why in &figures.left_out {
        println!("Left out: {why}");
    }
    if figures.results.is_empty() {
        return Err("no position in the summary has figures to send".to_string());
    }
    if args.dry_run {
        return dry_run(args, &figures.results);
    }
    send(args, &figures.results)
}

fn context(
    args: &SubmitArgs,
    station_id: &str,
    operator_id: &str,
    measurement_type: &str,
) -> Context {
    Context {
        station_id: station_id.to_string(),
        station_version: env!("CARGO_PKG_VERSION").to_string(),
        operator_id: operator_id.to_string(),
        identity_id: args.identity.clone(),
        work_order: args.work_order.clone(),
        qa_standard: args.qa_standard.clone(),
        temperature_condition: args.temperature.clone(),
        measurement_type: measurement_type.to_string(),
        timestamp_iso: now_iso(),
    }
}

fn dry_run(args: &SubmitArgs, results: &[payload::PositionResult]) -> Result<ExitCode, String> {
    let measurement_type = args
        .measurement_type
        .as_deref()
        .unwrap_or("Initial Certification");
    let ctx = context(
        args,
        "<station id>",
        args.operator.as_deref().unwrap_or(""),
        measurement_type,
    );
    for result in results {
        let signed = result.signed_fields(&ctx);
        println!(
            "{}: {}",
            result.position,
            String::from_utf8_lossy(&timegrapher_erp::canonical::to_bytes(&signed))
        );
    }
    println!("Dry run: nothing was signed or sent.");
    Ok(ExitCode::SUCCESS)
}

fn sign_in(client: &Client, operator_id: Option<&str>) -> Result<Option<OperatorSession>, String> {
    let Some(typed) = operator_id else {
        println!("No --operator given: the ERP will record these results as Invalid (a station signature is one of two required).");
        return Ok(None);
    };
    let id = operator::clean_operator_id(typed);
    let pin: Pin = read_secret(&format!("PIN for {id}: "))?;
    if !operator::pin_format_ok(&pin) {
        return Err("The PIN is 6 to 8 digits. Nothing was sent, so no try was used.".to_string());
    }
    match client
        .operator_login(id, &pin, SystemTime::now())
        .map_err(|e| e.to_string())?
    {
        LoginOutcome::SignedIn(session) => {
            println!("Signed in: {}", session.operator_name);
            Ok(Some(session))
        }
        LoginOutcome::Refused(reason) => Err(reason),
    }
}

fn send(args: &SubmitArgs, results: &[payload::PositionResult]) -> Result<ExitCode, String> {
    let (config, client, dir) = connect()?;
    let identity = client
        .lookup_identity(&args.identity)
        .map_err(|e| e.to_string())?;
    if !identity.found {
        print_suggestions(&identity);
        return Err(format!(
            "\"{}\" is not known to the ERP; nothing was sent",
            args.identity
        ));
    }
    let (signer, created) = Signer::load_or_create(&dir, &Signer::key_id_for(&config.station_id))?;
    ensure_registered(&client, &signer, &dir, created)?;

    let mut operator = sign_in(&client, args.operator.as_deref())?;
    let operator_id = operator.as_ref().map_or_else(
        || args.operator.clone().unwrap_or_default(),
        |s| s.operator_id.clone(),
    );
    let measurement_type = args
        .measurement_type
        .clone()
        .or(identity.recommended_measurement_type)
        .unwrap_or_else(|| "Initial Certification".to_string());
    let mut ctx = context(args, &config.station_id, &operator_id, &measurement_type);
    ctx.identity_id = identity
        .identity_id
        .unwrap_or_else(|| args.identity.clone());

    let report = submit::send_session(
        &client,
        Some(&signer),
        &ctx,
        &mut operator,
        results,
        SystemTime::now,
    );
    if let Some(session) = &operator {
        client.operator_logout(session);
    }
    Ok(print_report(&report))
}

fn print_report(report: &Report) -> ExitCode {
    let mut attested = 0;
    for sent in &report.sent {
        match &sent.outcome {
            Ok(reply) => {
                let status = reply.verification_status.as_deref().unwrap_or("?");
                attested += usize::from(status == "Attested" || status == "Verified");
                println!(
                    "{}: sent. {}; {}",
                    sent.position,
                    reply.pass_fail.as_deref().unwrap_or("?"),
                    status
                );
            }
            Err(why) => println!("{}: NOT sent. {why}", sent.position),
        }
    }
    if let Some(session) = &report.session {
        println!("ERP session: {session}");
    }
    match &report.completed {
        Some(Ok(done)) => {
            if let Some(grade) = &done.suggested_functional_grade {
                println!(
                    "Suggested grade: {grade} ({}). Not applied: confirm it in the ERP.",
                    done.suggested_grade_reason.as_deref().unwrap_or("")
                );
            }
        }
        Some(Err(why)) => println!("The session could not be completed: {why}"),
        None => {}
    }
    println!(
        "{} of {} positions accepted; {attested} attested.",
        report.accepted(),
        report.sent.len()
    );
    if report.signin_rejected {
        println!("The ERP did not accept the operator's sign-in on some results; sign in again.");
    }
    if report.signin_stale {
        println!(
            "The operator's sign-in had passed the station's limits, so some results carried none."
        );
    }
    if report.accepted() == report.sent.len() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
