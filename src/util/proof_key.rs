use sodiumoxide::crypto::hash::sha256;

pub fn verify_challenge(challenge: &str, verifier: &str) -> Result<(), anyhow::Error> {
    let verifier_digest = sha256::hash(verifier.as_bytes());
    let verifier_hash = base64::encode_config(verifier_digest.as_ref(), base64::URL_SAFE_NO_PAD);
    if verifier_hash == challenge {
        Ok(())
    } else {
        anyhow::bail!("Challenge was not successfully verified.")
    }
}