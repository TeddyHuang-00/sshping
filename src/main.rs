mod auth;
mod cli;
mod style;
mod summary;
mod tests;
mod util;

use std::{
    fs::File,
    io::{BufReader, Read},
    net::TcpStream,
    process::ExitCode,
};

use auth::authenticate_all;
use clap::Parser;
use cli::{Options, Test};
use log::{debug, error, trace, LevelFilter};
use simple_logger::SimpleLogger;
use ssh2::Session;
use ssh2_config::{ParseRule, SshConfig};
use summary::Record;
use tabled::{
    settings::{style::BorderSpanCorrection, Alignment, Span},
    Table,
};
use tests::{run_echo_test, run_speed_test};
use util::Formatter;

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
    }

    trace!("Options: {:?}", opts);
    debug!("User: {}", opts.target.user);
    debug!("Host: {}", opts.target.host);
    debug!("Port: {}", opts.target.port);

    // Connect to the local SSH server
    let tcp = match TcpStream::connect(format!("{}:{}", opts.target.host, opts.target.port)) {
        Ok(tcp) => tcp,
        Err(e) => {
            error!("Failed to connect to server: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut session = match Session::new() {
        Ok(session) => session,
        Err(e) => {
            error!("Failed to create session: {e}");
            return ExitCode::FAILURE;
        }
    };
    session.set_timeout((opts.ssh_timeout * 1000.0) as u32);
    session.set_tcp_stream(tcp);
    match session.handshake() {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to handshake: {e}");
            return ExitCode::FAILURE;
        }
    }

    // Try to authenticate with the server using:
    // 1) identity in the agent;
    // 2) specified identity;
    // 3) password
    let ssh_connect_time = match authenticate_all(
        &session,
        &opts.target.user,
        opts.password.as_deref(),
        opts.identity.as_ref(),
    ) {
        Ok(time) => time,
        Err(e) => {
            error!("Exiting due to authenticate: {e}");
            return ExitCode::FAILURE;
        }
    };
    // Make sure we succeeded
    assert!(session.authenticated());

    // Running tests
    let echo_test_result = if opts.run_tests == Test::Echo || opts.run_tests == Test::Both {
        match run_echo_test(
            &session,
            &opts.echo_cmd,
            opts.char_count,
            opts.echo_timeout,
            &formatter,
        ) {
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
            &session,
            opts.size,
            opts.chunk_size,
            &opts.remote_file,
            &formatter,
        ) {
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
    let mut data = vec![Record::new(
        "SSH",
        "Connect time",
        formatter.format_duration(ssh_connect_time),
    )];
    let mut modifications = vec![];
    let mut row_count = 1;
    if let Some(result) = echo_test_result {
        let records = result.to_formatted_frame();
        modifications.push(((row_count + 1, 0), Span::row(records.len())));
        row_count += records.len();
        data.extend(records);
    }
    if let Some(result) = speed_test_result {
        let records = result.to_formatted_frame();
        modifications.push(((row_count + 1, 0), Span::row(records.len())));
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
        .with(BorderSpanCorrection);
    // Clear the line before printing the table
    print!("{:<80}\r", "");
    println!("{}", table);

    // Waiting for key input before exiting
    if opts.key_wait {
        println!("Press enter to exit...");
        let mut buf = [0u8; 1];
        let _ = std::io::stdin().read(&mut buf).unwrap();
    }

    // Exit successfully
    ExitCode::SUCCESS
}
