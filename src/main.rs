mod auth;
mod cli;
mod client;
mod completer;
mod summary;
mod tests;
mod util;

use std::{io::Read, process::ExitCode, time::Instant};

use clap::{CommandFactory, Parser};
use clap_complete::CompleteEnv;
use cli::{Options, Test};
use client::{build_connection_plan, connect_plan};
use log::{debug, error, trace, LevelFilter};
use simple_logger::SimpleLogger;
use summary::Record;
use tabled::{
    settings::{themes::BorderCorrection, Alignment, Span},
    Table,
};
use tests::{run_echo_test, run_speed_test};
use util::Formatter;

#[tokio::main]
async fn main() -> ExitCode {
    CompleteEnv::with_factory(Options::command)
        .var("SSHPING_COMPLETE")
        .complete();

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
        .unwrap_or_else(|e| eprintln!("Failed to initialize logger: {e}"));

    // Get the formatter for output
    let formatter = Formatter::new(opts.human_readable, opts.delimiter);

    let plan = match build_connection_plan(&mut opts) {
        Ok(plan) => plan,
        Err(e) => {
            error!("{e}");
            return ExitCode::FAILURE;
        }
    };

    trace!("Options: {:?}", opts);
    let target = &opts.target;
    debug!("User: {}", target.user);
    debug!("Host: {}", target.host);
    debug!("Port: {}", target.port);

    let connect_start = Instant::now();
    let session = match connect_plan(&plan, opts.ssh_timeout, opts.password.as_deref()).await {
        Ok(session) => session,
        Err(e) => {
            error!("Failed to connect/authenticate: {e}");
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
            &session,
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
                let Ok(span) = records.len().try_into() else {
                    unreachable!("records length always fits in isize")
                };
                modifications.push(((row_count + 1, 0), Span::row(span)));
                row_count += records.len();
                data.extend(records);
            }
            if let Some(result) = speed_test_result {
                let records = result.to_formatted_frame();
                let Ok(span) = records.len().try_into() else {
                    unreachable!("records length always fits in isize")
                };
                modifications.push(((row_count + 1, 0), Span::row(span)));
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
            match serde_json::to_string_pretty(&json) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    error!("Failed to serialize JSON output: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
    }

    // Waiting for key input before exiting
    if opts.key_wait {
        println!("Press enter to exit...");
        let mut buf = [0u8; 1];
        if let Err(e) = std::io::stdin().read(&mut buf) {
            error!("Failed to read keyboard input: {e}");
            return ExitCode::FAILURE;
        }
    }

    // Exit successfully
    ExitCode::SUCCESS
}
