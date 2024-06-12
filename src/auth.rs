use log::{debug, info, warn};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use ssh2::Session;

pub fn authenticate_all(
    session: &Session,
    user: &str,
    password: Option<&str>,
    identity: Option<&PathBuf>,
) -> Result<Duration, String> {
    let methods = session
        .auth_methods(user)
        .unwrap()
        .split(",")
        .collect::<Vec<&str>>();
    debug!("Available authentication methods: {methods:?}");
    // Try all authentication methods in order of preference
    let now = Instant::now();
    match session.userauth_agent(user) {
        Ok(_) => {
            info!("Agent authentication succeeded");
            return Ok(now.elapsed());
        }
        Err(e) => warn!("Agent authentication failed: {e}"),
    }
    if let Some(identity) = identity {
        if !methods.contains(&"publickey") {
            warn!("Public key authentication not supported on server");
        } else {
            let now = Instant::now();
            match session.userauth_pubkey_file(user, None, identity, password) {
                Ok(_) => {
                    info!("Public key authentication succeeded");
                    return Ok(now.elapsed());
                }
                Err(e) => warn!("Pubkey authentication failed: {e}"),
            }
        }
    }
    if !methods.contains(&"password") {
        warn!("Password authentication not supported on server");
    } else {
        let now = Instant::now();
        match session.userauth_password(user, password.unwrap_or_default()) {
            Ok(_) => {
                info!("Password authentication succeeded");
                return Ok(now.elapsed());
            }
            Err(e) => warn!("Password authentication failed: {e}"),
        }
    }
    // Fails if all authentication methods fail
    Err("All authentication methods failed".to_string())
}
