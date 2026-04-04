use std::{path::PathBuf, sync::Arc};

use log::debug;
use russh::client;
use russh_config::parse_path;

use crate::{auth::authenticate_all, cli::Options};

pub struct SshHandler;

impl client::Handler for SshHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

#[derive(Clone, Debug)]
pub struct Endpoint {
    pub user: String,
    pub host: String,
    pub port: u16,
    pub identity: Option<PathBuf>,
    pub identity_source: &'static str,
    pub user_source: &'static str,
}

#[derive(Clone, Debug)]
struct JumpSpec {
    user: Option<String>,
    host: String,
    port: Option<u16>,
}

#[derive(Clone, Debug)]
pub struct ResolvedConnection {
    pub target_endpoint: Endpoint,
    pub proxy_jump: Option<String>,
    pub proxy_command: Option<String>,
}

pub fn resolve_identity(
    host_config_identity: Option<PathBuf>,
    cli_identity: Option<&PathBuf>,
    host_config_source: &'static str,
) -> (Option<PathBuf>, &'static str) {
    if let Some(identity) = host_config_identity {
        (Some(identity), host_config_source)
    } else if let Some(identity) = cli_identity {
        (Some(identity.clone()), "cli identity")
    } else {
        (None, "fallback")
    }
}

pub fn resolve_user(
    host_config_user: Option<String>,
    inherited_user: &str,
    explicit_user: Option<&str>,
    host_config_source: &'static str,
    explicit_source: &'static str,
) -> (String, &'static str) {
    if let Some(user) = explicit_user {
        (user.to_string(), explicit_source)
    } else if let Some(user) = host_config_user {
        (user, host_config_source)
    } else {
        (inherited_user.to_string(), "inherited/default")
    }
}

pub fn resolve_target_and_proxies(opts: &mut Options) -> Result<ResolvedConnection, String> {
    let mut proxy_jump = None;
    let mut proxy_command = None;
    let mut target_identity_from_config = None;
    let mut target_user_from_config = false;
    let cli_identity = opts.identity.clone();

    if let Some(ssh_config) = &opts.config
        && ssh_config.exists()
    {
        debug!("SSH Config: {:?}", ssh_config);
        let config = parse_path(ssh_config, opts.target.host.as_str())
            .map_err(|e| format!("Failed to parse configuration: {e}"))?;

        let config_host = config.host();
        if !config_host.is_empty() {
            opts.target.host = config_host.to_string();
        }
        if let Some(user) = config.host_config.user {
            opts.target.user = user;
            target_user_from_config = true;
        }
        if let Some(port) = config.host_config.port {
            opts.target.port = port;
        }
        if let Some(identity) = config
            .host_config
            .identity_file
            .and_then(|files| files.first().cloned())
        {
            target_identity_from_config = Some(identity);
        }
        proxy_jump = config.host_config.proxy_jump;
        proxy_command = config.host_config.proxy_command;
    }

    let (target_identity, target_identity_source) =
        resolve_identity(target_identity_from_config, cli_identity.as_ref(), "target host config");
    let target_user_source = if target_user_from_config {
        "target host config"
    } else {
        "target/default"
    };
    let target_endpoint = Endpoint {
        user: opts.target.user.clone(),
        host: opts.target.host.clone(),
        port: opts.target.port,
        identity: target_identity,
        identity_source: target_identity_source,
        user_source: target_user_source,
    };

    Ok(ResolvedConnection {
        target_endpoint,
        proxy_jump,
        proxy_command,
    })
}

pub async fn establish_authenticated_session(
    resolved: &ResolvedConnection,
    ssh_config: Option<&PathBuf>,
    timeout: f64,
    password: Option<&str>,
    cli_identity: Option<&PathBuf>,
) -> Result<client::Handle<SshHandler>, String> {
    let connector = SessionConnector::new(timeout, password);

    if let Some(proxy_jump) = &resolved.proxy_jump {
        let jumps = parse_proxy_jump(proxy_jump.as_str())?;
        if jumps.is_empty() {
            return connector
                .connect_direct_auth(&resolved.target_endpoint)
                .await;
        }

        let default_user = resolved.target_endpoint.user.clone();
        let first_jump = endpoint_from_jump_spec(
            jumps.first().ok_or("ProxyJump list became empty unexpectedly")?,
            ssh_config,
            default_user.as_str(),
            cli_identity,
        )?;
        let first_session = connector.connect_direct_auth(&first_jump).await?;
        let mut jump_sessions: Vec<client::Handle<SshHandler>> = vec![first_session];

        for jump_spec in jumps.iter().skip(1) {
            let endpoint =
                endpoint_from_jump_spec(jump_spec, ssh_config, default_user.as_str(), cli_identity)?;
            let Some(last_jump) = jump_sessions.last_mut() else {
                return Err("No jump session available while establishing ProxyJump chain".to_string());
            };
            let jump_session = connector
                .connect_through_jump_auth(last_jump, &endpoint)
                .await?;
            jump_sessions.push(jump_session);
        }

        let Some(last_jump) = jump_sessions.last_mut() else {
            return Err("No jump session available for final target connection".to_string());
        };
        return connector
            .connect_through_jump_auth(last_jump, &resolved.target_endpoint)
            .await;
    }

    if resolved.proxy_command.is_some() && ssh_config.is_some_and(|c| c.exists()) {
        return connector
            .connect_proxy_command_auth(
                ssh_config.expect("checked exists"),
                &resolved.target_endpoint,
            )
            .await;
    }

    connector
        .connect_direct_auth(&resolved.target_endpoint)
        .await
}

struct SessionConnector<'a> {
    timeout: f64,
    password: Option<&'a str>,
}

impl<'a> SessionConnector<'a> {
    fn new(timeout: f64, password: Option<&'a str>) -> Self {
        Self { timeout, password }
    }

    async fn authenticate_connected(
        &self,
        endpoint: &Endpoint,
        mut session: client::Handle<SshHandler>,
    ) -> Result<client::Handle<SshHandler>, String> {
        debug!(
            "Credentials for {}@{}:{} => user source: {}, identity source: {}",
            endpoint.user, endpoint.host, endpoint.port, endpoint.user_source, endpoint.identity_source
        );
        authenticate_all(
            &mut session,
            &endpoint.user,
            &endpoint.host,
            self.password,
            endpoint.identity.as_ref(),
            self.timeout,
        )
        .await
        .map_err(ToString::to_string)?;
        Ok(session)
    }

    async fn connect_direct_auth(&self, endpoint: &Endpoint) -> Result<client::Handle<SshHandler>, String> {
        let session = connect_direct(endpoint, self.timeout).await?;
        self.authenticate_connected(endpoint, session).await
    }

    async fn connect_proxy_command_auth(
        &self,
        ssh_config: &PathBuf,
        endpoint: &Endpoint,
    ) -> Result<client::Handle<SshHandler>, String> {
        let session = connect_with_proxy_command(ssh_config, endpoint, self.timeout).await?;
        self.authenticate_connected(endpoint, session).await
    }

    async fn connect_through_jump_auth(
        &self,
        jump: &mut client::Handle<SshHandler>,
        endpoint: &Endpoint,
    ) -> Result<client::Handle<SshHandler>, String> {
        let session = connect_through_jump(jump, endpoint, self.timeout).await?;
        self.authenticate_connected(endpoint, session).await
    }
}

fn parse_proxy_jump(proxy_jump: &str) -> Result<Vec<JumpSpec>, String> {
    proxy_jump
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty() && !item.eq_ignore_ascii_case("none"))
        .map(|item| {
            let (user, host_port) = if let Some((user, host_port)) = item.rsplit_once('@') {
                if user.is_empty() {
                    return Err(format!("Invalid ProxyJump user in target: {item}"));
                }
                (Some(user.to_string()), host_port)
            } else {
                (None, item)
            };
            let (host, port) = if host_port.starts_with('[') {
                let close = host_port
                    .find(']')
                    .ok_or_else(|| format!("Invalid bracketed ProxyJump host: {item}"))?;
                let host = &host_port[1..close];
                let rest = &host_port[close + 1..];
                let port = if rest.is_empty() {
                    None
                } else if let Some(port_str) = rest.strip_prefix(':') {
                    Some(
                        port_str
                            .parse::<u16>()
                            .map_err(|e| format!("Invalid ProxyJump port in {item}: {e}"))?,
                    )
                } else {
                    return Err(format!("Invalid ProxyJump host/port format: {item}"));
                };
                (host.to_string(), port)
            } else if host_port.matches(':').count() > 1 {
                (host_port.to_string(), None)
            } else if let Some((host, port)) = host_port.rsplit_once(':') {
                let port = port
                    .parse::<u16>()
                    .map_err(|e| format!("Invalid ProxyJump port in {item}: {e}"))?;
                (host.to_string(), Some(port))
            } else {
                (host_port.to_string(), None)
            };
            if host.is_empty() {
                return Err(format!("Invalid ProxyJump host: {item}"));
            }
            Ok(JumpSpec { user, host, port })
        })
        .collect()
}

fn endpoint_from_jump_spec(
    spec: &JumpSpec,
    ssh_config: Option<&PathBuf>,
    default_user: &str,
    cli_identity: Option<&PathBuf>,
) -> Result<Endpoint, String> {
    let mut host = spec.host.clone();
    let mut host_config_user = None;
    let mut port = 22;
    let mut host_config_identity = None;

    if let Some(ssh_config) = ssh_config
        && ssh_config.exists()
    {
        let config = parse_path(ssh_config, spec.host.as_str())
            .map_err(|e| format!("Failed to parse jump host configuration: {e}"))?;
        let config_host = config.host();
        if !config_host.is_empty() {
            host = config_host.to_string();
        }
        host_config_user = config.host_config.user.clone();
        port = config.port();
        host_config_identity = config
            .host_config
            .identity_file
            .and_then(|files| files.first().cloned());
    }

    let (user, user_source) = resolve_user(
        host_config_user,
        default_user,
        spec.user.as_deref(),
        "proxy host config",
        "jump spec",
    );
    if let Some(spec_port) = spec.port {
        port = spec_port;
    }
    let (identity, identity_source) =
        resolve_identity(host_config_identity, cli_identity, "proxy host config");

    Ok(Endpoint {
        user,
        host,
        port,
        identity,
        identity_source,
        user_source,
    })
}

async fn connect_direct(endpoint: &Endpoint, timeout: f64) -> Result<client::Handle<SshHandler>, String> {
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(std::time::Duration::from_secs_f64(timeout)),
        ..Default::default()
    });
    let addr = (endpoint.host.as_str(), endpoint.port);
    match tokio::time::timeout(
        std::time::Duration::from_secs_f64(timeout),
        client::connect(config, addr, SshHandler),
    )
    .await
    {
        Ok(Ok(session)) => Ok(session),
        Ok(Err(e)) => Err(format!("Failed to connect to {}: {e}", endpoint.host)),
        Err(_) => Err(format!("Connection timeout when connecting to {}", endpoint.host)),
    }
}

async fn connect_with_proxy_command(
    ssh_config: &PathBuf,
    endpoint: &Endpoint,
    timeout: f64,
) -> Result<client::Handle<SshHandler>, String> {
    let parsed = parse_path(ssh_config, endpoint.host.as_str())
        .map_err(|e| format!("Failed to parse SSH config for ProxyCommand: {e}"))?;
    let stream = parsed
        .stream()
        .await
        .map_err(|e| format!("Failed to connect through ProxyCommand: {e}"))?;
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(std::time::Duration::from_secs_f64(timeout)),
        ..Default::default()
    });
    match tokio::time::timeout(
        std::time::Duration::from_secs_f64(timeout),
        client::connect_stream(config, stream, SshHandler),
    )
    .await
    {
        Ok(Ok(session)) => Ok(session),
        Ok(Err(e)) => Err(format!(
            "Failed to establish stream connection through ProxyCommand to {}: {e}",
            endpoint.host
        )),
        Err(_) => Err(format!(
            "Connection timeout when stream-connecting through ProxyCommand to {}",
            endpoint.host
        )),
    }
}

async fn connect_through_jump(
    jump: &mut client::Handle<SshHandler>,
    endpoint: &Endpoint,
    timeout: f64,
) -> Result<client::Handle<SshHandler>, String> {
    let direct_channel = jump
        .channel_open_direct_tcpip(endpoint.host.clone(), endpoint.port as u32, "127.0.0.1", 0)
        .await
        .map_err(|e| format!("Failed to open direct-tcpip channel to {}: {e}", endpoint.host))?;
    let stream = direct_channel.into_stream();
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(std::time::Duration::from_secs_f64(timeout)),
        ..Default::default()
    });
    match tokio::time::timeout(
        std::time::Duration::from_secs_f64(timeout),
        client::connect_stream(config, stream, SshHandler),
    )
    .await
    {
        Ok(Ok(session)) => Ok(session),
        Ok(Err(e)) => Err(format!("Failed to establish stream connection to {}: {e}", endpoint.host)),
        Err(_) => Err(format!("Connection timeout when stream-connecting to {}", endpoint.host)),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_proxy_jump, resolve_identity, resolve_user};
    use std::path::PathBuf;

    #[test]
    fn identity_prefers_host_config_then_cli_then_fallback() {
        let host_cfg = PathBuf::from("/tmp/proxy_key");
        let cli = PathBuf::from("/tmp/cli_key");

        let (identity, source) =
            resolve_identity(Some(host_cfg.clone()), Some(&cli), "proxy host config");
        assert_eq!(identity, Some(host_cfg));
        assert_eq!(source, "proxy host config");

        let (identity, source) = resolve_identity(None, Some(&cli), "proxy host config");
        assert_eq!(identity, Some(cli));
        assert_eq!(source, "cli identity");

        let (identity, source) = resolve_identity(None, None, "proxy host config");
        assert_eq!(identity, None);
        assert_eq!(source, "fallback");
    }

    #[test]
    fn user_prefers_explicit_then_host_config_then_inherited() {
        let (user, source) = resolve_user(
            Some("cfg-user".to_string()),
            "default-user",
            Some("jump-user"),
            "proxy host config",
            "jump spec",
        );
        assert_eq!(user, "jump-user");
        assert_eq!(source, "jump spec");

        let (user, source) = resolve_user(
            Some("cfg-user".to_string()),
            "default-user",
            None,
            "proxy host config",
            "jump spec",
        );
        assert_eq!(user, "cfg-user");
        assert_eq!(source, "proxy host config");

        let (user, source) = resolve_user(
            None,
            "default-user",
            None,
            "proxy host config",
            "jump spec",
        );
        assert_eq!(user, "default-user");
        assert_eq!(source, "inherited/default");
    }

    #[test]
    fn proxy_jump_parses_ipv6_and_port_forms() {
        let parsed = parse_proxy_jump("[::1]:2222,fe80::1,jump.example:2200").unwrap();
        assert_eq!(parsed.len(), 3);

        assert_eq!(parsed[0].host, "::1");
        assert_eq!(parsed[0].port, Some(2222));

        assert_eq!(parsed[1].host, "fe80::1");
        assert_eq!(parsed[1].port, None);

        assert_eq!(parsed[2].host, "jump.example");
        assert_eq!(parsed[2].port, Some(2200));
    }
}
