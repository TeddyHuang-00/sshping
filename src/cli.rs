use bytesize::ByteSize;
use clap::{
    builder::{styling::AnsiColor, Styles},
    crate_authors, crate_description, crate_name, crate_version, ArgAction, Parser, ValueEnum,
    ValueHint,
};
use shellexpand::tilde;
use std::path::PathBuf;
use users::get_current_username;

// Define options struct
#[derive(Parser, Debug)]
#[command(name = crate_name!())]
#[command(version = crate_version!())]
#[command(about = crate_description!())]
#[command(long_about = crate_description!())]
#[command(author = crate_authors!())]
#[command(styles = get_styles())]
pub struct Options {
    /// [user@]host[:port]
    #[arg(value_parser = parse_target, value_hint = ValueHint::Hostname)]
    pub target: Target,

    /// Read the ssh config file FILE for options
    #[arg(
        short = 'f',
        long,
        value_name = "FILE",
        default_value = PathBuf::from("~/.ssh/config").into_os_string(),
        value_parser = parse_local_path,
        value_hint = ValueHint::FilePath
    )]
    pub config: PathBuf,

    /// Use identity FILE, i.e., ssh private key file
    #[arg(
        short,
        long,
        value_name = "FILE",
        value_parser = parse_local_path,
        value_hint = ValueHint::FilePath
    )]
    pub identity: Option<PathBuf>,

    /// Use password PWD for authentication (not recommended)
    #[arg(short, long, value_name = "PWD", value_hint = ValueHint::Other)]
    pub password: Option<String>,

    /// Time limit for ssh connection in seconds
    #[arg(
        short = 'T',
        long,
        value_name = "SECONDS",
        default_value_t = 10.0,
        value_hint = ValueHint::Other
    )]
    pub ssh_timeout: f64,

    /// Run TEST
    #[arg(
        short,
        long,
        value_enum,
        value_name = "TEST",
        default_value_t = Test::Both,
        value_hint = ValueHint::Other
    )]
    pub run_tests: Test,

    /// Number of characters to echo
    #[arg(short, long, value_name = "COUNT", default_value_t = 1000, value_hint = ValueHint::Other)]
    pub char_count: usize,

    /// Use CMD for echo command
    #[arg(
        short,
        long,
        value_name = "CMD",
        default_value = "cat > /dev/null",
        value_hint = ValueHint::CommandString
    )]
    pub echo_cmd: String,

    /// Time limit for echo test in seconds
    #[arg(short = 't', long, value_name = "SECONDS", value_hint = ValueHint::Other)]
    pub echo_timeout: Option<f64>,

    /// File SIZE for speed test
    #[arg(
        short,
        long,
        default_value = "8.0MB",
        value_parser = parse_file_size,
        value_hint = ValueHint::Other
    )]
    pub size: u64,

    /// Chunk SIZE for splitting file in speed test
    #[arg(
        short = 'u',
        long,
        default_value = "1.0MB",
        value_parser = parse_file_size,
        value_hint = ValueHint::Other
    )]
    pub chunk_size: u64,

    /// Remote FILE path for speed tests
    #[arg(
        short = 'z',
        long,
        value_name = "FILE",
        default_value = "/tmp/sshping-test.tmp",
        value_hint = ValueHint::FilePath
    )]
    pub remote_file: PathBuf,

    /// Specify delimiters to use (or None for not using) in big numbers
    #[arg(short, long, default_value = ",", value_hint = ValueHint::Other)]
    pub delimiter: Option<char>,

    /// Use human-friendly units
    #[arg(short = 'H', long)]
    pub human_readable: bool,

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
    Ok(PathBuf::from(tilde(s).to_string())
        .canonicalize()
        .expect("Failed to parse path"))
}

fn parse_file_size(s: &str) -> Result<u64, String> {
    let size = s.parse::<ByteSize>().unwrap().0;
    Ok(size)
}

pub fn get_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Green.on_default().bold())
        .usage(AnsiColor::Green.on_default().bold())
        .literal(AnsiColor::Cyan.on_default().bold())
        .placeholder(AnsiColor::Blue.on_default())
        .error(AnsiColor::Red.on_default().bold())
        .valid(AnsiColor::Green.on_default().bold())
        .invalid(AnsiColor::Yellow.on_default().bold())
}
