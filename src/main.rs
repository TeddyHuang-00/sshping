mod cli;
mod summary;
mod tests;
mod util;

use std::{io::Read, process::ExitCode, sync::Arc, time::Instant};

use clap::Parser;
use cli::{Options, Test};
use log::{debug, error, trace, LevelFilter};
use openssh::{KnownHosts, Session};
use simple_logger::SimpleLogger;
use tabled::{
    settings::{themes::BorderCorrection, Alignment, Span},
    Table,
};
use tests::{run_echo_test, run_speed_test};
use util::Formatter;

use crate::summary::Record;

#[tokio::main]
async fn main() -> ExitCode {
    let opts = Options::parse();

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

    trace!("Options: {:?}", opts);
    debug!("User: {}", opts.target.user);
    debug!("Host: {}", opts.target.host);
    debug!("Port: {}", opts.target.port);

    let connect_start = Instant::now();
    let session = match Session::connect(opts.target.to_string(), KnownHosts::Accept).await {
        Ok(session) => Arc::new(session),
        Err(e) => {
            error!("Failed to create session to {}: {e}", opts.target);
            return ExitCode::FAILURE;
        }
    };
    let ssh_connect_time = connect_start.elapsed();

    // Running tests
    let echo_test_result = if opts.run_tests == Test::Echo || opts.run_tests == Test::Both {
        match run_echo_test(
            &session,
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
            session.clone(),
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
                "ssh_connect_time": formatter.format_duration(ssh_connect_time)
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
