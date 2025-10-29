# SSH ping

![Crates.io Version](https://img.shields.io/crates/v/sshping)
![Crates.io License](https://img.shields.io/crates/l/sshping)
![Crates.io Total Downloads](https://img.shields.io/crates/d/sshping)
![Crates.io Downloads (latest version)](https://img.shields.io/crates/dv/sshping)
![GitHub Repo stars](https://img.shields.io/github/stars/TeddyHuang-00/sshping)

SSH-based ping that measures interactive character echo latency and file transfer throughput. Pronounced "shipping".

> A continuation and re-work of the original [ssh-ping](https://github.com/spook/sshping) in Rust.

## Installation

### Cargo

`sshping` is published on [crates.io](https://crates.io/crates/sshping), you can install it with (first having rust toolchain installed):

```sh
cargo install sshping
```

### Homebrew (macOS/Linux)

`sshping` is also available on Homebrew/Linuxbrew, you can install it with:

```sh
brew install TeddyHuang-00/app/sshping
```

### Pre-built binaries

Pre-built binaries are available on the [releases page](https://github.com/TeddyHuang-00/sshping/releases). You can download the binary for your platform and put it in your `$PATH`.

### From source

To install from source, you can clone the repository first:

```sh
git clone git@github.com:TeddyHuang-00/sshping.git
# Or
git clone https://github.com/TeddyHuang-00/sshping.git

# Then
cd sshping
```

Then build and install it with cargo:

```sh
cargo install --path .
```

## Usage

```sh
Usage: sshping [OPTIONS] <TARGET>

Arguments:
  <TARGET>  [user@]host[:port]

Options:
  -f, --config <FILE>            Read the ssh config file FILE for options [default: ~/.ssh/config]
  -o, --format <FORMAT>          Output format [default: table] [possible values: table, json]
  -i, --identity <FILE>          Use identity FILE, i.e., ssh private key file
  -p, --password <PWD>           Use password PWD for authentication (not recommended)
  -T, --ssh-timeout <SECONDS>    Time limit for ssh connection in seconds [default: 10]
  -r, --run-tests <TEST>         Run TEST [default: both] [possible values: echo, speed, both]
  -c, --char-count <COUNT>       Number of characters to echo [default: 1000]
  -e, --echo-cmd <CMD>           Use CMD for echo command [default: "cat > /dev/null"]
  -t, --echo-timeout <SECONDS>   Time limit for echo test in seconds
  -s, --size <SIZE>              File SIZE for speed test [default: 8.0MB]
  -u, --chunk-size <CHUNK_SIZE>  Chunk SIZE for splitting file in speed test [default: 1.0MB]
  -z, --remote-file <FILE>       Remote FILE path for speed tests [default: /tmp/sshping-test.tmp]
  -b, --table-style <STYLE>      Table style for output [default: ascii] [possible values: empty, blank, ascii, psql, markdown, modern, sharp, extended, dots, rst, rounded, ascii-rounded, modern-rounded]
  -d, --delimiter <DELIMITER>    Specify delimiters to use (or None for not using) in big numbers [default: ,]
  -H, --human-readable           Use human-friendly units
  -k, --key-wait                 Wait for keyboard input before exiting
  -v, --verbose...               Show verbose output, use multiple for more noise
  -h, --help                     Print help (see more with '--help')
  -V, --version                  Print version
```

## Examples

Ping a host from ssh config with human-readable output and modern table style with rounded corners:

```sh
$ sshping OverLAN -H -b modern-rounded
╭─────────┬───────────────┬─────────────╮
│  Test   │    Metric     │   Result    │
├─────────┼───────────────┼─────────────┤
│   SSH   │ Connect time  │ 49ms 775us  │
├─────────┼───────────────┼─────────────┤
│         │    Average    │ 177us 731ns │
│         ├───────────────┼─────────────┤
│         │ Std deviation │ 59us 706ns  │
│         ├───────────────┼─────────────┤
│ Latency │    Median     │ 203us 263ns │
│         ├───────────────┼─────────────┤
│         │    Minimum    │ 11us 387ns  │
│         ├───────────────┼─────────────┤
│         │    Maximum    │ 270us 20ns  │
├─────────┼───────────────┼─────────────┤
│         │    Upload     │  153 MB/s   │
│  Speed  ├───────────────┼─────────────┤
│         │   Download    │  89.2 MB/s  │
╰─────────┴───────────────┴─────────────╯
```

Ping a certain host with username and port, using `_` as delimiter and a specific identity file:

```sh
$ sshping user@host:7890 -i ~/.ssh/id_rsa -d _
+---------+---------------+-----------------+
|  Test   |    Metric     |     Result      |
+---------+---------------+-----------------+
|   SSH   | Connect time  |  49_725_720ns   |
+---------+---------------+-----------------+
|         |    Average    |    10_268ns     |
|         +---------------+-----------------+
|         | Std deviation |     3_055ns     |
|         +---------------+-----------------+
| Latency |    Median     |     9_773ns     |
|         +---------------+-----------------+
|         |    Minimum    |     8_075ns     |
|         +---------------+-----------------+
|         |    Maximum    |    40_603ns     |
+---------+---------------+-----------------+
|         |    Upload     | 127_897_360 B/s |
|  Speed  +---------------+-----------------+
|         |   Download    | 94_500_777 B/s  |
+---------+---------------+-----------------+
```

Output results in JSON format:

```sh
$ sshping user@host -H -o json
{
  "echo_test": {
    "avg_latency": "12us 135ns",
    "char_sent": 1000,
    "max_latency": "70us 550ns",
    "med_latency": "10us 943ns",
    "min_latency": "6us 690ns",
    "std_latency": "4us 580ns"
  },
  "speed_test": {
    "download": {
      "size": "8.00 MB",
      "speed": "88.4 MB/s",
      "time": "90ms 516us"
    },
    "upload": {
      "size": "8.00 MB",
      "speed": "123 MB/s",
      "time": "64ms 781us"
    }
  },
  "ssh_connect_time": "35ms 998us"
}
```

## Contributing

Contributions are welcome! Feel free to open an issue or a pull request. Anything from bug report to feature request to code contribution is appreciated.

Currently, there are a few things that can be added but haven't been yet. If you would like to help but don't know where to start, you can check this list below:

- [x] Table style customization
- [ ] Unit test
- [ ] Man page generation
- [x] Shell autocompletion script generation
- [ ] Packaging for various platforms
- [ ] More SSH tests
- [ ] Better error handling
- [ ] Code optimization

## FAQ

### How to use public-private key pair for authentication?

Using public-private key pair is recommended. Provide the identity file (private key) path through the `-i` argument:

```sh
sshping user@host -i ~/.ssh/id_rsa
```

Note: SSH agent authentication is not currently supported. Support may be added in future releases as the underlying russh library evolves.

### Why isn't XXX functionality of SSH supported?

Many features like `BindAddress` are currently not supported due to the limitation of upstream libraries. However, `ProxyCommand` is now supported through the SSH configuration file.

If they got implemented in the upstream libraries, they will be added to this project as well. Or you can open a pull request to add them yourself!

### Why isn't all my identity file in SSH config being used?

If more than one identity file is given in the configuration file, only the first one will be used. This is an opinionated design choice to keep the implementation simple.

### Shell autocompletion doesn't work

Make sure you have downloaded the completion script and sourced it in your shell profile, or place it in the appropriate directory for your shell.
