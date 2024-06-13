use crate::summary::{EchoTestSummary, SpeedTestResult, SpeedTestSummary};
use crate::util::Formatter;
use log::{debug, info, log_enabled, trace, warn, Level};
use rand::{
    distributions::{Distribution, Uniform},
    thread_rng,
};
use ssh2::Session;
use std::{
    io::{Read, Write},
    path::PathBuf,
    time::{Duration, Instant},
};

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
    let mut total_latency = 0;
    let write_buffer = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut read_buffer = [0; 1];
    let mut latencies = Vec::with_capacity(char_count);
    let timeout = time_limit.map(|time| Duration::from_secs_f64(time));
    let start_time = Instant::now();
    let mut last_log_time = Instant::now();
    let log_interval = Duration::from_secs_f64(1.0 / 60.0);

    for (n, idx) in (0..char_count).zip((0..write_buffer.len()).cycle()) {
        let start = Instant::now();
        channel
            .write_all(&write_buffer[idx..idx + 1])
            .map_err(|e| e.to_string())?;
        channel
            .read_exact(&mut read_buffer)
            .map_err(|e| e.to_string())?;
        let latency = start.elapsed().as_nanos();
        total_latency += latency;
        latencies.push(latency);
        if let Some(timeout) = timeout {
            if start_time.elapsed() > timeout {
                break;
            }
        }
        if last_log_time.elapsed() > log_interval || log_enabled!(Level::Info) {
            last_log_time = Instant::now();
            let avg_latency = Duration::from_nanos((total_latency as u64) / ((n + 1) as u64));
            let log = format!(
                "Ping {n}/{char_count}, Average Latency: {}",
                formatter.format_duration(avg_latency)
            );
            print!("{log:<80}\r");
        }
    }

    // Calculate latency statistics
    latencies.sort();
    let result = EchoTestSummary::from_latencies(&latencies, char_count, formatter);
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

pub fn run_upload_test(
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
    let dist = Uniform::from(0..128 as u8);
    let buffer = dist
        .sample_iter(thread_rng())
        .take(size as usize)
        .map(|v| ((v & 0x3f) + 32) as char)
        .collect::<String>();
    // Preparing logging variables
    let mut total_bytes_sent = 0;
    let start_time: Instant = Instant::now();
    let mut last_log_time = Instant::now();
    let log_interval = Duration::from_secs_f64(1.0 / 60.0);
    let mut result: SpeedTestResult;

    // Starting uploading file
    trace!("Sending file in chunks");
    for chunk in buffer.as_bytes().chunks(chunk_size as usize) {
        channel.write_all(chunk).map_err(|e| e.to_string())?;
        total_bytes_sent += chunk.len();

        if last_log_time.elapsed() > log_interval || log_enabled!(Level::Info) {
            last_log_time = Instant::now();
            result = SpeedTestResult::new(total_bytes_sent as u64, start_time.elapsed(), formatter);
            let log = format!(
                "Sent {total_bytes_sent}/{size}, Average Speed: {}",
                result.speed
            );
            print!("{log:<80}\r");
        }
    }
    result = SpeedTestResult::new(total_bytes_sent as u64, start_time.elapsed(), formatter);
    // Clean up the channel
    channel.send_eof().map_err(|e| e.to_string())?;

    info!(
        "Sent {}, Time Elapsed: {}, Average Speed: {}",
        result.size, result.time, result.speed
    );

    Ok(result)
}

pub fn run_download_test(
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
    let mut last_log_time = Instant::now();
    let log_interval = Duration::from_secs_f64(1.0 / 60.0);
    let mut result: SpeedTestResult;

    // Starting downloading file
    trace!("Receiving file in chunks");
    while size - total_bytes_recv > chunk_size {
        channel.read_exact(&mut buffer).map_err(|e| e.to_string())?;
        total_bytes_recv += chunk_size;

        if last_log_time.elapsed() > log_interval || log_enabled!(Level::Info) {
            last_log_time = Instant::now();
            result = SpeedTestResult::new(total_bytes_recv as u64, start_time.elapsed(), formatter);
            let log = format!(
                "Received {total_bytes_recv}/{size}, Average Speed: {}",
                result.speed
            );
            print!("{log:<80}\r");
        }
    }
    if size - total_bytes_recv > 0 {
        total_bytes_recv += channel
            .read_to_end(&mut buffer)
            .map_err(|e| e.to_string())? as u64;
    }
    result = SpeedTestResult::new(total_bytes_recv as u64, start_time.elapsed(), formatter);
    // Clean up the channel
    channel.send_eof().map_err(|e| e.to_string())?;

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
