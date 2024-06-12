use std::path::PathBuf;

use log::debug;
use ssh2::Session;

pub fn run_echo_test(
    session: &Session,
    echo_cmd: &str,
    char_count: usize,
    echo_timeout: Option<f64>,
) {
    debug!("Running echo test with command: {echo_cmd:?}");
    debug!("Number of characters to echo: {char_count:?}");
    debug!("Timeout for echo: {echo_timeout:?} seconds");
    // TODO: Implement echo test
}

pub fn run_speed_test(session: &Session, size: f64, remote_file: &PathBuf) {
    debug!("Running speed test with file size: {size} MB");
    debug!("Remote file path: {remote_file:?}");
    // TODO: Implement speed test
}
