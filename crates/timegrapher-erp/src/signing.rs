//! The station's own signing key: Ed25519, registered with the ERP so it can
//! check what the station signs.
//!
//! The key file holds the 32-byte seed followed by the 32-byte public key,
//! the layout libsodium uses for a secret key, so the Ln-tg station's
//! `workstation_signing_key.bin` can be copied here and the same registered
//! key keeps working.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use ring::rand::{SecureRandom, SystemRandom};
use ring::signature::{Ed25519KeyPair, KeyPair};
use zeroize::Zeroize;

/// The file the key is kept in, inside the configuration folder.
pub const KEY_FILE: &str = "workstation_signing_key.bin";

/// What the ERP expects a station's key to be named after its Station ID.
/// Only a key named this way, for a registered station, is active at once.
pub const KEY_ID_SUFFIX: &str = "-workstation";

const SEED_BYTES: usize = 32;
const PUBLIC_BYTES: usize = 32;

pub struct Signer {
    pair: Ed25519KeyPair,
    key_id: String,
}

impl Signer {
    /// The key ID for a station: `<Station ID>-workstation`.
    pub fn key_id_for(station_id: &str) -> String {
        format!("{station_id}{KEY_ID_SUFFIX}")
    }

    /// Where the key file is kept.
    pub fn path(dir: &Path) -> PathBuf {
        dir.join(KEY_FILE)
    }

    /// A signer from a seed.
    pub fn from_seed(seed: &[u8; SEED_BYTES], key_id: &str) -> Result<Signer, String> {
        let pair = Ed25519KeyPair::from_seed_unchecked(seed)
            .map_err(|_| "the signing key's seed was not accepted".to_string())?;
        Ok(Signer {
            pair,
            key_id: key_id.to_string(),
        })
    }

    /// Loads the key kept in `dir`, or makes and keeps one. The second value
    /// is true when the key is new, and so not yet known to the ERP.
    pub fn load_or_create(dir: &Path, key_id: &str) -> Result<(Signer, bool), String> {
        let path = Signer::path(dir);
        if path.exists() {
            return Ok((Signer::load(&path, key_id)?, false));
        }
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let mut seed = [0u8; SEED_BYTES];
        SystemRandom::new()
            .fill(&mut seed)
            .map_err(|_| "the system could not give random bytes for a key".to_string())?;
        let signer = Signer::from_seed(&seed, key_id)?;
        let result = signer.keep(&path, &seed);
        seed.zeroize();
        result?;
        Ok((signer, true))
    }

    fn load(path: &Path, key_id: &str) -> Result<Signer, String> {
        let mut bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let result = if bytes.len() == SEED_BYTES + PUBLIC_BYTES {
            let (seed, public) = bytes.split_at(SEED_BYTES);
            Ed25519KeyPair::from_seed_and_public_key(seed, public)
                .map(|pair| Signer {
                    pair,
                    key_id: key_id.to_string(),
                })
                .map_err(|_| format!("{}: the public key does not match the seed", path.display()))
        } else {
            Err(format!(
                "{}: expected {} bytes (seed and public key), found {}",
                path.display(),
                SEED_BYTES + PUBLIC_BYTES,
                bytes.len()
            ))
        };
        bytes.zeroize();
        result
    }

    /// Writes the key file, never over one that is there: a lost key cannot
    /// be replaced under the same ID (the ERP refuses a second registration).
    fn keep(&self, path: &Path, seed: &[u8; SEED_BYTES]) -> Result<(), String> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let mut bytes = Vec::with_capacity(SEED_BYTES + PUBLIC_BYTES);
        bytes.extend_from_slice(seed);
        bytes.extend_from_slice(self.pair.public_key().as_ref());
        let written = file.write_all(&bytes);
        bytes.zeroize();
        written.map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// The public key, 32 raw bytes in standard base64: what the ERP's
    /// `register_key` takes.
    pub fn public_key_b64(&self) -> String {
        STANDARD.encode(self.pair.public_key().as_ref())
    }

    /// The signature over `message`, standard base64 with padding.
    pub fn sign_b64(&self, message: &[u8]) -> String {
        STANDARD.encode(self.pair.sign(message).as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed() -> [u8; 32] {
        std::array::from_fn(|i| i as u8)
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tg-erp-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    // The seed 00 01 .. 1f, signed with Python's cryptography library, over
    // the guide's test payload (see payload.rs, `the_guides_test_vector`).
    #[test]
    fn the_test_seed_gives_the_published_key() {
        let signer = Signer::from_seed(&seed(), "EXAMPLE-STATION-01-workstation").unwrap();
        assert_eq!(
            signer.public_key_b64(),
            "A6EHv/POEL4dcN0Y50vAmWfk1jCbpQ1fHdyGZBJVMbg="
        );
    }

    #[test]
    fn a_new_key_is_kept_and_found_again() {
        let dir = temp_dir("keep");
        let (first, created) = Signer::load_or_create(&dir, "S-workstation").unwrap();
        assert!(created);
        let (second, created) = Signer::load_or_create(&dir, "S-workstation").unwrap();
        assert!(!created);
        assert_eq!(first.public_key_b64(), second.public_key_b64());
        assert_eq!(first.sign_b64(b"x"), second.sign_b64(b"x"));
        assert_eq!(fs::read(Signer::path(&dir)).unwrap().len(), 64);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_in_the_layout_ln_tg_writes_is_read() {
        let dir = temp_dir("lntg");
        fs::create_dir_all(&dir).unwrap();
        let known = Signer::from_seed(&seed(), "S-workstation").unwrap();
        let mut file = seed().to_vec();
        file.extend_from_slice(&STANDARD.decode(known.public_key_b64()).unwrap());
        fs::write(Signer::path(&dir), file).unwrap();
        let (loaded, created) = Signer::load_or_create(&dir, "S-workstation").unwrap();
        assert!(!created);
        assert_eq!(loaded.public_key_b64(), known.public_key_b64());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_key_file_whose_halves_disagree_or_is_the_wrong_size_is_refused() {
        let dir = temp_dir("bad");
        fs::create_dir_all(&dir).unwrap();
        fs::write(Signer::path(&dir), [7u8; 64]).unwrap();
        let err = Signer::load_or_create(&dir, "S-workstation").err().unwrap();
        assert!(err.contains("does not match"), "{err}");
        fs::write(Signer::path(&dir), [7u8; 10]).unwrap();
        let err = Signer::load_or_create(&dir, "S-workstation").err().unwrap();
        assert!(err.contains("expected 64 bytes"), "{err}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_key_is_never_written_over_one_that_is_there() {
        let dir = temp_dir("keep-once");
        let (first, _) = Signer::load_or_create(&dir, "S-workstation").unwrap();
        let again = Signer::from_seed(&seed(), "S-workstation").unwrap();
        let err = again.keep(&Signer::path(&dir), &seed()).err().unwrap();
        assert!(!err.is_empty());
        let (still, _) = Signer::load_or_create(&dir, "S-workstation").unwrap();
        assert_eq!(first.public_key_b64(), still.public_key_b64());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_key_id_is_the_station_id_with_the_suffix() {
        assert_eq!(Signer::key_id_for("LN-TG-001"), "LN-TG-001-workstation");
    }
}
