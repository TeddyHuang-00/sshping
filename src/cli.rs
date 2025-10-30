use std::path::PathBuf;

use bytesize::ByteSize;
use clap::{
    builder::{styling::AnsiColor, Styles},
    crate_authors, crate_description, crate_name, crate_version, ArgAction, Parser, ValueEnum,
    ValueHint,
};
use regex::Regex;
use shellexpand::tilde;
use tabled::{settings::Style, Table};
use whoami::username;

#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum TableStyle {
    Empty,
    Blank,
    ASCII,
    PSQL,
    Markdown,
    Modern,
    Sharp,
    Extended,
    Dots,
    RST,
    Rounded,
    ASCIIRounded,
    ModernRounded,
}

impl TableStyle {
    pub fn stylize<'a>(&self, table: &'a mut Table) -> &'a mut Table {
        match self {
            TableStyle::Empty => table.with(Style::empty()),
            TableStyle::Blank => table.with(Style::blank()),
            TableStyle::ASCII => table.with(Style::ascii()),
            TableStyle::PSQL => table.with(Style::psql()),
            TableStyle::Markdown => table.with(Style::markdown()),
            TableStyle::Modern => table.with(Style::modern()),
            TableStyle::Sharp => table.with(Style::sharp()),
            TableStyle::Extended => table.with(Style::extended()),
            TableStyle::Dots => table.with(Style::dots()),
            TableStyle::RST => table.with(Style::re_structured_text()),
            TableStyle::Rounded => table.with(Style::rounded()),
            TableStyle::ASCIIRounded => table.with(Style::ascii_rounded()),
            TableStyle::ModernRounded => table.with(Style::modern_rounded()),
        }
    }
}

// Define options struct
#[derive(Debug, Parser)]
#[command(name = crate_name!())]
#[command(version = crate_version!())]
#[command(about = crate_description!())]
#[command(long_about = crate_description!())]
#[command(author = crate_authors!())]
#[command(styles = get_styles())]
pub struct Options {
    /// [user@]host[:port]
    #[arg(value_parser = parse_target, value_hint = ValueHint::Hostname, group = "main_action")]
    pub target: Target,

    /// Read the ssh config file FILE for options
    ///
    /// We get the user, host, port, identity file and proxy jump from ssh config
    ///
    /// NOTE: Some options like bind address are not supported
    #[arg(
        short = 'f',
        long,
        value_name = "FILE",
        default_value = PathBuf::from("~/.ssh/config").into_os_string(),
        value_parser = parse_local_path,
        value_hint = ValueHint::FilePath
    )]
    pub config: PathBuf,

    /// Output format
    ///
    /// NOTE: JSON format will not be written to a file. Redirect the
    /// output to a file if needed
    #[arg(
        short = 'o',
        long,
        value_enum,
        value_name = "FORMAT",
        default_value_t = Format::Table,
        value_hint = ValueHint::Other
    )]
    pub format: Format,

    /// Use identity FILE, i.e., ssh private key file
    ///
    /// Typically ~/.ssh/id_<algo> where <algo> is rsa, dsa, ecdsa, etc.
    ///
    /// TIP: If you have already added the key to ssh-agent,
    /// you don't need to specify this
    #[arg(
        short,
        long,
        value_name = "FILE",
        value_parser = parse_local_path,
        value_hint = ValueHint::FilePath
    )]
    pub identity: Option<PathBuf>,

    /// Use password PWD for authentication (not recommended)
    ///
    /// WARNING: Password authentication is not recommended for security reasons
    ///
    /// Please use public key authentication instead where possible
    #[arg(short, long, value_name = "PWD", value_hint = ValueHint::Other)]
    pub password: Option<String>,

    /// Time limit for ssh connection in seconds
    ///
    /// Timeout for all the ssh operations including authentication
    #[arg(
        short = 'T',
        long,
        value_name = "SECONDS",
        default_value_t = 10.0,
        value_hint = ValueHint::Other
    )]
    pub ssh_timeout: f64,

    /// Run TEST
    ///
    /// Echo test: sends a large number of characters to the remote server
    /// and measures the latency
    ///
    /// Speed test: sends/receives a large file through scp
    /// and measures the throughput
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
    ///
    /// Should capture all following input and discard them safely
    ///
    /// NOTE: This command should not exit immediately
    #[arg(
        short,
        long,
        value_name = "CMD",
        default_value = "cat > /dev/null",
        value_hint = ValueHint::CommandString
    )]
    pub echo_cmd: String,

    /// Time limit for echo test in seconds
    ///
    /// Early termination of the echo test if exceeding this time limit
    #[arg(short = 't', long, value_name = "SECONDS", value_hint = ValueHint::Other)]
    pub echo_timeout: Option<f64>,

    /// File SIZE for speed test
    ///
    /// Not recommended to use very small sizes for accurate results
    ///
    /// Examples of possible value: 1.5K(B), 3Mi(B), 0.1Ki(B), 500(B)
    #[arg(
        short,
        long,
        default_value = "8.0MB",
        value_parser = parse_file_size,
        value_hint = ValueHint::Other
    )]
    pub size: u64,

    /// Chunk SIZE for splitting file in speed test
    ///
    /// Use a smaller value for better progress updates
    /// and a larger value for better throughput
    ///
    /// Examples of possible value: 1.5K(B), 3Mi(B), 0.1Ki(B), 500(B)
    #[arg(
        short = 'u',
        long,
        default_value = "1.0MB",
        value_parser = parse_file_size,
        value_hint = ValueHint::Other
    )]
    pub chunk_size: u64,

    /// Remote FILE path for speed tests
    ///
    /// The file will be created on the remote server for the speed test
    ///
    /// NOTE: This file will not be deleted after the test,
    /// so it is recommended to be in /tmp
    #[arg(
        short = 'z',
        long,
        value_name = "FILE",
        default_value = "/tmp/sshping-test.tmp",
        value_hint = ValueHint::FilePath
    )]
    pub remote_file: PathBuf,

    /// Table style for output
    ///
    /// See https://github.com/zhiburt/tabled?tab=readme-ov-file#styles
    /// for examples
    #[arg(
        short = 'b',
        long,
        value_enum,
        value_name = "STYLE",
        default_value_t = TableStyle::ASCII,
        value_hint = ValueHint::Other
    )]
    pub table_style: TableStyle,

    /// Specify delimiters to use (or None for not using) in big numbers
    ///
    /// Used to separate digits in big numbers for better readability
    ///
    /// This option is only used in non- human readable mode
    ///
    /// Examples of possible value: ",", " ", "_", None
    #[arg(short, long, default_value = ",", value_hint = ValueHint::Other)]
    pub delimiter: Option<char>,

    /// Use human-friendly units
    ///
    /// Big numbers will be formatted in human-friendly units
    ///
    /// Examples: 1.5 MB/s, 1s 259ms
    #[arg(short = 'H', long)]
    pub human_readable: bool,

    /// Wait for keyboard input before exiting
    #[arg(short, long)]
    pub key_wait: bool,

    /// Show verbose output, use multiple for more noise
    ///
    /// -v: Show warnings
    ///
    /// -vv: Show info messages
    ///
    /// -vvv: Show debug messages
    ///
    /// -vvvv: Show trace messages
    #[arg(short, long, action = ArgAction::Count)]
    pub verbose: u8,

    /// Execute a remote command instead of running tests
    ///
    /// When specified, the given command will be executed on the remote host
    /// instead of running the echo and speed tests.
    ///
    /// This is useful for integrating sshping with other tools or for
    /// custom remote operations.
    #[arg(short = 'R', long, value_name = "CMD", value_hint = ValueHint::CommandString)]
    pub remote_command: Option<String>,

    /// Connect through one or more jump hosts
    ///
    /// Format: [user@]host[:port][,[user@]host[:port],...]
    ///
    /// Multiple jump hosts can be specified separated by commas.
    /// Each jump host will be connected in sequence to reach the target.
    ///
    /// Example: -J jump1.example.com,user@jump2.example.com:2222
    #[arg(short = 'J', long, value_name = "JUMP_HOST", value_hint = ValueHint::Hostname)]
    pub proxy_jump: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, ValueEnum)]
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

#[derive(Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum Format {
    /// Table in console
    Table,
    /// JSON format
    Json,
}

fn parse_target(s: &str) -> Result<Target, String> {
    let pat = Regex::new(r"^(?:([a-zA-Z0-9_.-]+)@)?([a-zA-Z0-9_.-]+)(?::(\d+))?$").unwrap();
    if let Some(cap) = pat.captures(s) {
        let user = cap.get(1).map_or(username(), |m| m.as_str().to_string());
        let host = cap.get(2).unwrap().as_str().to_string();
        let port = cap.get(3).map_or(22, |m| m.as_str().parse().unwrap());
        Ok(Target { user, host, port })
    } else {
        Err("Invalid target format. Must be [user@]host[:port]".to_string())
    }
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

fn get_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Green.on_default().bold())
        .usage(AnsiColor::Green.on_default().bold())
        .literal(AnsiColor::Cyan.on_default().bold())
        .placeholder(AnsiColor::Blue.on_default())
        .error(AnsiColor::Red.on_default().bold())
        .valid(AnsiColor::Green.on_default().bold())
        .invalid(AnsiColor::Yellow.on_default().bold())
}
