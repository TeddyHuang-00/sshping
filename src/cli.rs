use std::path::PathBuf;

use bytesize::ByteSize;
use clap::{
    builder::{styling::AnsiColor, Styles},
    crate_authors, crate_description, crate_name, crate_version, ArgAction, Parser, ValueEnum,
    ValueHint,
};
use clap_complete::engine::ArgValueCompleter;
use regex::Regex;
use shellexpand::tilde;
use tabled::{settings::Style, Table};
use whoami::username;

use crate::completer::complete_host;

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
            Self::Empty => table.with(Style::empty()),
            Self::Blank => table.with(Style::blank()),
            Self::ASCII => table.with(Style::ascii()),
            Self::PSQL => table.with(Style::psql()),
            Self::Markdown => table.with(Style::markdown()),
            Self::Modern => table.with(Style::modern()),
            Self::Sharp => table.with(Style::sharp()),
            Self::Extended => table.with(Style::extended()),
            Self::Dots => table.with(Style::dots()),
            Self::RST => table.with(Style::re_structured_text()),
            Self::Rounded => table.with(Style::rounded()),
            Self::ASCIIRounded => table.with(Style::ascii_rounded()),
            Self::ModernRounded => table.with(Style::modern_rounded()),
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
    #[arg(
        value_parser = parse_target,
        value_hint = ValueHint::Hostname,
        group = "main_action",
        add = ArgValueCompleter::new(complete_host)
    )]
    pub target: Target,

    /// Read the ssh config file FILE for options
    ///
    /// Per endpoint (target and each ProxyJump hop), sshping resolves:
    /// user/host/port and the first IdentityFile from matching Host config.
    ///
    /// Identity precedence per endpoint:
    /// Host IdentityFile > --identity > default SSH key discovery.
    ///
    /// NOTE: Some options (for example BindAddress) are not supported
    #[arg(
        short = 'f',
        long,
        value_name = "FILE",
        default_value = "~/.ssh/config",
        value_parser = parse_local_path,
        value_hint = ValueHint::FilePath
    )]
    pub config: Option<PathBuf>,

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
    ///
    /// This is used per endpoint only when no Host-specific IdentityFile
    /// exists.
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
        default_value = "cat",
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
        let user = cap
            .get(1)
            .map_or_else(username, |m| Ok(m.as_str().to_string()))
            .map_err(|e| format!("Error: {e}"))?;
        let host = cap.get(2).unwrap().as_str().to_string();
        let port = cap.get(3).map_or(Ok(22), |m| {
            m.as_str()
                .parse()
                .map_err(|e| format!("Invalid port '{}': {e}", m.as_str()))
        })?;
        Ok(Target { user, host, port })
    } else {
        Err("Invalid target format. Must be [user@]host[:port]".to_string())
    }
}

fn parse_local_path(s: &str) -> Result<PathBuf, String> {
    match PathBuf::from(tilde(s).to_string()) {
        path if !path.exists() => Ok(path),
        path => path
            .canonicalize()
            .map_err(|e| format!("Failed to parse path '{}': {e}", path.display())),
    }
}

fn parse_file_size(s: &str) -> Result<u64, String> {
    s.parse::<ByteSize>()
        .map(|size| size.0)
        .map_err(|e| format!("Invalid file size '{s}': {e}"))
}

const fn get_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Green.on_default().bold())
        .usage(AnsiColor::Green.on_default().bold())
        .literal(AnsiColor::Cyan.on_default().bold())
        .placeholder(AnsiColor::Blue.on_default())
        .error(AnsiColor::Red.on_default().bold())
        .valid(AnsiColor::Green.on_default().bold())
        .invalid(AnsiColor::Yellow.on_default().bold())
}

#[cfg(test)]
mod tests {
    use super::{parse_file_size, parse_local_path, parse_target};

    #[test]
    fn parse_target_rejects_out_of_range_port() {
        let result = parse_target("user@localhost:70000");
        assert!(result.is_err());
    }

    #[test]
    fn parse_file_size_rejects_invalid_input() {
        let result = parse_file_size("not-a-size");
        assert!(result.is_err());
    }

    #[test]
    fn parse_local_path_allows_non_existing_path() {
        let result = parse_local_path("/tmp/sshping-path-does-not-exist");
        assert!(result.is_ok());
    }
}
