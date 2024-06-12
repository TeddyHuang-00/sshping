use std::{
    io::{Read, Write},
    path::PathBuf,
    time::{Duration, Instant},
};

use log::{debug, info, log_enabled, trace, warn, Level};
use ssh2::Session;

pub struct EchoTestResult {
    pub char_count: usize,
    pub char_sent: usize,
    pub avg_latency: f64,
    pub std_latency: f64,
    pub med_latency: f64,
    pub min_latency: f64,
    pub max_latency: f64,
}

pub fn run_echo_test(
    session: &Session,
    echo_cmd: &str,
    char_count: usize,
    time_limit: Option<f64>,
) -> Result<EchoTestResult, String> {
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
            let avg_latency = total_latency as f64 / (n + 1) as f64;
            let min_latency = latencies.iter().min().unwrap();
            let max_latency = latencies.iter().max().unwrap();
            print!(
                "Ping {n}/{char_count}, Latency: {avg_latency:.2} us (min: {min_latency:.2} us, max: {max_latency:.2} us)\r",
            );
        }
    }
    channel.close().map_err(|e| e.to_string())?;
    channel.wait_close().map_err(|e| e.to_string())?;

    let char_sent = latencies.len();
    if char_sent == 0 {
        return Err("Unable to get any echos in given time".to_string());
    }
    if char_sent < 20 {
        warn!("Insufficient data points for accurate latency measurement");
    }

    // Calculate latency statistics
    latencies.sort();
    let avg_latency = total_latency as f64 / char_sent as f64;
    let std_latency = latencies
        .iter()
        .map(|&latency| (latency as f64 - avg_latency).powi(2))
        .sum::<f64>()
        .sqrt();
    let med_latency = match latencies.len() % 2 {
        0 => (latencies[latencies.len() / 2 - 1] + latencies[latencies.len() / 2]) as f64 / 2.0,
        _ => latencies[latencies.len() / 2] as f64,
    };
    let min_latency = latencies.iter().min().unwrap().to_owned() as f64;
    let max_latency = latencies.iter().max().unwrap().to_owned() as f64;

    info!("Sent {char_sent}/{char_count}, Latency: {avg_latency:.2} us (min: {min_latency:.2} us, max: {max_latency:.2} us)\r");
    Ok(EchoTestResult {
        char_count,
        char_sent,
        avg_latency,
        std_latency,
        med_latency,
        min_latency,
        max_latency,
    })
}

pub fn run_speed_test(session: &Session, size: f64, remote_file: &PathBuf) -> Result<(), String> {
    debug!("Running speed test with file size: {size} MB");
    debug!("Remote file path: {remote_file:?}");
    // TODO: Implement speed test
    Ok(())
}
