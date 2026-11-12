//! Registering the station's signing key with the ERP, once.
//!
//! The ERP refuses a second registration of a key ID. So a marker file beside
//! the key records that this station's key was registered, and a refusal for
//! a key made just now means a different key holds that ID: nothing signed
//! with the new one would verify.

use std::path::{Path, PathBuf};

use crate::client::{Client, ErpError};
use crate::signing::Signer;

const MARKER_FILE: &str = "workstation_signing_key.registered";

/// What [`ensure`] found or did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Registration {
    /// Noted as registered already; the ERP was not asked.
    Known,
    /// Registered now. `Pending` means an administrator must activate it
    /// before results verify.
    Registered { status: String },
    /// The ERP already holds a key under this ID, and this key file was not
    /// new (copied from another station, or a first run that did not finish):
    /// taken to be this one's.
    HeldAlready,
}

pub fn marker_path(dir: &Path) -> PathBuf {
    dir.join(MARKER_FILE)
}

/// Is this station's key noted as registered?
pub fn is_registered(dir: &Path, signer: &Signer) -> bool {
    is_registered_as(dir, signer.key_id())
}

/// Is a key with this ID noted as registered? Reads only the marker, so a
/// window can ask every frame without loading the key.
pub fn is_registered_as(dir: &Path, key_id: &str) -> bool {
    std::fs::read_to_string(marker_path(dir)).is_ok_and(|t| t.trim() == key_id)
}

/// Registers the key with the ERP unless this station already has.
/// `created` is true when the key was made just now.
pub fn ensure(
    client: &Client,
    signer: &Signer,
    dir: &Path,
    created: bool,
) -> Result<Registration, String> {
    if is_registered(dir, signer) {
        return Ok(Registration::Known);
    }
    let outcome = match client.register_key(signer.key_id(), &signer.public_key_b64()) {
        Ok(reply) => Registration::Registered {
            status: reply["status"]
                .as_str()
                .unwrap_or("status unknown")
                .to_string(),
        },
        Err(ErpError::Status { body, .. }) if body.contains("Key ID already exists") => {
            if created {
                return Err(format!(
                    "the ERP already holds a different key named {}. Results signed with this new key would not verify; an administrator must revoke the old one.",
                    signer.key_id()
                ));
            }
            Registration::HeldAlready
        }
        Err(e) => return Err(format!("could not register the signing key: {e}")),
    };
    let marker = marker_path(dir);
    std::fs::write(&marker, signer.key_id()).map_err(|e| format!("{}: {e}", marker.display()))?;
    Ok(outcome)
}
