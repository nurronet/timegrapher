//! The timegrapher's link to the LN ERP.
//!
//! A bench station that uses the ERP does five things: connect with an API
//! key ([`config`], [`client`]); find what is on the bench ([`client::Client::lookup_identity`]);
//! sign an operator in with a PIN ([`operator`]); send each position's figures
//! signed with the station's own key ([`payload`], [`signing`], [`submit`]);
//! and complete the session. The bytes that are signed are the ERP's
//! `LN-CANONICAL-JSON-1` ([`canonical`]).
//!
//! This crate decides no reading. Every figure it sends comes from the
//! `timegrapher session` summary, by core's rules.

pub mod canonical;
pub mod client;
pub mod config;
pub mod operator;
pub mod payload;
pub mod register;
pub mod signing;
pub mod submit;

#[cfg(feature = "testing")]
pub mod testing;
