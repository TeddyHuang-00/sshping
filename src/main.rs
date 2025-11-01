mod auth;
mod cli;
mod summary;
mod tests;
mod util;

use std::{
    fs::File,
    io::{BufReader, Read},
    process::ExitCode,
    sync::Arc,
};

use auth::authenticate_all;
use clap::Parser;
use cli::{Options, Test};
use log::{debug, error, trace, LevelFilter};
use russh::client;
use simple_logger::SimpleLogger;
use ssh2_config::{ParseRule, SshConfig};
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

async fn execute_remote_command<H: client::Handler>(
    session: &mut client::Handle<H>,
    command: &str,
) -> Result<String, String> {
    use russh::ChannelMsg;

    trace!("Opening channel for remote command execution");
    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|e| e.to_string())?;

    trace!("Executing command: {command}");
    channel
        .exec(true, command)
        .await
        .map_err(|e| e.to_string())?;

    let mut output = String::new();
    let mut stderr_output = String::new();

    // Read all output from the command
    while let Some(msg) = channel.wait().await {
        match msg {
            ChannelMsg::Data { ref data } => {
                output.push_str(&String::from_utf8_lossy(data));
            }
            ChannelMsg::ExtendedData { ref data, ext } => {
                if ext == 1 {
                    // stderr
                    stderr_output.push_str(&String::from_utf8_lossy(data));
                }
            }
            ChannelMsg::ExitStatus { exit_status } => {
                trace!("Command exited with status: {exit_status}");
                if exit_status != 0 {
                    if !stderr_output.is_empty() {
                        return Err(format!(
                            "Command failed with exit code {}: {}",
                            exit_status, stderr_output
                        ));
                    }
                    return Err(format!("Command failed with exit code {}", exit_status));
                }
            }
            ChannelMsg::Eof => {
                break;
            }
            _ => {}
        }
    }

    if !stderr_output.is_empty() {
        debug!("Command stderr: {stderr_output}");
    }

    Ok(output)
}

async fn connect_with_proxy_jump(
    config: Arc<client::Config>,
    proxy_jump: &str,
    target: &cli::Target,
    timeout: f64,
    identity: Option<&std::path::PathBuf>,
    password: Option<&str>,
) -> Result<client::Handle<SshHandler>, String> {
    use regex::Regex;
    use whoami::username;

    // Parse jump hosts (format: [user@]host[:port][,[user@]host[:port],...])
    let jump_hosts: Vec<_> = proxy_jump.split(',').collect();

    if jump_hosts.is_empty() {
        return Err("No jump hosts specified".to_string());
    }

    // Parse the first jump host
    let pat = Regex::new(r"^(?:([a-zA-Z0-9_.-]+)@)?([a-zA-Z0-9_.-]+)(?::(\d+))?$").unwrap();
    let cap = pat
        .captures(jump_hosts[0])
        .ok_or_else(|| format!("Invalid jump host format: {}", jump_hosts[0]))?;

    let jump_user = cap.get(1).map_or(username(), |m| m.as_str().to_string());
    let jump_host = cap.get(2).unwrap().as_str().to_string();
    let jump_port = cap.get(3).map_or(22, |m| m.as_str().parse().unwrap());

    debug!(
        "Connecting to jump host: {}@{}:{}",
        jump_user, jump_host, jump_port
    );

    // Connect to the jump host
    let handler = SshHandler;
    let addr = (jump_host.as_str(), jump_port);
    let mut jump_session = match tokio::time::timeout(
        std::time::Duration::from_secs_f64(timeout),
        client::connect(config.clone(), addr, handler),
    )
    .await
    {
        Ok(Ok(session)) => session,
        Ok(Err(e)) => {
            return Err(format!("Failed to connect to jump host: {e}"));
        }
        Err(_) => {
            return Err("Jump host connection timeout".to_string());
        }
    };

    // Authenticate with jump host
    debug!("Authenticating with jump host");
    authenticate_all(&mut jump_session, &jump_user, password, identity, timeout)
        .await
        .map_err(|e| format!("Failed to authenticate with jump host: {e}"))?;

    // If there are more jump hosts, we would need to chain them
    // For now, we support only one jump host
    if jump_hosts.len() > 1 {
        return Err("Multiple jump hosts are not yet supported".to_string());
    }

    // Open a direct-tcpip channel to the target through the jump host
    debug!(
        "Opening tunnel to target {}@{}:{} through jump host",
        target.user, target.host, target.port
    );

    let channel = jump_session
        .channel_open_direct_tcpip(
            &target.host,
            target.port as u32,
            "127.0.0.1", // originator address (local)
            22,          // originator port (local)
        )
        .await
        .map_err(|e| format!("Failed to open tunnel through jump host: {e}"))?;

    debug!("Tunnel established, connecting to target through tunnel");

    // Now we need to establish an SSH connection through this channel
    // This is the tricky part - russh doesn't directly support using a channel as transport
    // We'll need to use the channel's stream as the transport
    let stream = channel.into_stream();

    // Create a new SSH session using the tunneled stream
    let handler = SshHandler;
    let target_session = client::connect_stream(config, stream, handler)
        .await
        .map_err(|e| format!("Failed to connect to target through tunnel: {e}"))?;

    Ok(target_session)
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

    // Respect the SSH configuration file if it exists
    if opts.config.exists() {
        debug!("SSH Config: {:?}", opts.config);
        let mut reader =
            BufReader::new(File::open(&opts.config).expect("Could not open configuration file"));
        let config = SshConfig::default()
            .parse(&mut reader, ParseRule::ALLOW_UNKNOWN_FIELDS)
            .expect("Failed to parse configuration");
        // Query attributes for host
        let params = config.query(opts.target.host.as_str());
        // Update options with configuration
        if let Some(host) = params.host_name {
            opts.target.host = host;
        }
        if let Some(user) = params.user {
            opts.target.user = user;
        }
        if let Some(port) = params.port {
            opts.target.port = port;
        }
        if let Some(identity) = params.identity_file {
            opts.identity = Some(identity[0].to_owned());
        }
        // Read proxy_jump from SSH config if not specified on command line
        if opts.proxy_jump.is_none()
            && let Some(proxy_jump) = params.proxy_jump
        {
            opts.proxy_jump = Some(proxy_jump.join(","));
        }
    }

    trace!("Options: {:?}", opts);
    debug!("User: {}", opts.target.user);
    debug!("Host: {}", opts.target.host);
    debug!("Port: {}", opts.target.port);

    // Connect to the SSH server (possibly through proxy jump hosts)
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(std::time::Duration::from_secs_f64(opts.ssh_timeout)),
        ..Default::default()
    });

    let mut session = if let Some(ref proxy_jump) = opts.proxy_jump {
        debug!("Using ProxyJump: {proxy_jump}");
        match connect_with_proxy_jump(
            config,
            proxy_jump,
            &opts.target,
            opts.ssh_timeout,
            opts.identity.as_ref(),
            opts.password.as_deref(),
        )
        .await
        {
            Ok(session) => session,
            Err(e) => {
                error!("Failed to connect via proxy jump: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        let handler = SshHandler;
        let addr = (opts.target.host.as_str(), opts.target.port);
        match tokio::time::timeout(
            std::time::Duration::from_secs_f64(opts.ssh_timeout),
            client::connect(config, addr, handler),
        )
        .await
        {
            Ok(Ok(session)) => session,
            Ok(Err(e)) => {
                error!("Failed to connect to server: {e}");
                return ExitCode::FAILURE;
            }
            Err(_) => {
                error!("Connection timeout");
                return ExitCode::FAILURE;
            }
        }
    };

    // Try to authenticate with the server using:
    // 1) identity in the agent;
    // 2) specified identity;
    // 3) password
    let ssh_connect_time = match authenticate_all(
        &mut session,
        &opts.target.user,
        opts.password.as_deref(),
        opts.identity.as_ref(),
        opts.ssh_timeout,
    )
    .await
    {
        Ok(time) => time,
        Err(e) => {
            error!("Exiting due to authenticate: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Execute remote command if specified
    if let Some(ref remote_cmd) = opts.remote_command {
        debug!("Executing remote command: {remote_cmd}");
        match execute_remote_command(&mut session, remote_cmd).await {
            Ok(output) => {
                println!("{}", output);
                return ExitCode::SUCCESS;
            }
            Err(e) => {
                error!("Failed to execute remote command: {e}");
                return ExitCode::FAILURE;
            }
        }
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
