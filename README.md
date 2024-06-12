# SSH ping

A continuation and re-work of the original [ssh-ping](https://github.com/spook/sshping) in Rust.

This is a WIP project, documentation will be updated as the project progresses.

## Usage

```sh
SSH-based ping that measures interactive character echo latency and file transfer throughput. Pronounced "shipping".

Usage: sshping [OPTIONS] <TARGET>

Arguments:
  <TARGET>  [user@]host[:port]

Options:
  -b, --bind-addr <SOURCE>      Bind to this SOURCE address
  -f, --config <FILE>           Read the ssh config file FILE for options [default: ~/.ssh/config]
  -i, --identity <FILE>         Use identity FILE, i.e., ssh private key file
  -p, --password <PWD>          Use password PWD for authentication (not recommended)
  -T, --ssh-timeout <SECONDS>   Time limit for ssh connection in seconds [default: 10]
  -r, --run-tests <TEST>        Run TEST [default: both] [possible values: echo, speed, both]
  -c, --char-count <COUNT>      Number of characters to echo [default: 1000]
  -e, --echo-cmd <CMD>          Use CMD for echo command [default: "cat > /dev/null"]
  -t, --echo-timeout <SECONDS>  Time limit for echo test in seconds
  -s, --size <SIZE>             File SIZE for speed test in megabytes [default: 8]
  -z, --remote-file <FILE>      Remote FILE path for speed tests [default: /tmp/sshping-PID.tmp]
  -P, --ping-summary            Append measurement in ping-like rtt format
  -H, --human-readable          Use human-friendly units
  -d, --delimit                 Use delimiters in big numbers, e.g., 1,234,567
  -k, --key-wait                Wait for keyboard input before exiting
  -v, --verbose...              Show verbose output, use multiple for more noise
  -h, --help                    Print help (see more with '--help')
  -V, --version                 Print version
```

## Notes

Using public-private key pair is recommended, but you may need to run the following command first (assuming your private key is `~/.ssh/id_rsa`):

```sh
ssh-add ~/.ssh/id_rsa
```

---

Many features like `ProxyJump` are currently not supported due to the limitation of upstream libraries.

---

If more than one identity file is given in the configuration file, only the first one will be used.
