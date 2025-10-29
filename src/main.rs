mod auth;
mod cli;
mod summary;
mod tests;
mod util;

use std::{
    io::Read,
    process::ExitCode,
    sync::Arc,
};

use auth::authenticate_all;
use clap::Parser;
use cli::{Options, Test};
use log::{debug, error, trace, LevelFilter};
use russh::client;
use russh_config;
use simple_logger::SimpleLogger;
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

    // Parse SSH configuration using russh-config
    let ssh_config = if opts.config.exists() {
        debug!("SSH Config: {:?}", opts.config);
        match russh_config::parse_path(&opts.config, &opts.target.host) {
            Ok(config) => Some(config),
            Err(e) => {
                debug!("Failed to parse SSH config: {e}, using defaults");
                None
            }
        }
    } else {
        debug!("SSH config file does not exist, using defaults");
        None
    };

    trace!("Options: {:?}", opts);
    debug!("User: {}", opts.target.user);
    debug!("Host: {}", opts.target.host);
    debug!("Port: {}", opts.target.port);

    // Connect to the SSH server using russh-config's stream when available
    let client_config = Arc::new(client::Config {
        inactivity_timeout: Some(std::time::Duration::from_secs_f64(opts.ssh_timeout)),
        ..Default::default()
    });
    let handler = SshHandler;

    let mut session = if let Some(config) = ssh_config {
        // Use russh-config to get the stream (handles ProxyCommand if configured)
        match tokio::time::timeout(
            std::time::Duration::from_secs_f64(opts.ssh_timeout),
            async {
                let stream = config.stream().await.map_err(|e| {
                    russh::Error::from(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
                })?;
                client::connect_stream(client_config, stream, handler).await
            },
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
    } else {
        // Fallback to direct connection when config is not available
        let addr = (opts.target.host.as_str(), opts.target.port);
        match tokio::time::timeout(
            std::time::Duration::from_secs_f64(opts.ssh_timeout),
            client::connect(client_config, addr, handler),
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
