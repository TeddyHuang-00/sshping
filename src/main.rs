mod auth;
mod cli;
mod summary;
mod tests;
mod util;

use std::{
    io::Read,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::Arc,
    time::Instant,
};

use auth::authenticate_all;
use clap::Parser;
use cli::{Options, Test};
use log::{debug, error, trace, LevelFilter};
use regex::Regex;
use russh::{ChannelMsg, client};
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
}

#[derive(Clone, Debug)]
struct JumpSpec {
    user: Option<String>,
    host: String,
    port: Option<u16>,
}

fn parse_proxy_jump(proxy_jump: &str) -> Result<Vec<JumpSpec>, String> {
    let target_pat = Regex::new(r"^(?:([a-zA-Z0-9_.-]+)@)?([a-zA-Z0-9_.-]+)(?::(\d+))?$")
        .map_err(|e| format!("Failed to build ProxyJump parser: {e}"))?;
    proxy_jump
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty() && !item.eq_ignore_ascii_case("none"))
        .map(|item| {
            let captures = target_pat
                .captures(item)
                .ok_or_else(|| format!("Invalid ProxyJump target format: {item}"))?;
            let user = captures.get(1).map(|m| m.as_str().to_string());
            let host = captures
                .get(2)
                .map(|m| m.as_str().to_string())
                .ok_or_else(|| format!("Invalid ProxyJump host: {item}"))?;
            let port = captures
                .get(3)
                .map(|m| {
                    m.as_str()
                        .parse::<u16>()
                        .map_err(|e| format!("Invalid ProxyJump port in {item}: {e}"))
                })
                .transpose()?;
            Ok(JumpSpec { user, host, port })
        })
        .collect()
}

fn wildcard_pattern_match(host: &str, pattern: &str) -> bool {
    let mut regex_pattern = String::from("^");
    for c in pattern.chars() {
        match c {
            '*' => regex_pattern.push_str(".*"),
            '?' => regex_pattern.push('.'),
            _ => regex_pattern.push_str(&regex::escape(c.to_string().as_str())),
        }
    }
    regex_pattern.push('$');
    Regex::new(&regex_pattern)
        .map(|r| r.is_match(host))
        .unwrap_or(false)
}

fn host_patterns_match(host: &str, patterns: &str) -> bool {
    let mut matched = false;
    for pattern in patterns.split_ascii_whitespace() {
        if pattern.is_empty() {
            continue;
        }
        if let Some(negated) = pattern.strip_prefix('!') {
            if wildcard_pattern_match(host, negated) {
                return false;
            }
        } else if wildcard_pattern_match(host, pattern) {
            matched = true;
        }
    }
    matched
}

fn parse_remote_command(ssh_config: &Path, host: &str) -> Option<String> {
    let content = std::fs::read_to_string(ssh_config).ok()?;
    let mut current_host_matches = false;
    let mut remote_command: Option<String> = None;
    for line in content.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let key = parts.next().unwrap_or("");
        let value = parts.next().unwrap_or("").trim_start();
        if key.eq_ignore_ascii_case("host") {
            current_host_matches = host_patterns_match(host, value);
            continue;
        }
        if key.eq_ignore_ascii_case("remotecommand")
            && current_host_matches
            && remote_command.is_none()
            && !value.is_empty()
        {
            remote_command = Some(value.to_string());
        }
    }
    remote_command
}

fn endpoint_from_jump_spec(
    spec: &JumpSpec,
    ssh_config: Option<&PathBuf>,
    default_user: &str,
) -> Result<Endpoint, String> {
    let mut host = spec.host.clone();
    let mut user = default_user.to_string();
    let mut port = 22;
    let mut identity = None;

    if let Some(ssh_config) = ssh_config
        && ssh_config.exists()
    {
        let config = parse_path(ssh_config, spec.host.as_str())
            .map_err(|e| format!("Failed to parse jump host configuration: {e}"))?;
        let config_host = config.host();
        if !config_host.is_empty() {
            host = config_host.to_string();
        }
        user = config.user();
        port = config.port();
        identity = config
            .host_config
            .identity_file
            .and_then(|files| files.first().cloned());
    }

    if let Some(spec_user) = &spec.user {
        user = spec_user.clone();
    }
    if let Some(spec_port) = spec.port {
        port = spec_port;
    }

    Ok(Endpoint {
        user,
        host,
        port,
        identity,
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
    authenticate_all(
        &mut session,
        &endpoint.user,
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
    authenticate_all(
        &mut session,
        &endpoint.user,
        password,
        endpoint.identity.as_ref(),
        timeout,
    )
    .await
    .map_err(ToString::to_string)?;
    Ok(session)
}

async fn execute_remote_command(
    session: &mut client::Handle<SshHandler>,
    command: &str,
    timeout: f64,
) -> Result<(), String> {
    debug!("Executing RemoteCommand: {command}");
    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|e| format!("Failed to open session channel for RemoteCommand: {e}"))?;
    channel
        .exec(false, command.as_bytes())
        .await
        .map_err(|e| format!("Failed to execute RemoteCommand: {e}"))?;

    let timeout_duration = std::time::Duration::from_secs_f64(timeout);
    let mut exit_status = None;
    loop {
        match tokio::time::timeout(timeout_duration, channel.wait()).await {
            Ok(Some(ChannelMsg::ExitStatus { exit_status: status })) => {
                exit_status = Some(status);
            }
            Ok(Some(ChannelMsg::Close)) | Ok(None) => break,
            Ok(Some(ChannelMsg::Eof)) => {}
            Ok(Some(_)) => {}
            Err(_) => {
                return Err(format!(
                    "Timed out waiting for RemoteCommand completion after {timeout} seconds"
                ));
            }
        }
    }

    if let Some(status) = exit_status
        && status != 0
    {
        return Err(format!("RemoteCommand exited with status {status}"));
    }
    Ok(())
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
    let mut remote_command = None;
    let original_target_host = opts.target.host.clone();

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
        }
        if let Some(port) = config.host_config.port {
            opts.target.port = port;
        }
        if let Some(identity) = config
            .host_config
            .identity_file
            .and_then(|files| files.first().cloned())
        {
            opts.identity = Some(identity);
        }
        proxy_jump = config.host_config.proxy_jump;
        remote_command = parse_remote_command(ssh_config, original_target_host.as_str());
    }

    trace!("Options: {:?}", opts);
    debug!("User: {}", opts.target.user);
    debug!("Host: {}", opts.target.host);
    debug!("Port: {}", opts.target.port);

    let target_endpoint = Endpoint {
        user: opts.target.user.clone(),
        host: opts.target.host.clone(),
        port: opts.target.port,
        identity: opts.identity.clone(),
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
            let default_user = whoami::username();
            let first_jump = match endpoint_from_jump_spec(
                jumps.first().expect("unreachable"),
                opts.config.as_ref(),
                default_user.as_str(),
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
                let endpoint =
                    match endpoint_from_jump_spec(jump_spec, opts.config.as_ref(), default_user.as_str()) {
                        Ok(endpoint) => endpoint,
                        Err(e) => {
                            error!("{e}");
                            return ExitCode::FAILURE;
                        }
                    };
                let previous_jump = jump_sessions.last_mut().expect("at least one jump session");
                let jump_session = match connect_and_authenticate_through_jump(
                    previous_jump,
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

            let last_jump = jump_sessions.last_mut().expect("at least one jump session");
            match connect_and_authenticate_through_jump(
                last_jump,
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
        match connect_and_authenticate(&target_endpoint, opts.ssh_timeout, opts.password.as_deref()).await {
            Ok(session) => session,
            Err(e) => {
                error!("Failed to connect to target: {e}");
                return ExitCode::FAILURE;
            }
        }
    };
    let ssh_connect_time = connect_start.elapsed();

    if let Some(command) = remote_command
        && let Err(e) = execute_remote_command(&mut session, command.as_str(), opts.ssh_timeout).await
    {
        error!("Failed to execute RemoteCommand: {e}");
        return ExitCode::FAILURE;
    }

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
