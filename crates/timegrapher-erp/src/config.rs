//! The connection settings: the ERP's address, the station's API key and
//! secret, and its Station ID.
//!
//! They are kept in `erp.toml` in the configuration folder. The secret is
//! stored there as plain text (readable by the one account that owns the
//! file), so protect that account; or leave it out of the file and give it as
//! `TIMEGRAPHER_ERP_SECRET`. The key, address and Station ID can be given in
//! the environment too, and the environment wins.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const CONFIG_FILE: &str = "erp.toml";
const FOLDER: &str = "timegrapher-erp";

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The ERP's address with no path, e.g. `https://erp.example.com`.
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub api_secret: String,
    /// Must match an LN Workstation record in the ERP.
    pub station_id: String,
    /// Leave on. Off only for a test server with a self-signed certificate.
    #[serde(default = "yes")]
    pub verify_tls: bool,
    /// Send enhanced data too: each position's whole measurement and a
    /// recording of its sound. Off by default; it is much more to send.
    #[serde(default)]
    pub enhanced: bool,
}

fn yes() -> bool {
    true
}

impl Default for Config {
    /// Nothing set, with the certificate checked.
    fn default() -> Self {
        Config {
            base_url: String::new(),
            api_key: String::new(),
            api_secret: String::new(),
            station_id: String::new(),
            verify_tls: true,
            enhanced: false,
        }
    }
}

// The secret never appears when a Config is printed.
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key)
            .field("api_secret", &"<hidden>")
            .field("station_id", &self.station_id)
            .field("verify_tls", &self.verify_tls)
            .field("enhanced", &self.enhanced)
            .finish()
    }
}

/// The folder the settings and the signing key live in:
/// `TIMEGRAPHER_ERP_DIR` if set, else the account's own configuration folder.
pub fn dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("TIMEGRAPHER_ERP_DIR") {
        return PathBuf::from(dir);
    }
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        home().map(|h| h.join("Library").join("Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home().map(|h| h.join(".config")))
    };
    base.unwrap_or_else(|| PathBuf::from(".")).join(FOLDER)
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

impl Config {
    pub fn path(dir: &Path) -> PathBuf {
        dir.join(CONFIG_FILE)
    }

    /// Reads the settings, then applies the environment over them. With no
    /// file, the environment alone must give them.
    pub fn load(dir: &Path) -> Result<Config, String> {
        Config::load_with(dir, |name| std::env::var(name).ok())
    }

    pub(crate) fn load_with(
        dir: &Path,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<Config, String> {
        let path = Config::path(dir);
        let mut config = match fs::read_to_string(&path) {
            Ok(text) => {
                toml::from_str::<Config>(&text).map_err(|e| format!("{}: {e}", path.display()))?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        for (name, slot) in [
            ("TIMEGRAPHER_ERP_URL", &mut config.base_url),
            ("TIMEGRAPHER_ERP_KEY", &mut config.api_key),
            ("TIMEGRAPHER_ERP_SECRET", &mut config.api_secret),
            ("TIMEGRAPHER_ERP_STATION", &mut config.station_id),
        ] {
            if let Some(value) = env(name).filter(|v| !v.is_empty()) {
                *slot = value;
            }
        }
        config.base_url = config.base_url.trim().trim_end_matches('/').to_string();
        Ok(config)
    }

    /// Writes the settings, readable by this account only where the system
    /// allows it.
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = Config::path(dir);
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// Is everything needed to talk to the ERP given, and is the address one
    /// that is safe to send a key to? Only `https` is, except for this
    /// computer itself (a test ERP).
    pub fn check(&self) -> Result<(), String> {
        for (name, value) in [
            ("ERP address", &self.base_url),
            ("API key", &self.api_key),
            ("API secret", &self.api_secret),
            ("Station ID", &self.station_id),
        ] {
            if value.is_empty() {
                return Err(format!(
                    "{name} is not set. Run `timegrapher-erp setup`, or set the TIMEGRAPHER_ERP_* variables."
                ));
            }
        }
        match self.base_url.split_once("://") {
            Some(("https", rest)) if !rest.is_empty() => Ok(()),
            Some(("http", rest)) if is_this_computer(rest) => Ok(()),
            Some(("http", _)) => Err(
                "The ERP address must start with https://. The API key and secret are sent with every call."
                    .to_string(),
            ),
            _ => Err("The ERP address must look like https://erp.example.com".to_string()),
        }
    }
}

fn is_this_computer(authority_and_path: &str) -> bool {
    let authority = authority_and_path.split('/').next().unwrap_or("");
    let host = if let Some(rest) = authority.strip_prefix('[') {
        rest.split(']').next().unwrap_or("")
    } else {
        authority.split(':').next().unwrap_or("")
    };
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn sample() -> Config {
        Config {
            base_url: "https://erp.example.com".into(),
            api_key: "key".into(),
            api_secret: "very-secret".into(),
            station_id: "S1".into(),
            verify_tls: true,
            enhanced: false,
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tg-erp-cfg-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn settings_are_kept_and_read_back() {
        let dir = temp_dir("roundtrip");
        sample().save(&dir).unwrap();
        let back = Config::load_with(&dir, env(&[])).unwrap();
        assert_eq!(back.base_url, "https://erp.example.com");
        assert_eq!(back.api_secret, "very-secret");
        assert!(back.verify_tls);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_environment_wins_over_the_file_and_can_stand_alone() {
        let dir = temp_dir("env");
        sample().save(&dir).unwrap();
        let c = Config::load_with(
            &dir,
            env(&[
                ("TIMEGRAPHER_ERP_SECRET", "from-env"),
                ("TIMEGRAPHER_ERP_URL", "https://other.test/"),
            ]),
        )
        .unwrap();
        assert_eq!(c.api_secret, "from-env");
        assert_eq!(c.base_url, "https://other.test");
        assert_eq!(c.api_key, "key");
        let alone = Config::load_with(
            &temp_dir("none"),
            env(&[
                ("TIMEGRAPHER_ERP_URL", "https://e.test"),
                ("TIMEGRAPHER_ERP_KEY", "k"),
                ("TIMEGRAPHER_ERP_SECRET", "s"),
                ("TIMEGRAPHER_ERP_STATION", "ST"),
            ]),
        )
        .unwrap();
        assert!(alone.check().is_ok());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_with_an_unknown_setting_is_refused_not_ignored() {
        let dir = temp_dir("unknown");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            Config::path(&dir),
            "base_url='https://e.test'\napi_key='k'\nstation_id='s'\nverfy_tls=false\n",
        )
        .unwrap();
        let err = Config::load_with(&dir, env(&[])).err().unwrap();
        assert!(err.contains("verfy_tls"), "{err}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn credentials_go_only_over_https_or_to_this_computer() {
        let mut c = sample();
        assert!(c.check().is_ok());
        for ok in [
            "http://localhost:8000",
            "http://127.0.0.1",
            "http://[::1]:9/",
            "https://erp.test",
        ] {
            c.base_url = ok.into();
            assert!(c.check().is_ok(), "{ok}");
        }
        for bad in [
            "http://erp.example.com",
            "http://localhost.evil.test",
            "erp.example.com",
            "ftp://x",
            "https://",
        ] {
            c.base_url = bad.into();
            assert!(c.check().is_err(), "{bad}");
        }
    }

    #[test]
    fn a_missing_setting_is_named() {
        let mut c = sample();
        c.api_secret.clear();
        assert!(c.check().err().unwrap().contains("API secret"));
        let mut c = sample();
        c.station_id.clear();
        assert!(c.check().err().unwrap().contains("Station ID"));
    }

    #[test]
    fn the_secret_is_not_shown_when_the_settings_are_printed() {
        let text = format!("{:?}", sample());
        assert!(!text.contains("very-secret"), "{text}");
    }
}
