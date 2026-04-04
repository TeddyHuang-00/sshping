use std::{fmt, path::PathBuf, sync::Arc, time::Duration};

use log::debug;
use russh::client;
use russh_config::parse_path;

use crate::{auth::authenticate_all, cli::Options};

type Result<T> = std::result::Result<T, ClientError>;

pub struct SshHandler;

impl client::Handler for SshHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        Ok(true)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Source {
    TargetHostConfig,
    ProxyHostConfig,
    CliIdentity,
    JumpSpec,
    TargetDefault,
    InheritedDefault,
    Fallback,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Source::TargetHostConfig => "target host config",
            Source::ProxyHostConfig => "proxy host config",
            Source::CliIdentity => "cli identity",
            Source::JumpSpec => "jump spec",
            Source::TargetDefault => "target/default",
            Source::InheritedDefault => "inherited/default",
            Source::Fallback => "fallback",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AuthSpec {
    pub user: String,
    pub identity: Option<PathBuf>,
    pub user_source: Source,
    pub identity_source: Source,
}

#[derive(Clone, Debug)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub auth: AuthSpec,
}

impl Endpoint {
    async fn authenticate(
        &self,
        mut session: client::Handle<SshHandler>,
        password: Option<&str>,
        timeout: f64,
    ) -> Result<client::Handle<SshHandler>> {
        debug!(
            "Credentials for {}@{}:{} => user source: {}, identity source: {}",
            self.auth.user,
            self.host,
            self.port,
            self.auth.user_source.as_str(),
            self.auth.identity_source.as_str()
        );
        authenticate_all(
            &mut session,
            &self.auth.user,
            &self.host,
            password,
            self.auth.identity.as_ref(),
            timeout,
        )
        .await
        .map_err(|e| ClientError::Auth(e.to_string()))?;
        Ok(session)
    }
}

#[derive(Clone, Debug)]
pub enum Route {
    Direct,
    ProxyCommand(PathBuf),
    ProxyJump(Vec<Endpoint>),
}

#[derive(Clone, Debug)]
pub struct ConnectionPlan {
    pub target: Endpoint,
    pub route: Route,
}

#[derive(Debug)]
pub enum ClientError {
    ConfigParse { context: &'static str, source: String },
    InvalidProxyJump(String),
    Connect { host: String, source: String },
    Timeout { context: &'static str, host: String },
    Auth(String),
    Route(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::ConfigParse { context, source } => {
                write!(f, "Failed to parse {context}: {source}")
            }
            ClientError::InvalidProxyJump(msg) => write!(f, "{msg}"),
            ClientError::Connect { host, source } => write!(f, "Failed to connect to {host}: {source}"),
            ClientError::Timeout { context, host } => {
                write!(f, "Connection timeout when {context} {host}")
            }
            ClientError::Auth(msg) => write!(f, "{msg}"),
            ClientError::Route(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ClientError {}

#[derive(Clone, Debug)]
struct JumpSpec {
    user: Option<String>,
    host: String,
    port: Option<u16>,
}

pub fn build_connection_plan(opts: &mut Options) -> Result<ConnectionPlan> {
    let cli_identity = opts.identity.clone();
    let mut proxy_jump = None;
    let mut proxy_command = None;
    let mut target_user_source = Source::TargetDefault;
    let mut target_identity_from_config = None;

    if let Some(ssh_config) = &opts.config
        && ssh_config.exists()
    {
        debug!("SSH Config: {:?}", ssh_config);
        let config = parse_path(ssh_config, opts.target.host.as_str()).map_err(|e| {
            ClientError::ConfigParse {
                context: "configuration",
                source: e.to_string(),
            }
        })?;
        if !config.host().is_empty() {
            opts.target.host = config.host().to_string();
        }
        if let Some(user) = config.host_config.user {
            opts.target.user = user;
            target_user_source = Source::TargetHostConfig;
        }
        if let Some(port) = config.host_config.port {
            opts.target.port = port;
        }
        target_identity_from_config = config
            .host_config
            .identity_file
            .and_then(|files| files.first().cloned());
        proxy_jump = config.host_config.proxy_jump;
        proxy_command = config.host_config.proxy_command;
    }

    let target_identity = resolve_identity(
        target_identity_from_config,
        cli_identity.as_ref(),
        Source::TargetHostConfig,
    );
    let target = Endpoint {
        host: opts.target.host.clone(),
        port: opts.target.port,
        auth: AuthSpec {
            user: opts.target.user.clone(),
            identity: target_identity.0,
            identity_source: target_identity.1,
            user_source: target_user_source,
        },
    };

    let route = build_route(
        proxy_jump.as_deref(),
        proxy_command.as_deref(),
        opts.config.as_ref(),
        &target.auth.user,
        cli_identity.as_ref(),
    )?;

    Ok(ConnectionPlan { target, route })
}

fn build_route(
    proxy_jump: Option<&str>,
    proxy_command: Option<&str>,
    ssh_config: Option<&PathBuf>,
    default_user: &str,
    cli_identity: Option<&PathBuf>,
) -> Result<Route> {
    if let Some(proxy_jump) = proxy_jump {
        let hops = parse_proxy_jump(proxy_jump)?
            .into_iter()
            .map(|spec| resolve_jump_endpoint(&spec, ssh_config, default_user, cli_identity))
            .collect::<Result<Vec<_>>>()?;
        return Ok(if hops.is_empty() {
            Route::Direct
        } else {
            Route::ProxyJump(hops)
        });
    }
    if proxy_command.is_some() && ssh_config.is_some_and(|cfg| cfg.exists()) {
        return Ok(Route::ProxyCommand(
            ssh_config.expect("checked exists").clone(),
        ));
    }
    Ok(Route::Direct)
}

pub async fn connect_plan(
    plan: &ConnectionPlan,
    timeout: f64,
    password: Option<&str>,
) -> Result<client::Handle<SshHandler>> {
    match &plan.route {
        Route::Direct => {
            let session = connect_via(Transport::Direct(&plan.target), timeout).await?;
            plan.target.authenticate(session, password, timeout).await
        }
        Route::ProxyCommand(ssh_config) => {
            let session =
                connect_via(Transport::ProxyCommand(ssh_config, &plan.target), timeout).await?;
            plan.target.authenticate(session, password, timeout).await
        }
        Route::ProxyJump(hops) => {
            let mut iter = hops.iter();
            let first = iter
                .next()
                .ok_or_else(|| ClientError::Route("ProxyJump route unexpectedly empty".to_string()))?;
            let mut upstream = first
                .authenticate(connect_via(Transport::Direct(first), timeout).await?, password, timeout)
                .await?;

            for hop in iter {
                let session =
                    connect_via(Transport::Jump(&mut upstream, hop), timeout).await?;
                upstream = hop.authenticate(session, password, timeout).await?;
            }

            let target_session =
                connect_via(Transport::Jump(&mut upstream, &plan.target), timeout).await?;
            plan.target.authenticate(target_session, password, timeout).await
        }
    }
}

pub fn resolve_identity(
    host_config_identity: Option<PathBuf>,
    cli_identity: Option<&PathBuf>,
    host_config_source: Source,
) -> (Option<PathBuf>, Source) {
    if let Some(identity) = host_config_identity {
        (Some(identity), host_config_source)
    } else if let Some(identity) = cli_identity {
        (Some(identity.clone()), Source::CliIdentity)
    } else {
        (None, Source::Fallback)
    }
}

pub fn resolve_user(
    host_config_user: Option<String>,
    inherited_user: &str,
    explicit_user: Option<&str>,
    host_config_source: Source,
    explicit_source: Source,
) -> (String, Source) {
    if let Some(user) = explicit_user {
        (user.to_string(), explicit_source)
    } else if let Some(user) = host_config_user {
        (user, host_config_source)
    } else {
        (inherited_user.to_string(), Source::InheritedDefault)
    }
}

fn resolve_jump_endpoint(
    spec: &JumpSpec,
    ssh_config: Option<&PathBuf>,
    default_user: &str,
    cli_identity: Option<&PathBuf>,
) -> Result<Endpoint> {
    let mut host = spec.host.clone();
    let mut host_config_user = None;
    let mut port = 22;
    let mut host_config_identity = None;

    if let Some(ssh_config) = ssh_config
        && ssh_config.exists()
    {
        let config = parse_path(ssh_config, spec.host.as_str()).map_err(|e| {
            ClientError::ConfigParse {
                context: "jump host configuration",
                source: e.to_string(),
            }
        })?;
        if !config.host().is_empty() {
            host = config.host().to_string();
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
        Source::ProxyHostConfig,
        Source::JumpSpec,
    );
    if let Some(spec_port) = spec.port {
        port = spec_port;
    }
    let (identity, identity_source) =
        resolve_identity(host_config_identity, cli_identity, Source::ProxyHostConfig);
    Ok(Endpoint {
        host,
        port,
        auth: AuthSpec {
            user,
            identity,
            user_source,
            identity_source,
        },
    })
}

fn parse_proxy_jump(proxy_jump: &str) -> Result<Vec<JumpSpec>> {
    proxy_jump
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty() && !item.eq_ignore_ascii_case("none"))
        .map(|item| {
            let (user, host_port) = if let Some((user, host_port)) = item.rsplit_once('@') {
                if user.is_empty() {
                    return Err(ClientError::InvalidProxyJump(format!(
                        "Invalid ProxyJump user in target: {item}"
                    )));
                }
                (Some(user.to_string()), host_port)
            } else {
                (None, item)
            };
            let (host, port) = if host_port.starts_with('[') {
                let close = host_port.find(']').ok_or_else(|| {
                    ClientError::InvalidProxyJump(format!(
                        "Invalid bracketed ProxyJump host: {item}"
                    ))
                })?;
                let rest = &host_port[close + 1..];
                let port = if rest.is_empty() {
                    None
                } else if let Some(port_str) = rest.strip_prefix(':') {
                    Some(port_str.parse::<u16>().map_err(|e| {
                        ClientError::InvalidProxyJump(format!("Invalid ProxyJump port in {item}: {e}"))
                    })?)
                } else {
                    return Err(ClientError::InvalidProxyJump(format!(
                        "Invalid ProxyJump host/port format: {item}"
                    )));
                };
                (host_port[1..close].to_string(), port)
            } else if host_port.matches(':').count() > 1 {
                (host_port.to_string(), None)
            } else if let Some((host, port)) = host_port.rsplit_once(':') {
                let port = port.parse::<u16>().map_err(|e| {
                    ClientError::InvalidProxyJump(format!("Invalid ProxyJump port in {item}: {e}"))
                })?;
                (host.to_string(), Some(port))
            } else {
                (host_port.to_string(), None)
            };
            if host.is_empty() {
                return Err(ClientError::InvalidProxyJump(format!(
                    "Invalid ProxyJump host: {item}"
                )));
            }
            Ok(JumpSpec { user, host, port })
        })
        .collect()
}

enum Transport<'a> {
    Direct(&'a Endpoint),
    ProxyCommand(&'a PathBuf, &'a Endpoint),
    Jump(&'a mut client::Handle<SshHandler>, &'a Endpoint),
}

async fn connect_via(transport: Transport<'_>, timeout: f64) -> Result<client::Handle<SshHandler>> {
    match transport {
        Transport::Direct(endpoint) => {
            let addr = (endpoint.host.as_str(), endpoint.port);
            connect_with_timeout(
                "connecting to",
                &endpoint.host,
                timeout,
                client::connect(client_config(timeout), addr, SshHandler),
            )
            .await
        }
        Transport::ProxyCommand(ssh_config, endpoint) => {
            let parsed = parse_path(ssh_config, endpoint.host.as_str()).map_err(|e| {
                ClientError::ConfigParse {
                    context: "SSH config for ProxyCommand",
                    source: e.to_string(),
                }
            })?;
            let stream = parsed
                .stream()
                .await
                .map_err(|e| ClientError::Connect {
                    host: endpoint.host.clone(),
                    source: format!("Failed to connect through ProxyCommand: {e}"),
                })?;
            connect_with_timeout(
                "stream-connecting through ProxyCommand to",
                &endpoint.host,
                timeout,
                client::connect_stream(client_config(timeout), stream, SshHandler),
            )
            .await
        }
        Transport::Jump(upstream, endpoint) => {
            let channel = upstream
                .channel_open_direct_tcpip(endpoint.host.clone(), endpoint.port as u32, "127.0.0.1", 0)
                .await
                .map_err(|e| ClientError::Connect {
                    host: endpoint.host.clone(),
                    source: format!("Failed to open direct-tcpip channel: {e}"),
                })?;
            connect_with_timeout(
                "stream-connecting to",
                &endpoint.host,
                timeout,
                client::connect_stream(client_config(timeout), channel.into_stream(), SshHandler),
            )
            .await
        }
    }
}

fn client_config(timeout: f64) -> Arc<client::Config> {
    Arc::new(client::Config {
        inactivity_timeout: Some(Duration::from_secs_f64(timeout)),
        ..Default::default()
    })
}

async fn connect_with_timeout<F>(
    context: &'static str,
    host: &str,
    timeout: f64,
    future: F,
) -> Result<client::Handle<SshHandler>>
where
    F: std::future::Future<Output = std::result::Result<client::Handle<SshHandler>, russh::Error>>,
{
    match tokio::time::timeout(Duration::from_secs_f64(timeout), future).await {
        Ok(Ok(session)) => Ok(session),
        Ok(Err(e)) => Err(ClientError::Connect {
            host: host.to_string(),
            source: e.to_string(),
        }),
        Err(_) => Err(ClientError::Timeout {
            context,
            host: host.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_proxy_jump, resolve_identity, resolve_user, Source};
    use std::path::PathBuf;

    #[test]
    fn identity_prefers_host_config_then_cli_then_fallback() {
        let host_cfg = PathBuf::from("/tmp/proxy_key");
        let cli = PathBuf::from("/tmp/cli_key");

        let (identity, source) =
            resolve_identity(Some(host_cfg.clone()), Some(&cli), Source::ProxyHostConfig);
        assert_eq!(identity, Some(host_cfg));
        assert_eq!(source, Source::ProxyHostConfig);

        let (identity, source) = resolve_identity(None, Some(&cli), Source::ProxyHostConfig);
        assert_eq!(identity, Some(cli));
        assert_eq!(source, Source::CliIdentity);

        let (identity, source) = resolve_identity(None, None, Source::ProxyHostConfig);
        assert_eq!(identity, None);
        assert_eq!(source, Source::Fallback);
    }

    #[test]
    fn user_prefers_explicit_then_host_config_then_inherited() {
        let (user, source) = resolve_user(
            Some("cfg-user".to_string()),
            "default-user",
            Some("jump-user"),
            Source::ProxyHostConfig,
            Source::JumpSpec,
        );
        assert_eq!(user, "jump-user");
        assert_eq!(source, Source::JumpSpec);

        let (user, source) = resolve_user(
            Some("cfg-user".to_string()),
            "default-user",
            None,
            Source::ProxyHostConfig,
            Source::JumpSpec,
        );
        assert_eq!(user, "cfg-user");
        assert_eq!(source, Source::ProxyHostConfig);

        let (user, source) = resolve_user(
            None,
            "default-user",
            None,
            Source::ProxyHostConfig,
            Source::JumpSpec,
        );
        assert_eq!(user, "default-user");
        assert_eq!(source, Source::InheritedDefault);
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
