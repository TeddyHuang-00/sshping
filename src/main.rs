mod auth;
mod cli;
mod tests;

use auth::authenticate_all;
use cli::{Options, Test};
use tests::{run_echo_test, run_speed_test};

use clap::Parser;
use log::{error, info, LevelFilter};
use simple_logger::SimpleLogger;
use ssh2::Session;
use ssh2_config::{ParseRule, SshConfig};
use std::fs::File;
use std::io::{BufReader, Read};
use std::net::TcpStream;
use std::process::ExitCode;

// use std::path::Path;
// use std::time::Duration;

fn main() -> ExitCode {
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
        .init()
        .unwrap();

    // Respect the SSH configuration file if it exists
    if opts.config.exists() {
        info!("SSH Config: {:?}", opts.config);
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
        if let Some(bind_addr) = params.bind_address {
            opts.bind_addr = Some(bind_addr);
        }
        if let Some(identity) = params.identity_file {
            opts.identity = Some(identity[0].to_owned());
        }
    }

    info!("User: {}", opts.target.user);
    info!("Host: {}", opts.target.host);
    info!("Port: {}", opts.target.port);
    info!("Echo Command: {}", opts.echo_cmd);

    // Connect to the local SSH server
    let tcp = match TcpStream::connect(format!("{}:{}", opts.target.host, opts.target.port)) {
        Ok(tcp) => tcp,
        Err(e) => {
            error!("Failed to connect to server: {e}");
            return ExitCode::from(1);
        }
    };
    let mut session = match Session::new() {
        Ok(session) => session,
        Err(e) => {
            error!("Failed to create session: {e}");
            return ExitCode::from(1);
        }
    };
    session.set_timeout((opts.ssh_timeout * 1000.0) as u32);
    session.set_tcp_stream(tcp);
    match session.handshake() {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to handshake: {e}");
            return ExitCode::from(1);
        }
    }

    // Try to authenticate with the 1) first identity in the agent; 2) specified identity; 3) password
    match authenticate_all(
        &session,
        &opts.target.user,
        opts.password.as_deref(),
        opts.identity.as_ref(),
    ) {
        Ok(_) => {}
        Err(e) => {
            error!("{e}");
            error!("Existing due to authentication failure");
            return ExitCode::from(1);
        }
    }
    // Make sure we succeeded
    assert!(session.authenticated());

    // Running tests
    if opts.run_tests == Test::Echo || opts.run_tests == Test::Both {
        run_echo_test(&session, &opts.echo_cmd, opts.char_count, opts.echo_timeout);
    }
    if opts.run_tests == Test::Speed || opts.run_tests == Test::Both {
        run_speed_test(&session, opts.size, &opts.remote_file);
    }

    // Waiting for key input before exiting
    if opts.key_wait {
        println!("Press any key to exit...");
        let mut buf = [0u8; 1];
        let _ = std::io::stdin().read(&mut buf).unwrap();
    }
    // Exit successfully
    ExitCode::from(0)
}
