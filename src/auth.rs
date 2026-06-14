use std::{
    env, fmt,
    io::{self, IsTerminal},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use log::{debug, info, warn};
use russh::{
    client,
    keys::{agent::client::AgentClient, decode_secret_key, PrivateKeyWithHashAlg},
};

#[derive(Debug)]
pub enum AuthError {
    ReadIdentityFile(String),
    DecodeSecretKey(String),
    RsaHash(String),
    PublicKeyTimeout(f64),
    PublicKeyFailed(String),
    PublicKeyRejected,
    PasswordTimeout(f64),
    PasswordFailed(String),
    PasswordRejected,
    AgentConnect(String),
    AgentNoIdentities,
    AgentTimeout(f64),
    AgentFailed(String),
    AllMethodsFailed,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadIdentityFile(msg) => write!(f, "Failed to read identity file: {msg}"),
            Self::DecodeSecretKey(msg) => write!(f, "Failed to decode secret key: {msg}"),
            Self::RsaHash(msg) => write!(f, "Failed to get RSA hash algorithm: {msg}"),
            Self::PublicKeyTimeout(timeout) => {
                write!(
                    f,
                    "Public key authentication timed out after {timeout} seconds"
                )
            }
            Self::PublicKeyFailed(msg) => write!(f, "Public key authentication failed: {msg}"),
            Self::PublicKeyRejected => write!(f, "Public key authentication returned false"),
            Self::PasswordTimeout(timeout) => {
                write!(
                    f,
                    "Password authentication timed out after {timeout} seconds"
                )
            }
            Self::PasswordFailed(msg) => write!(f, "Password authentication failed: {msg}"),
            Self::PasswordRejected => write!(f, "Password authentication returned false"),
            Self::AgentConnect(msg) => write!(f, "Failed to connect to SSH agent: {msg}"),
            Self::AgentNoIdentities => {
                write!(f, "SSH agent returned no identities")
            }
            Self::AgentTimeout(timeout) => {
                write!(
                    f,
                    "SSH agent authentication timed out after {timeout} seconds"
                )
            }
            Self::AgentFailed(msg) => write!(f, "SSH agent authentication failed: {msg}"),
            Self::AllMethodsFailed => write!(f, "All authentication methods failed"),
        }
    }
}

impl std::error::Error for AuthError {}

async fn authenticate_publickey<H: client::Handler>(
    session: &mut client::Handle<H>,
    user: &str,
    identity: &Path,
    password: Option<&str>,
    timeout: f64,
) -> Result<(), AuthError> {
    let key_content = std::fs::read_to_string(identity)
        .map_err(|e| AuthError::ReadIdentityFile(e.to_string()))?;

    // Try to decode the key with the provided password first
    let mut key_result = decode_secret_key(&key_content, password);

    // If decoding fails and we're in an interactive terminal, prompt for passphrase
    if key_result.is_err() && password.is_none() && io::stdin().is_terminal() {
        eprint!("Enter passphrase for key '{}': ", identity.display());
        if let Ok(passphrase) = rpassword::read_password()
            && !passphrase.is_empty()
        {
            key_result = decode_secret_key(&key_content, Some(&passphrase));
        }
    }

    let key = key_result.map_err(|e| AuthError::DecodeSecretKey(e.to_string()))?;

    // Get the best supported RSA hash algorithm for the connection
    let rsa_hash = session
        .best_supported_rsa_hash()
        .await
        .map_err(|e| AuthError::RsaHash(e.to_string()))?
        .flatten();

    let timeout_result = tokio::time::timeout(
        Duration::from_secs_f64(timeout),
        session.authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), rsa_hash)),
    )
    .await
    .map_err(|_| AuthError::PublicKeyTimeout(timeout))?;
    let auth_result = timeout_result.map_err(|e| AuthError::PublicKeyFailed(e.to_string()))?;
    if !auth_result.success() {
        return Err(AuthError::PublicKeyRejected);
    }

    info!("Public key authentication succeeded");
    Ok(())
}

fn discover_default_identity_files() -> Vec<PathBuf> {
    let Some(home) = env::var_os("HOME") else {
        return vec![];
    };
    let ssh_dir = PathBuf::from(home).join(".ssh");
    [
        "id_ed25519",
        "id_ecdsa",
        "id_ecdsa_sk",
        "id_rsa",
        "id_dsa",
        "id_xmss",
        "identity",
    ]
    .iter()
    .map(|name| ssh_dir.join(name))
    .filter(|path| path.exists() && path.is_file())
    .collect()
}

async fn authenticate_password<H: client::Handler>(
    session: &mut client::Handle<H>,
    user: &str,
    password: &str,
    timeout: f64,
) -> Result<(), AuthError> {
    let timeout_result = tokio::time::timeout(
        Duration::from_secs_f64(timeout),
        session.authenticate_password(user, password),
    )
    .await
    .map_err(|_| AuthError::PasswordTimeout(timeout))?;
    let auth_result = timeout_result.map_err(|e| AuthError::PasswordFailed(e.to_string()))?;
    if !auth_result.success() {
        return Err(AuthError::PasswordRejected);
    }

    info!("Password authentication succeeded");
    Ok(())
}

async fn authenticate_agent<H: client::Handler>(
    session: &mut client::Handle<H>,
    user: &str,
    timeout: f64,
) -> Result<(), AuthError> {
    // Connect to the SSH agent (platform-specific)
    #[cfg(unix)]
    let mut agent = {
        AgentClient::connect_env()
            .await
            .map_err(|e| AuthError::AgentConnect(e.to_string()))?
            .dynamic()
    };

    #[cfg(windows)]
    let mut agent = {
        // Try named pipe first, fall back to Pageant
        match AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent").await {
            Ok(client) => client.dynamic(),
            Err(_) => AgentClient::connect_pageant()
                .await
                .map_err(|e| AuthError::AgentConnect(e.to_string()))?
                .dynamic(),
        }
    };

    // Request identities from the agent
    let identities = agent
        .request_identities()
        .await
        .map_err(|e| AuthError::AgentFailed(e.to_string()))?;

    if identities.is_empty() {
        return Err(AuthError::AgentNoIdentities);
    }

    // Get the best supported RSA hash algorithm for the connection
    let rsa_hash = session
        .best_supported_rsa_hash()
        .await
        .map_err(|e| AuthError::AgentFailed(e.to_string()))?
        .flatten();

    // Try each identity
    for identity in &identities {
        let public_key = identity.public_key().into_owned();
        let timeout_result = tokio::time::timeout(
            Duration::from_secs_f64(timeout),
            session.authenticate_publickey_with(
                user.to_string(),
                public_key,
                rsa_hash,
                &mut agent,
            ),
        )
        .await
        .map_err(|_| AuthError::AgentTimeout(timeout))?;
        let auth_result = timeout_result.map_err(|e| AuthError::AgentFailed(e.to_string()))?;
        if auth_result.success() {
            debug!("SSH agent authentication succeeded");
            return Ok(());
        }
    }

    Err(AuthError::AgentFailed(
        "No agent identity was accepted by server".to_string(),
    ))
}

pub async fn authenticate_all<H: client::Handler>(
    session: &mut client::Handle<H>,
    user: &str,
    host: &str,
    password: Option<&str>,
    identity: Option<&Path>,
    timeout: f64,
    use_agent: bool,
) -> Result<Duration, AuthError> {
    let start = Instant::now();

    // Try SSH agent authentication first (if enabled)
    if use_agent
        && authenticate_agent(session, user, timeout)
            .await
            .inspect_err(|e| debug!("SSH agent authentication failed: {e}"))
            .is_ok()
    {
        return Ok(start.elapsed());
    }

    // Try public key authentication if identity file is provided
    if let Some(identity_path) = identity
        && authenticate_publickey(session, user, identity_path, password, timeout)
            .await
            .inspect_err(|e| warn!("{e}"))
            .is_ok()
    {
        return Ok(start.elapsed());
    }

    if identity.is_none() {
        let discovered_identities = discover_default_identity_files();
        if !discovered_identities.is_empty() {
            debug!(
                "No explicit identity provided for {user}@{host}, trying default identities from ~/.ssh"
            );
        }
        for discovered_identity in discovered_identities {
            if authenticate_publickey(session, user, &discovered_identity, password, timeout)
                .await
                .inspect_err(|e| warn!("{e}"))
                .is_ok()
            {
                return Ok(start.elapsed());
            }
        }
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

    // If no password was provided and we're in an interactive terminal,
    // prompt for one
    if password.is_none() && io::stdin().is_terminal() {
        eprint!("Password for {user}@{host}: ");
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
    Err(AuthError::AllMethodsFailed)
}
