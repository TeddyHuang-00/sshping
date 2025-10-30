use std::{
    io::{self, IsTerminal},
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
    
    // Try to decode the key with the provided password first
    let mut key_result = decode_secret_key(&key_content, password);
    
    // If decoding fails and we're in an interactive terminal, prompt for passphrase
    if key_result.is_err() && password.is_none() && io::stdin().is_terminal() {
        eprintln!("Enter passphrase for key '{}': ", identity.display());
        if let Ok(passphrase) = rpassword::read_password()
            && !passphrase.is_empty() {
                key_result = decode_secret_key(&key_content, Some(&passphrase));
            }
    }
    
    let key = key_result.map_err(|e| format!("Failed to decode secret key: {e}"))?;
    
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

    // Try password authentication with provided password
    if let Some(pwd) = password
        && authenticate_password(session, user, pwd, timeout)
            .await
            .inspect_err(|e| warn!("{e}"))
            .is_ok()
    {
        return Ok(start.elapsed());
    }

    // If no password was provided and we're in an interactive terminal, prompt for one
    if password.is_none() && io::stdin().is_terminal() {
        eprintln!("{}'s password: ", user);
        if let Ok(pwd) = rpassword::read_password()
            && !pwd.is_empty()
                && authenticate_password(session, user, &pwd, timeout)
                    .await
                    .inspect_err(|e| warn!("{e}"))
                    .is_ok()
            {
                return Ok(start.elapsed());
            }
    }

    // Fails if all authentication methods fail
    Err("All authentication methods failed")
}
