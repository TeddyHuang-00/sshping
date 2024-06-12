use crate::util::Formatter;
use log::{debug, info, log_enabled, trace, warn, Level};
use ssh2::Session;
use std::{
    io::{Read, Write},
    path::PathBuf,
    time::{Duration, Instant},
};

pub struct EchoTestResult {
    pub char_count: usize,
    pub char_sent: usize,
    pub avg_latency: Duration,
    pub std_latency: Duration,
    pub med_latency: Duration,
    pub min_latency: Duration,
    pub max_latency: Duration,
}

pub fn run_echo_test(
    session: &Session,
    echo_cmd: &str,
    char_count: usize,
    time_limit: Option<f64>,
    formatter: &Formatter,
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
            let avg_latency = Duration::from_nanos((total_latency as u64) / ((n + 1) as u64));
            let min_latency =
                Duration::from_nanos(latencies.iter().min().unwrap().to_owned() as u64);
            let max_latency =
                Duration::from_nanos(latencies.iter().max().unwrap().to_owned() as u64);
            let log = format!(
                "Ping {n}/{char_count}, Average Latency: {}",
                formatter.format_duration(avg_latency)
            );
            print!("{log:<80}\r");
        }
    }

    let char_sent = latencies.len();
    if char_sent == 0 {
        return Err("Unable to get any echos in given time".to_string());
    }
    if char_sent < 20 {
        warn!("Insufficient data points for accurate latency measurement");
    }

    // Calculate latency statistics
    latencies.sort();
    let avg_latency = latencies.iter().sum::<u128>() / (char_sent as u128);
    let std_latency = Duration::from_nanos(
        ((latencies
            .iter()
            .map(|&latency| ((latency as i128) - (avg_latency as i128)).pow(2))
            .sum::<i128>() as f64)
            / (char_sent as f64))
            .sqrt() as u64,
    );
    let avg_latency = Duration::from_nanos(avg_latency as u64);
    let med_latency = Duration::from_nanos(
        (match char_sent % 2 {
            0 => (latencies[char_sent / 2 - 1] + latencies[char_sent / 2]) / 2,
            _ => latencies[char_sent / 2],
        }) as u64,
    );
    let min_latency = Duration::from_nanos(latencies.first().unwrap().to_owned() as u64);
    let max_latency = Duration::from_nanos(latencies.last().unwrap().to_owned() as u64);

    if log_enabled!(Level::Info) {
        let p1_latency = Duration::from_nanos(
            latencies
                .iter()
                .rev()
                .nth(char_sent / 100)
                .unwrap()
                .to_owned() as u64,
        );
        let p5_latency = Duration::from_nanos(
            latencies
                .iter()
                .rev()
                .nth(char_sent / 20)
                .unwrap()
                .to_owned() as u64,
        );
        let p10_latency = Duration::from_nanos(
            latencies
                .iter()
                .rev()
                .nth(char_sent / 10)
                .unwrap()
                .to_owned() as u64,
        );
        info!(
            "Sent {char_sent}/{char_count}, Latency:\n\tMean:\t{}\n\tStd:\t{}\n\tMin:\t{}\n\tMedian:\t{}\n\tMax:\t{}\n\t1% High:\t{}\n\t5% High:\t{}\n\t10% High:\t{}",
            formatter.format_duration(avg_latency),
            formatter.format_duration(std_latency),
            formatter.format_duration(min_latency),
            formatter.format_duration(med_latency),
            formatter.format_duration(max_latency),
            formatter.format_duration(p1_latency),
            formatter.format_duration(p5_latency),
            formatter.format_duration(p10_latency)
        );
    }
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

pub fn run_speed_test(
    session: &Session,
    size: u64,
    remote_file: &PathBuf,
    formatter: &Formatter,
) -> Result<(), String> {
    debug!(
        "Running speed test with file size: {}",
        formatter.format_size(size)
    );
    debug!("Remote file path: {remote_file:?}");
    // TODO: Implement speed test
    Ok(())
}
