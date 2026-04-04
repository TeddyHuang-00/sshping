mod auth;
mod cli;
mod summary;
mod tests;
mod util;

use std::{
    io::Read,
    path::PathBuf,
    process::ExitCode,
    sync::Arc,
    time::Instant,
};

use auth::authenticate_all;
use clap::Parser;
use cli::{Options, Test};
use log::{debug, error, trace, LevelFilter};
use russh::client;
use simple_logger::SimpleLogger;
use russh_config::parse_path;
use summary::Record;
use tabled::{
    settings::{themes::BorderCorrection, Alignment, Span},
    Table,
};
use tests::{run_echo_test, run_speed_test};
use util::Formatter;

struct SshHandler;

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
struct Endpoint {
    user: String,
    host: String,
    port: u16,
    identity: Option<PathBuf>,
    identity_source: &'static str,
    user_source: &'static str,
}

#[derive(Clone, Debug)]
struct JumpSpec {
    user: Option<String>,
    host: String,
    port: Option<u16>,
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
            let (host, port) = if let Some((host, port)) = host_port.rsplit_once(':') {
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

fn resolve_identity(
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

fn resolve_user(
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

async fn connect_direct(
    endpoint: &Endpoint,
    timeout: f64,
) -> Result<client::Handle<SshHandler>, String> {
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

async fn connect_and_authenticate(
    endpoint: &Endpoint,
    timeout: f64,
    password: Option<&str>,
) -> Result<client::Handle<SshHandler>, String> {
    let mut session = connect_direct(endpoint, timeout).await?;
    debug!(
        "Credentials for {}@{}:{} => user source: {}, identity source: {}",
        endpoint.user, endpoint.host, endpoint.port, endpoint.user_source, endpoint.identity_source
    );
    authenticate_all(
        &mut session,
        &endpoint.user,
        &endpoint.host,
        password,
        endpoint.identity.as_ref(),
        timeout,
    )
    .await
    .map_err(ToString::to_string)?;
    Ok(session)
}

async fn connect_and_authenticate_with_proxy_command(
    ssh_config: &PathBuf,
    endpoint: &Endpoint,
    timeout: f64,
    password: Option<&str>,
) -> Result<client::Handle<SshHandler>, String> {
    let mut session = connect_with_proxy_command(ssh_config, endpoint, timeout).await?;
    debug!(
        "Credentials for {}@{}:{} => user source: {}, identity source: {}",
        endpoint.user, endpoint.host, endpoint.port, endpoint.user_source, endpoint.identity_source
    );
    authenticate_all(
        &mut session,
        &endpoint.user,
        &endpoint.host,
        password,
        endpoint.identity.as_ref(),
        timeout,
    )
    .await
    .map_err(ToString::to_string)?;
    Ok(session)
}

async fn connect_and_authenticate_through_jump(
    jump: &mut client::Handle<SshHandler>,
    endpoint: &Endpoint,
    timeout: f64,
    password: Option<&str>,
) -> Result<client::Handle<SshHandler>, String> {
    let mut session = connect_through_jump(jump, endpoint, timeout).await?;
    debug!(
        "Credentials for {}@{}:{} => user source: {}, identity source: {}",
        endpoint.user, endpoint.host, endpoint.port, endpoint.user_source, endpoint.identity_source
    );
    authenticate_all(
        &mut session,
        &endpoint.user,
        &endpoint.host,
        password,
        endpoint.identity.as_ref(),
        timeout,
    )
    .await
    .map_err(ToString::to_string)?;
    Ok(session)
}

#[tokio::main]
async fn main() -> ExitCode {
    let mut opts = Options::parse();

    // Initialize logging
    SimpleLogger::new()
        .with_level(LevelFilter::Off)
        .with_module_level(
            "sshping",
            match opts.verbose {
                0 => LevelFilter::Error,
                1 => LevelFilter::Warn,
                2 => LevelFilter::Info,
                3 => LevelFilter::Debug,
                4.. => LevelFilter::Trace,
            },
        )
        .without_timestamps()
        .init()
        .unwrap();

    // Get the formatter for output
    let formatter = Formatter::new(opts.human_readable, opts.delimiter);

    let mut proxy_jump = None;
    let mut proxy_command = None;
    let mut target_identity_from_config = None;
    let mut target_user_from_config = false;
    let cli_identity = opts.identity.clone();

    // Respect the SSH configuration file if it exists
    if let Some(ssh_config) = &opts.config
        && ssh_config.exists()
    {
        debug!("SSH Config: {:?}", ssh_config);
        let config = parse_path(ssh_config, opts.target.host.as_str())
            .expect("Failed to parse configuration");

        // Update options with configuration
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

    trace!("Options: {:?}", opts);
    debug!("User: {}", opts.target.user);
    debug!("Host: {}", opts.target.host);
    debug!("Port: {}", opts.target.port);

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

    let connect_start = Instant::now();
    let mut jump_sessions: Vec<client::Handle<SshHandler>> = Vec::new();
    let mut session = if let Some(proxy_jump) = proxy_jump {
        let jumps = match parse_proxy_jump(proxy_jump.as_str()) {
            Ok(v) => v,
            Err(e) => {
                error!("{e}");
                return ExitCode::FAILURE;
            }
        };
        if jumps.is_empty() {
            match connect_and_authenticate(
                &target_endpoint,
                opts.ssh_timeout,
                opts.password.as_deref(),
            )
            .await
            {
                Ok(session) => session,
                Err(e) => {
                    error!("Failed to connect to target: {e}");
                    return ExitCode::FAILURE;
                }
            }
        } else {
            let default_user = opts.target.user.clone();
            let first_jump = match endpoint_from_jump_spec(
                jumps.first().expect("non-empty jumps"),
                opts.config.as_ref(),
                default_user.as_str(),
                cli_identity.as_ref(),
            ) {
                Ok(endpoint) => endpoint,
                Err(e) => {
                    error!("{e}");
                    return ExitCode::FAILURE;
                }
            };
            let first_session = match connect_and_authenticate(
                &first_jump,
                opts.ssh_timeout,
                opts.password.as_deref(),
            )
            .await
            {
                Ok(session) => session,
                Err(e) => {
                    error!("Failed to connect to jump host {}: {e}", first_jump.host);
                    return ExitCode::FAILURE;
                }
            };
            jump_sessions.push(first_session);

            for jump_spec in jumps.iter().skip(1) {
                let endpoint = match endpoint_from_jump_spec(
                    jump_spec,
                    opts.config.as_ref(),
                    default_user.as_str(),
                    cli_identity.as_ref(),
                ) {
                    Ok(endpoint) => endpoint,
                    Err(e) => {
                        error!("{e}");
                        return ExitCode::FAILURE;
                    }
                };
                let jump_session = match connect_and_authenticate_through_jump(
                    jump_sessions.last_mut().expect("at least one jump session"),
                    &endpoint,
                    opts.ssh_timeout,
                    opts.password.as_deref(),
                )
                .await
                {
                    Ok(session) => session,
                    Err(e) => {
                        error!("Failed to connect through jump host {}: {e}", endpoint.host);
                        return ExitCode::FAILURE;
                    }
                };
                jump_sessions.push(jump_session);
            }

            match connect_and_authenticate_through_jump(
                jump_sessions.last_mut().expect("at least one jump session"),
                &target_endpoint,
                opts.ssh_timeout,
                opts.password.as_deref(),
            )
            .await
            {
                Ok(session) => session,
                Err(e) => {
                    error!("Failed to connect to target through jump host(s): {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
    } else {
        if proxy_command.is_some() && opts.config.as_ref().is_some_and(|c| c.exists()) {
            let config_path = opts.config.as_ref().expect("config exists by condition");
            match connect_and_authenticate_with_proxy_command(
                config_path,
                &target_endpoint,
                opts.ssh_timeout,
                opts.password.as_deref(),
            )
            .await
            {
                Ok(session) => session,
                Err(e) => {
                    error!("Failed to connect to target through ProxyCommand: {e}");
                    return ExitCode::FAILURE;
                }
            }
        } else {
            match connect_and_authenticate(&target_endpoint, opts.ssh_timeout, opts.password.as_deref()).await {
                Ok(session) => session,
                Err(e) => {
                    error!("Failed to connect to target: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
    };
    let ssh_connect_time = connect_start.elapsed();

    // Running tests
    let echo_test_result = if opts.run_tests == Test::Echo || opts.run_tests == Test::Both {
        match run_echo_test(
            &mut session,
            &opts.echo_cmd,
            opts.char_count,
            opts.echo_timeout,
            &formatter,
        )
        .await
        {
            Ok(result) => Some(result),
            Err(e) => {
                error!("Failed to finish echo test: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        None
    };
    let speed_test_result = if opts.run_tests == Test::Speed || opts.run_tests == Test::Both {
        match run_speed_test(
            &mut session,
            opts.size,
            opts.chunk_size,
            &opts.remote_file,
            &formatter,
        )
        .await
        {
            Ok(result) => Some(result),
            Err(e) => {
                error!("Failed to finish speed test: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        None
    };

    // Output results
    match opts.format {
        cli::Format::Table => {
            let mut data = vec![Record::new(
                "SSH",
                "Connect time",
                formatter.format_duration(ssh_connect_time),
            )];
            let mut modifications = vec![];
            let mut row_count = 1;
            if let Some(result) = echo_test_result {
                let records = result.to_formatted_frame();
                modifications.push((
                    (row_count + 1, 0),
                    Span::row(records.len().try_into().unwrap()),
                ));
                row_count += records.len();
                data.extend(records);
            }
            if let Some(result) = speed_test_result {
                let records = result.to_formatted_frame();
                modifications.push((
                    (row_count + 1, 0),
                    Span::row(records.len().try_into().unwrap()),
                ));
                data.extend(records);
            }
            let mut table = Table::new(data);
            modifications.into_iter().for_each(|(span, span_mod)| {
                table.modify(span, span_mod);
            });
            opts.table_style
                .stylize(&mut table)
                .with(Alignment::center())
                .with(Alignment::center_vertical())
                .with(BorderCorrection::span());
            // Clear the line before printing the table
            print!("{:<80}\r", "");
            println!("{}", table);
        }
        cli::Format::Json => {
            let mut json = serde_json::json!({
                "ssh_connect_time": formatter.format_duration(ssh_connect_time),
            });
            if let Some(result) = echo_test_result {
                json["echo_test"] = serde_json::json!(result);
            }
            if let Some(result) = speed_test_result {
                json["speed_test"] = serde_json::json!(result);
            }
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
        }
    }

    // Waiting for key input before exiting
    if opts.key_wait {
        println!("Press enter to exit...");
        let mut buf = [0u8; 1];
        let _ = std::io::stdin().read(&mut buf).unwrap();
    }

    // Exit successfully
    ExitCode::SUCCESS
}

#[cfg(test)]
mod auth_resolution_tests {
    use super::{resolve_identity, resolve_user};
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
}
