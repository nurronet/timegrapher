//! Operator PIN sign-in: the rules that need no network.
//!
//! The operator types an Operator ID and PIN; the ERP answers with a session
//! token, which the station keeps in memory and sends with each result in
//! place of the PIN. The token is never written to a file or a log, and the
//! PIN is wiped as soon as the call returns.

use std::time::{Duration, SystemTime};

use zeroize::Zeroizing;

/// The ERP ends a sign-in 8 hours after it began, or after 30 minutes
/// without a result. The station stops using a token a little before that,
/// so a stale one is never sent: a result sent with an ended sign-in arrives
/// at the ERP as Invalid.
pub const MAX_AGE: Duration = Duration::from_secs(7 * 3600 + 50 * 60);
pub const MAX_IDLE: Duration = Duration::from_secs(25 * 60);

/// What the ERP's message starts with when it did not accept the sign-in.
const REJECTED_PREFIX: &str = "Operator sign-in:";

/// A live sign-in. Held in memory only.
pub struct OperatorSession {
    token: Zeroizing<String>,
    pub operator_id: String,
    pub operator_name: String,
    signed_at: SystemTime,
    last_used: SystemTime,
}

impl OperatorSession {
    pub fn new(token: String, operator_id: String, operator_name: String, now: SystemTime) -> Self {
        OperatorSession {
            token: Zeroizing::new(token),
            operator_id,
            operator_name,
            signed_at: now,
            last_used: now,
        }
    }

    /// The token, for the one place that sends it.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Is the sign-in still safe to send at `now`?
    pub fn is_usable(&self, now: SystemTime) -> bool {
        is_fresh(self.signed_at, self.last_used, now)
    }

    /// A result was accepted just now: the ERP counted the sign-in as used.
    pub fn used(&mut self, now: SystemTime) {
        self.last_used = now;
    }
}

impl std::fmt::Debug for OperatorSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OperatorSession")
            .field("operator_id", &self.operator_id)
            .field("token", &"<hidden>")
            .finish()
    }
}

/// Is a sign-in made at `signed_at` and last used at `last_used` still safe
/// to send at `now`? A clock that moved back counts as stale, not fresh.
pub fn is_fresh(signed_at: SystemTime, last_used: SystemTime, now: SystemTime) -> bool {
    let within = |since: SystemTime, limit: Duration| {
        now.duration_since(since)
            .map(|age| age < limit)
            .unwrap_or(false)
    };
    within(signed_at, MAX_AGE) && within(last_used, MAX_IDLE)
}

/// 6 to 8 ASCII digits. Checked before sending, so a mistyped PIN never
/// costs the operator one of their five tries. (Weak PINs cannot exist on
/// the ERP side, so a pattern here is simply a wrong PIN.)
pub fn pin_format_ok(pin: &str) -> bool {
    (6..=8).contains(&pin.len()) && pin.bytes().all(|b| b.is_ascii_digit())
}

/// Does the ERP's message about a result say the operator sign-in was not
/// accepted?
pub fn message_rejects_signin(verification_message: &str) -> bool {
    verification_message.starts_with(REJECTED_PREFIX)
}

/// An Operator ID as typed or scanned, without stray spaces or line ends.
pub fn clean_operator_id(typed: &str) -> &str {
    typed.trim_matches(|c: char| c == ' ' || c == '\t' || c == '\r' || c == '\n')
}

/// A PIN held so that it is wiped when dropped.
pub type Pin = Zeroizing<String>;

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000 + secs)
    }

    #[test]
    fn a_pin_is_six_to_eight_digits() {
        for ok in ["123456", "1234567", "12345678", "000000"] {
            assert!(pin_format_ok(ok), "{ok}");
        }
        for bad in [
            "",
            "12345",
            "123456789",
            "12345a",
            "12 456",
            "١٢٣٤٥٦",
            "+123456",
        ] {
            assert!(!pin_format_ok(bad), "{bad}");
        }
    }

    #[test]
    fn a_sign_in_is_used_up_to_the_stations_own_limits() {
        let signed = at(0);
        // Age: 7 h 50 min is the edge.
        assert!(is_fresh(
            signed,
            at(7 * 3600 + 49 * 60),
            at(7 * 3600 + 49 * 60 + 59)
        ));
        assert!(!is_fresh(
            signed,
            at(7 * 3600 + 50 * 60 - 60),
            at(7 * 3600 + 50 * 60)
        ));
        // Idle: 25 min is the edge.
        assert!(is_fresh(signed, at(0), at(25 * 60 - 1)));
        assert!(!is_fresh(signed, at(0), at(25 * 60)));
        // Using it again moves the idle edge, not the age edge.
        assert!(is_fresh(signed, at(20 * 60), at(40 * 60)));
        assert!(!is_fresh(signed, at(7 * 3600), at(7 * 3600 + 50 * 60)));
    }

    #[test]
    fn a_clock_that_moved_back_is_not_fresh() {
        assert!(!is_fresh(at(100), at(100), at(50)));
    }

    #[test]
    fn a_session_notes_when_it_was_last_used() {
        let mut s = OperatorSession::new("t".into(), "OP1".into(), "Op".into(), at(0));
        assert!(s.is_usable(at(24 * 60)));
        assert!(!s.is_usable(at(26 * 60)));
        s.used(at(24 * 60));
        assert!(s.is_usable(at(26 * 60)));
        assert_eq!(s.token(), "t");
    }

    #[test]
    fn the_token_is_not_shown_when_a_session_is_printed() {
        let s = OperatorSession::new("secret-token".into(), "OP1".into(), "Op".into(), at(0));
        let text = format!("{s:?}");
        assert!(!text.contains("secret-token"), "{text}");
        assert!(text.contains("OP1"));
    }

    #[test]
    fn the_erps_refusal_is_told_by_its_prefix() {
        assert!(message_rejects_signin(
            "Operator sign-in: The operator has signed out. Sign in again."
        ));
        assert!(!message_rejects_signin(
            "Station signature verified; operator OP1 attested"
        ));
        assert!(!message_rejects_signin(
            "An Operator sign-in: appears later"
        ));
    }

    #[test]
    fn an_operator_id_loses_stray_spaces_and_line_ends() {
        assert_eq!(clean_operator_id("  OP1\r\n"), "OP1");
        assert_eq!(clean_operator_id("O P"), "O P");
    }
}
