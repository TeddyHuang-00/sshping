use std::{
    io::{Read, Write},
    path::PathBuf,
    time::{Duration, Instant},
};

use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use log::{Level, debug, info, log_enabled, trace, warn};
use rand::{
    distr::{Distribution, Uniform},
    rng,
};
use ssh2::Session;

use crate::{
    summary::{EchoTestSummary, SpeedTestResult, SpeedTestSummary},
    util::Formatter,
};

fn get_progress_bar_style(test_name: &str) -> ProgressStyle {
    ProgressStyle::default_bar()
        .template(
            &format!(
                "{name} {{spinner:.green}} [{{elapsed_precise}}] [{{wide_bar:.cyan/blue}}] {{bytes}}/{{total_bytes}} ({{eta}})",
                name = test_name
            )
        )
        .unwrap()
        .with_key("eta", |state: &ProgressState, w: &mut dyn std::fmt::Write|
            write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap()
        )
        .progress_chars("#>-")
}

pub fn run_echo_test(
    session: &Session,
    echo_cmd: &str,
    char_count: usize,
    time_limit: Option<f64>,
    formatter: &Formatter,
) -> Result<EchoTestSummary, String> {
    info!("Running echo latency test");
    debug!("Running echo test with command: {echo_cmd:?}");
    debug!("Number of characters to echo: {char_count:?}");
    debug!("Time limit for echo: {time_limit:?} seconds");
    // Start the channel server
    trace!("Preparing channel session");
    let mut channel = session.channel_session().map_err(|e| e.to_string())?;
    // Request a pseudo-terminal for the interactive shell
    channel
        .request_pty("sshping", None, Some((10, 5, 0, 0)))
        .map_err(|e| e.to_string())?;
    channel.shell().map_err(|e| e.to_string())?;
    // Send the echo command to accept input
    trace!("Starting echo command");
    let echo_cmd = format!("{echo_cmd}\n");
    channel
        .write_all(echo_cmd.as_bytes())
        .map_err(|e| e.to_string())?;
    channel.flush().map_err(|e| e.to_string())?;
    // Read the initial buffer to clear the echo command
    let mut buffer = [0; 1500];
    channel.read(&mut buffer).map_err(|e| e.to_string())?;

    // Prepare the echo test
    trace!("Testing echo latency");
    let write_buffer = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut read_buffer = [0; 1];
    let mut latencies = Vec::with_capacity(char_count);
    let timeout = time_limit.map(|time| Duration::from_secs_f64(time));
    let start_time = Instant::now();
    let progress_bar = ProgressBar::new(char_count as u64);
    progress_bar.set_style(get_progress_bar_style("Echo test"));

    for (n, idx) in (0..char_count).zip((0..write_buffer.len()).cycle()) {
        let start = Instant::now();
        channel
            .write_all(&write_buffer[idx..idx + 1])
            .map_err(|e| e.to_string())?;
        channel
            .read_exact(&mut read_buffer)
            .map_err(|e| e.to_string())?;
        let latency = start.elapsed().as_nanos();
        latencies.push(latency);
        if let Some(timeout) = timeout {
            if start_time.elapsed() > timeout {
                break;
            }
        }
        progress_bar.set_position((n as u64) + 1);
    }
    progress_bar.finish_and_clear();

    // Calculate latency statistics
    latencies.sort();
    let result = EchoTestSummary::from_latencies(&latencies, formatter);
    if result.char_sent == 0 {
        return Err("Unable to get any echos in given time".to_string());
    }
    if result.char_sent < 20 {
        warn!("Insufficient data points for accurate latency measurement");
    }

    if log_enabled!(Level::Info) {
        let p1_latency = Duration::from_nanos(
            latencies
                .iter()
                .rev()
                .nth(result.char_sent / 100)
                .unwrap()
                .to_owned() as u64,
        );
        let p5_latency = Duration::from_nanos(
            latencies
                .iter()
                .rev()
                .nth(result.char_sent / 20)
                .unwrap()
                .to_owned() as u64,
        );
        let p10_latency = Duration::from_nanos(
            latencies
                .iter()
                .rev()
                .nth(result.char_sent / 10)
                .unwrap()
                .to_owned() as u64,
        );
        info!(
            "Sent {}/{char_count}, Latency:\n\tMean:\t{}\n\tStd:\t{}\n\tMin:\t{}\n\tMedian:\t{}\n\tMax:\t{}\n\t1% High:\t{}\n\t5% High:\t{}\n\t10% High:\t{}",
            result.char_sent,
            result.avg_latency,
            result.std_latency,
            result.min_latency,
            result.med_latency,
            result.max_latency,
            formatter.format_duration(p1_latency),
            formatter.format_duration(p5_latency),
            formatter.format_duration(p10_latency)
        );
    }
    Ok(result)
}

fn run_upload_test(
    session: &Session,
    size: u64,
    chunk_size: u64,
    remote_file: &PathBuf,
    formatter: &Formatter,
) -> Result<SpeedTestResult, String> {
    info!("Running upload speed test");
    // Prepare the upload test
    trace!("Establishing SCP channel");
    let mut channel = session
        .scp_send(&remote_file, 0o644, size, None)
        .map_err(|e| e.to_string())?;
    // Generate random data to upload
    trace!("Generating random data");
    let dist = Uniform::try_from(0..128_u8).unwrap();
    let buffer = dist
        .sample_iter(rng())
        .take(size as usize)
        .map(|v| ((v & 0x3f) + 32) as char)
        .collect::<String>();
    // Preparing logging variables
    let mut total_bytes_sent = 0;
    let start_time: Instant = Instant::now();
    let progress_bar = ProgressBar::new(size);
    progress_bar.set_style(get_progress_bar_style("Upload test"));

    // Starting uploading file
    trace!("Sending file in chunks");
    for chunk in buffer.as_bytes().chunks(chunk_size as usize) {
        channel.write_all(chunk).map_err(|e| e.to_string())?;
        total_bytes_sent += chunk.len();
        progress_bar.set_position(total_bytes_sent as u64);
    }
    progress_bar.finish_and_clear();
    // Clean up the channel
    channel.send_eof().map_err(|e| e.to_string())?;

    let result = SpeedTestResult::new(total_bytes_sent as u64, start_time.elapsed(), formatter);
    info!(
        "Sent {}, Time Elapsed: {}, Average Speed: {}",
        result.size, result.time, result.speed
    );

    Ok(result)
}

fn run_download_test(
    session: &Session,
    chunk_size: u64,
    remote_file: &PathBuf,
    formatter: &Formatter,
) -> Result<SpeedTestResult, String> {
    info!("Running download speed test");
    // Prepare the upload test
    trace!("Establishing SCP channel");
    let (mut channel, stat) = session.scp_recv(&remote_file).map_err(|e| e.to_string())?;
    let size = stat.size();
    if size == 0 {
        return Err("Remote file is empty".to_string());
    }
    // Prepare buffer for downloading
    trace!("Preparing buffer for downloading");
    let mut buffer = vec![0; chunk_size as usize];
    // Preparing logging variables
    let mut total_bytes_recv = 0;
    let start_time: Instant = Instant::now();
    let progress_bar = ProgressBar::new(size);
    progress_bar.set_style(get_progress_bar_style("Download test"));

    // Starting downloading file
    trace!("Receiving file in chunks");
    while size - total_bytes_recv > chunk_size {
        channel.read_exact(&mut buffer).map_err(|e| e.to_string())?;
        total_bytes_recv += chunk_size;
        progress_bar.set_position(total_bytes_recv as u64);
    }
    if size - total_bytes_recv > 0 {
        total_bytes_recv += channel
            .read_to_end(&mut buffer)
            .map_err(|e| e.to_string())? as u64;
        progress_bar.set_position(total_bytes_recv as u64);
    }
    progress_bar.finish_and_clear();
    // Clean up the channel
    channel.send_eof().map_err(|e| e.to_string())?;

    let result = SpeedTestResult::new(total_bytes_recv as u64, start_time.elapsed(), formatter);
    info!(
        "Received {}, Time Elapsed: {}, Average Speed: {}",
        result.size, result.time, result.speed
    );

    Ok(result)
}

pub fn run_speed_test(
    session: &Session,
    size: u64,
    chunk_size: u64,
    remote_file: &PathBuf,
    formatter: &Formatter,
) -> Result<SpeedTestSummary, String> {
    info!("Running speed test");
    debug!(
        "Running speed test with file size: {}",
        formatter.format_size(size)
    );
    debug!("Remote file path: {remote_file:?}");

    let upload_result = run_upload_test(session, size, chunk_size, remote_file, formatter)?;
    let download_result = run_download_test(session, chunk_size, remote_file, formatter)?;
    Ok(SpeedTestSummary {
        upload: upload_result,
        download: download_result,
    })
}
