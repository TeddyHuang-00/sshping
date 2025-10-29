use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use log::{info, warn};
use russh::{
    client,
    keys::{decode_secret_key, PrivateKeyWithHashAlg},
};

async fn authenticate_publickey<H: client::Handler>(
    session: &mut client::Handle<H>,
    user: &str,
    identity: &PathBuf,
    password: Option<&str>,
    timeout: f64,
) -> Result<(), String> {
    let key_content = std::fs::read_to_string(identity)
        .map_err(|e| format!("Failed to read identity file: {e}"))?;
    let key = decode_secret_key(&key_content, password)
        .map_err(|e| format!("Failed to decode secret key: {e}"))?;

    // Get the best supported RSA hash algorithm for the connection
    let rsa_hash = session
        .best_supported_rsa_hash()
        .await
        .map_err(|e| format!("Failed to get RSA hash algorithm: {e}"))?
        .flatten();

    let timeout_result = tokio::time::timeout(
        Duration::from_secs_f64(timeout),
        session.authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), rsa_hash)),
    )
    .await
    .map_err(|_| format!("Public key authentication timed out after {timeout} seconds"))?;
    let auth_result =
        timeout_result.map_err(|e| format!("Public key authentication failed: {e}"))?;
    if !auth_result.success() {
        return Err("Public key authentication returned false".to_string());
    }

    info!("Public key authentication succeeded");
    Ok(())
}

async fn authenticate_password<H: client::Handler>(
    session: &mut client::Handle<H>,
    user: &str,
    password: &str,
    timeout: f64,
) -> Result<(), String> {
    let timeout_result = tokio::time::timeout(
        Duration::from_secs_f64(timeout),
        session.authenticate_password(user, password),
    )
    .await
    .map_err(|_| format!("Password authentication timed out after {timeout} seconds"))?;
    let auth_result = timeout_result.map_err(|e| format!("Password authentication failed: {e}"))?;
    if !auth_result.success() {
        return Err("Password authentication returned false".to_string());
    }

    info!("Password authentication succeeded");
    Ok(())
}

pub async fn authenticate_all<H: client::Handler>(
    session: &mut client::Handle<H>,
    user: &str,
    password: Option<&str>,
    identity: Option<&PathBuf>,
    timeout: f64,
) -> Result<Duration, &'static str> {
    let start = Instant::now();

    // Try public key authentication if identity file is provided
    if let Some(identity_path) = identity
        && authenticate_publickey(session, user, identity_path, password, timeout)
            .await
            .inspect_err(|e| warn!("{e}"))
            .is_ok()
    {
        return Ok(start.elapsed());
    }

    // Try password authentication
    if let Some(pwd) = password
        && authenticate_password(session, user, pwd, timeout)
            .await
            .inspect_err(|e| warn!("{e}"))
            .is_ok()
    {
        return Ok(start.elapsed());
    }

    // Fails if all authentication methods fail
    Err("All authentication methods failed")
}
