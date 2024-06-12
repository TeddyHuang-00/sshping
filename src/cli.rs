// Import necessary crates
extern crate clap;
extern crate shellexpand;
extern crate users;
use std::path::PathBuf;
use users::get_current_username;

use clap::{
    crate_authors, crate_description, crate_name, crate_version, ArgAction, Parser, ValueEnum,
};

// Define options struct
#[derive(Parser, Debug)]
#[command(name = crate_name!())]
#[command(version = crate_version!())]
#[command(about = crate_description!())]
#[command(long_about = crate_description!())]
#[command(author = crate_authors!())]
pub struct Options {
    /// [user@]host[:port]
    #[arg(value_parser = parse_target)]
    pub target: Target,

    /// Bind to this SOURCE address
    #[arg(short, long, value_name = "SOURCE")]
    pub bind_addr: Option<String>,

    /// Read the ssh config file FILE for options
    #[arg(
        short = 'f',
        long,
        value_name = "FILE",
        default_value = PathBuf::from("~/.ssh/config").into_os_string(),
        value_parser = parse_local_path
    )]
    pub config: PathBuf,

    /// Use identity FILE, i.e., ssh private key file
    #[arg(short, long, value_name = "FILE", value_parser = parse_local_path)]
    pub identity: Option<PathBuf>,

    /// Use password PWD for authentication (not recommended)
    #[arg(short, long, value_name = "PWD")]
    pub password: Option<String>,

    /// Time limit for ssh connection in seconds
    #[arg(short = 'T', long, value_name = "SECONDS", default_value_t = 10.0)]
    pub ssh_timeout: f64,

    /// Run TEST
    #[arg(short, long, value_enum, value_name = "TEST", default_value_t = Test::Both)]
    pub run_tests: Test,

    /// Number of characters to echo
    #[arg(short, long, value_name = "COUNT", default_value_t = 1000)]
    pub char_count: usize,

    /// Use CMD for echo command
    #[arg(short, long, value_name = "CMD", default_value = "cat > /dev/null")]
    pub echo_cmd: String,

    /// Time limit for echo test in seconds
    #[arg(short = 't', long, value_name = "SECONDS")]
    pub echo_timeout: Option<f64>,

    /// File SIZE for speed test in megabytes
    #[arg(short, long, default_value_t = 8.0)]
    pub size: f64,

    /// Remote FILE path for speed tests
    #[arg(
        short = 'z',
        long,
        value_name = "FILE",
        default_value = "/tmp/sshping-PID.tmp"
    )]
    pub remote_file: PathBuf,

    /// Append measurement in ping-like rtt format
    #[arg(short = 'P', long)]
    pub ping_summary: bool,

    /// Use human-friendly units
    #[arg(short = 'H', long)]
    pub human_readable: bool,

    /// Specify delimiters to use in big numbers, e.g., 1,234,567
    #[arg(short, long, default_value = ",")]
    pub delimit: Option<char>,

    /// Wait for keyboard input before exiting
    #[arg(short, long)]
    pub key_wait: bool,

    /// Show verbose output, use multiple for more noise
    #[arg(short, long, action = ArgAction::Count)]
    pub verbose: u8,
}

#[derive(ValueEnum, Clone, PartialEq, Eq, Debug)]
pub enum Test {
    /// Run echo test
    Echo,
    /// Run speed test
    Speed,
    /// Run both echo and speed tests
    Both,
}

#[derive(Clone, Debug)]
pub struct Target {
    pub user: String,
    pub host: String,
    pub port: u16,
}

fn parse_target(s: &str) -> Result<Target, String> {
    let mut parts = s.split('@');
    let user = match parts.clone().count() {
        // Get the current username if not specified
        1 => {
            if let Some(user) = get_current_username() {
                user.to_string_lossy().to_string()
            } else {
                return Err("Failed to get current username".to_string());
            }
        }
        // Or use the specified username
        2 => parts.next().unwrap().to_string(),
        // Throw an error if @ present more than once
        _ => {
            return Err("Invalid target format. Must be [user@]host[:port]".to_string());
        }
    };
    let mut parts = parts.next().unwrap().split(':');
    let host = parts.next().unwrap().to_string();
    let port = match parts.clone().count() {
        // Use default port 22 if not specified
        0 => 22,
        // Or use the specified port
        1 => parts.next().unwrap().parse().unwrap(),
        // Throw an error if : present more than once
        _ => {
            return Err("Invalid target format. Must be [user@]host[:port]".to_string());
        }
    };
    Ok(Target { user, host, port })
}

fn parse_local_path(s: &str) -> Result<PathBuf, String> {
    Ok(PathBuf::from(shellexpand::tilde(s).to_string())
        .canonicalize()
        .expect("Failed to parse path"))
}
