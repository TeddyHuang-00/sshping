use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use log::{info, warn};
use russh::client;
use russh::keys::{decode_secret_key, PrivateKeyWithHashAlg};

pub async fn authenticate_all<H: client::Handler>(
    session: &mut client::Handle<H>,
    user: &str,
    password: Option<&str>,
    identity: Option<&PathBuf>,
    timeout: f64,
) -> Result<Duration, &'static str> {
    let start = Instant::now();

    // Try public key authentication if identity file is provided
    if let Some(identity_path) = identity {
        match std::fs::read_to_string(identity_path) {
            Ok(key_content) => {
                match decode_secret_key(&key_content, password) {
                    Ok(key) => {
                        match tokio::time::timeout(
                            Duration::from_secs_f64(timeout),
                            session.authenticate_publickey(
                                user,
                                PrivateKeyWithHashAlg::new(Arc::new(key), None),
                            ),
                        )
                        .await
                        {
                            Ok(Ok(auth_result)) => {
                                if auth_result.success() {
                                    info!("Public key authentication succeeded");
                                    return Ok(start.elapsed());
                                } else {
                                    warn!("Public key authentication returned false");
                                }
                            }
                            Ok(Err(e)) => warn!("Public key authentication failed: {e}"),
                            Err(_) => warn!("Public key authentication timed out"),
                        }
                    }
                    Err(e) => warn!("Failed to decode secret key: {e}"),
                }
            }
            Err(e) => warn!("Failed to read identity file: {e}"),
        }
    }

    // Try password authentication
    if let Some(pwd) = password {
        match tokio::time::timeout(
            Duration::from_secs_f64(timeout),
            session.authenticate_password(user, pwd),
        )
        .await
        {
            Ok(Ok(auth_result)) => {
                if auth_result.success() {
                    info!("Password authentication succeeded");
                    return Ok(start.elapsed());
                } else {
                    warn!("Password authentication returned false");
                }
            }
            Ok(Err(e)) => warn!("Password authentication failed: {e}"),
            Err(_) => warn!("Password authentication timed out"),
        }
    }

    // Fails if all authentication methods fail
    Err("All authentication methods failed")
}
