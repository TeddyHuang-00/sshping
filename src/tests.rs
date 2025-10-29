use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use log::{debug, info, log_enabled, trace, warn, Level};
use rand::{
    distr::{Distribution, Uniform},
    rng,
};
use russh::client;
use russh::ChannelMsg;
use russh_sftp::client::SftpSession;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

pub async fn run_echo_test<H: client::Handler>(
    session: &mut client::Handle<H>,
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
    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|e| e.to_string())?;
    
    // Request a pseudo-terminal for the interactive shell
    channel
        .request_pty(true, "sshping", 10, 5, 0, 0, &[])
        .await
        .map_err(|e| e.to_string())?;
    
    channel
        .request_shell(false)
        .await
        .map_err(|e| e.to_string())?;
    
    // Send the echo command to accept input
    trace!("Starting echo command");
    let echo_cmd_bytes = format!("{echo_cmd}\n").into_bytes();
    channel
        .data(&echo_cmd_bytes[..])
        .await
        .map_err(|e| e.to_string())?;
    
    // Read the initial buffer to clear the echo command
    tokio::time::sleep(Duration::from_millis(100)).await;
    while let Some(msg) = channel.wait().await {
        match msg {
            ChannelMsg::Data { .. } => break,
            ChannelMsg::Eof => return Err("Channel closed unexpectedly".to_string()),
            _ => {}
        }
    }

    // Prepare the echo test
    trace!("Testing echo latency");
    let write_buffer = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut latencies = Vec::with_capacity(char_count);
    let timeout = time_limit.map(Duration::from_secs_f64);
    let start_time = Instant::now();
    let progress_bar = ProgressBar::new(char_count as u64);
    progress_bar.set_style(get_progress_bar_style("Echo test"));

    for (n, idx) in (0..char_count).zip((0..write_buffer.len()).cycle()) {
        let start = Instant::now();
        
        // Send one character
        let byte_slice = &write_buffer[idx..idx + 1];
        channel
            .data(byte_slice)
            .await
            .map_err(|e| e.to_string())?;
        
        // Wait for echo back
        loop {
            if let Some(msg) = channel.wait().await {
                match msg {
                    ChannelMsg::Data { data } => {
                        if !data.is_empty() {
                            break;
                        }
                    }
                    ChannelMsg::Eof => {
                        return Err("Channel closed unexpectedly".to_string());
                    }
                    _ => {}
                }
            }
        }
        
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

async fn run_upload_test<H: client::Handler>(
    session: &mut client::Handle<H>,
    size: u64,
    chunk_size: u64,
    remote_file: &Path,
    formatter: &Formatter,
) -> Result<SpeedTestResult, String> {
    info!("Running upload speed test");
    
    // Establish SFTP channel
    trace!("Establishing SFTP channel");
    let channel = session
        .channel_open_session()
        .await
        .map_err(|e| e.to_string())?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| e.to_string())?;
    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| e.to_string())?;
    
    // Generate random data to upload
    trace!("Generating random data");
    let dist = Uniform::try_from(0..128_u8).unwrap();
    let buffer: Vec<u8> = dist
        .sample_iter(rng())
        .take(size as usize)
        .map(|v| (v & 0x3f) + 32)
        .collect();
    
    // Open remote file for writing
    let remote_path = remote_file.to_str().ok_or("Invalid remote file path")?;
    let mut file = sftp
        .create(remote_path)
        .await
        .map_err(|e| e.to_string())?;
    
    // Preparing logging variables
    let mut total_bytes_sent = 0;
    let start_time: Instant = Instant::now();
    let progress_bar = ProgressBar::new(size);
    progress_bar.set_style(get_progress_bar_style("Upload test"));

    // Starting uploading file
    trace!("Sending file in chunks");
    for chunk in buffer.chunks(chunk_size as usize) {
        file.write_all(chunk).await.map_err(|e| e.to_string())?;
        total_bytes_sent += chunk.len();
        progress_bar.set_position(total_bytes_sent as u64);
    }
    progress_bar.finish_and_clear();
    
    // Close the file
    file.shutdown().await.map_err(|e| e.to_string())?;

    let result = SpeedTestResult::new(total_bytes_sent as u64, start_time.elapsed(), formatter);
    info!(
        "Sent {}, Time Elapsed: {}, Average Speed: {}",
        result.size, result.time, result.speed
    );

    Ok(result)
}

async fn run_download_test<H: client::Handler>(
    session: &mut client::Handle<H>,
    chunk_size: u64,
    remote_file: &Path,
    formatter: &Formatter,
) -> Result<SpeedTestResult, String> {
    info!("Running download speed test");
    
    // Establish SFTP channel
    trace!("Establishing SFTP channel");
    let channel = session
        .channel_open_session()
        .await
        .map_err(|e| e.to_string())?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| e.to_string())?;
    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| e.to_string())?;
    
    // Get file size
    let remote_path = remote_file.to_str().ok_or("Invalid remote file path")?;
    let metadata = sftp
        .metadata(remote_path)
        .await
        .map_err(|e| e.to_string())?;
    let size = metadata.len();
    
    if size == 0 {
        return Err("Remote file is empty".to_string());
    }
    
    // Open remote file for reading
    let mut file = sftp
        .open(remote_path)
        .await
        .map_err(|e| e.to_string())?;
    
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
        file.read_exact(&mut buffer).await.map_err(|e| e.to_string())?;
        total_bytes_recv += chunk_size;
        progress_bar.set_position(total_bytes_recv);
    }
    if size - total_bytes_recv > 0 {
        let mut remaining = vec![0; (size - total_bytes_recv) as usize];
        file.read_exact(&mut remaining).await.map_err(|e| e.to_string())?;
        total_bytes_recv += remaining.len() as u64;
        progress_bar.set_position(total_bytes_recv);
    }
    progress_bar.finish_and_clear();

    let result = SpeedTestResult::new(total_bytes_recv, start_time.elapsed(), formatter);
    info!(
        "Received {}, Time Elapsed: {}, Average Speed: {}",
        result.size, result.time, result.speed
    );

    Ok(result)
}

pub async fn run_speed_test<H: client::Handler>(
    session: &mut client::Handle<H>,
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

    let upload_result = run_upload_test(session, size, chunk_size, remote_file, formatter).await?;
    let download_result = run_download_test(session, chunk_size, remote_file, formatter).await?;
    Ok(SpeedTestSummary {
        upload: upload_result,
        download: download_result,
    })
}
