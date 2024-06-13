# SSH ping

A continuation and re-work of the original [ssh-ping](https://github.com/spook/sshping) in Rust.

## Installation

### Cargo

`sshping` is published on [crates.io](https://crates.io/crates/sshping), you can install it with (first having rust toolchain installed):

```sh
cargo install sshping
```

You can also opt-in to the `include-openssl` feature to bundle OpenSSL with the binary, for rare cases where the system OpenSSL is not available:

```sh
cargo install sshping -F include-openssl
```

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

As with the `include-openssl` feature, you can also enable it when installing from source:

```sh
cargo install --path . -F include-openssl
```

## Usage

```sh
SSH-based ping that measures interactive character echo latency and file transfer throughput. Pronounced "shipping".

Usage: sshping [OPTIONS] <TARGET>

Arguments:
  <TARGET>  [user@]host[:port]

Options:
  -f, --config <FILE>            Read the ssh config file FILE for options [default: ~/.ssh/config]
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
  -d, --delimiter <DELIMITER>    Specify delimiters to use (or None for not using) in big numbers [default: ,]
  -H, --human-readable           Use human-friendly units
  -k, --key-wait                 Wait for keyboard input before exiting
  -v, --verbose...               Show verbose output, use multiple for more noise
  -h, --help                     Print help (see more with '--help')
  -V, --version                  Print version
```

## Examples

```sh
$ sshping localhost -H
+---------+---------------+-------------+
|  Test   |    Metric     |   Result    |
+---------+---------------+-------------+
|   SSH   | Connect time  | 52ms 872us  |
+---------+---------------+-------------+
|         |    Average    | 39us 436ns  |
|         +---------------+-------------+
|         | Std deviation | 16us 767ns  |
|         +---------------+-------------+
| Latency |    Median     | 31us 208ns  |
|         +---------------+-------------+
|         |    Minimum    | 23us 708ns  |
|         +---------------+-------------+
|         |    Maximum    | 172us 208ns |
+---------+---------------+-------------+
|         |    Upload     |  99.9 MB/s  |
|  Speed  +---------------+-------------+
|         |   Download    |  264 MB/s   |
+---------+---------------+-------------+
```

```sh
$ sshping localhost -d _
+---------+---------------+-----------------+
|  Test   |    Metric     |     Result      |
+---------+---------------+-----------------+
|   SSH   | Connect time  |  55_056_125ns   |
+---------+---------------+-----------------+
|         |    Average    |    45_490ns     |
|         +---------------+-----------------+
|         | Std deviation |    39_150ns     |
|         +---------------+-----------------+
| Latency |    Median     |    27_292ns     |
|         +---------------+-----------------+
|         |    Minimum    |    23_625ns     |
|         +---------------+-----------------+
|         |    Maximum    |    436_875ns    |
+---------+---------------+-----------------+
|         |    Upload     | 100_289_743 B/s |
|  Speed  +---------------+-----------------+
|         |   Download    | 258_341_257 B/s |
+---------+---------------+-----------------+
```

## Contributing

Contributions are welcome! Feel free to open an issue or a pull request. Anything from bug report to feature request to code contribution is appreciated.

## FAQ

### How to use public-private key pair for authentication?

Using public-private key pair is recommended, you can either provide the identity file (private key) path through `-i` argument or use agent authentication by adding the identity file to your ssh-agent (assuming your private key is `~/.ssh/id_rsa`):

```sh
ssh-add ~/.ssh/id_rsa
```

### Why isn't XXX functionality of SSH supported?

Many features like `ProxyJump` and `BindAddress` are currently not supported due to the limitation of upstream libraries.

If they got implemented in the upstream libraries, they will be added to this project as well. Or you can open a pull request to add them yourself!

### Why isn't all my identity file in SSH config being used?

If more than one identity file is given in the configuration file, only the first one will be used. This is an opinionated design choice to keep the implementation simple.
