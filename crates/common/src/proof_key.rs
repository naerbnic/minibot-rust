//! Implements the SHA256 algorithm compatible with PKCE for Oauth.
//!
//! This allows us to do a key token and retrieval step safely, by ensuring an attacker would need
//! to have direct access to a program's memory to get at the token.
//!
//! For details, see RFC 7636 (https://tools.ietf.org/html/rfc7636)

use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sodiumoxide::crypto::hash::sha256;

#[derive(thiserror::Error, Debug)]
#[error("Failed to verify the challenge.")]
pub struct VerifyError;

fn generate_challenge(verifier: &str) -> String {
    let verifier_digest = sha256::hash(verifier.as_bytes());
    base64::encode_config(verifier_digest.as_ref(), base64::URL_SAFE_NO_PAD)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Challenge(String);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Verifier(String);

impl Verifier {
    pub fn verify(&self, challenge: &Challenge) -> Result<(), VerifyError> {
        if generate_challenge(&self.0) == challenge.0 {
            Ok(())
        } else {
            Err(VerifyError)
        }
    }
}

pub fn generate_pair() -> (Challenge, Verifier) {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes[..]);
    let verifier = base64::encode_config(&bytes[..], base64::URL_SAFE_NO_PAD);

    let challenge = generate_challenge(&verifier);
    (Challenge(challenge), Verifier(verifier))
}
