# Changelog

All notable changes to this project will be documented in this file.

## [0.3.1] - 2026-04-04

### 🚀 Features

- Add runtime completion engine

### 🐛 Bug Fixes

- Stabilize echo test by removing PTY/shell dependency and hardening channel byte handling (#197)

### 🎨 Styling

- Use inclusive slice

### ⚙️ Miscellaneous Tasks

- Pin changelog action version to latest 1.10.3

## [0.3.0] - 2026-04-04

### 🚀 Features

- Add SSH config `ProxyJump` + `ProxyCommand` support via multi-hop authenticated sessions (#194)

### 🐛 Bug Fixes

- [**breaking**] Migrate SSH config resolution from `ssh2-config` to `russh-config` (#193)
- *(deps)* Update rust crate whoami to 2.1.1
- Fix code format and linting errors
- Harden summary parsing and test flow edge cases
- Exclude data generation time from speed transfer timing
- Remove redundant type conversion

### 🚜 Refactor

- Harden parsing auth tests and docs for strict quality pass
- Use invariant let-else for guaranteed span conversion

### ⚙️ Miscellaneous Tasks

- Add format and lint check in CI workflow
- Add config for clippy
- Rename workflow
- Continue strict refactor implementation
- Address code review follow-ups
- Bump version to 0.3.0

## [0.2.4] - 2026-04-04

### 🐛 Bug Fixes

- *(deps)* Update rust crate russh to 0.56.0 (#155)
- *(deps)* Update rust crate russh to 0.57.0 (#168)
- *(deps)* Update rust crate whoami to v2 (#162)
- *(deps)* Update rust crate ssh2-config to 0.7.0 (#172)
- *(deps)* Update rust crate russh to 0.58.0 (#188)
- *(deps)* Update rust crate russh to 0.59.0 (#190)
- *(deps)* Update rust crate russh to 0.60.0 (#192)
- *(deps)* Update rust crate rand to 0.10.0 (#175)

### ⚙️ Miscellaneous Tasks

- Bump version to 0.2.3
- Bump version to 0.2.4

## [0.2.3] - 2025-12-18

### 🚀 Features

- Add interactive password/passphrase prompts for SSH authentication (#140)

### 🐛 Bug Fixes

- *(deps)* Update rust crate clap to v4.5.47 (#111)
- *(deps)* Update rust crate log to v0.4.28 (#112)
- *(deps)* Update rust crate humantime to v2.3.0 (#114)
- *(deps)* Update rust crate serde to v1.0.221 (#115)
- *(deps)* Update rust crate serde_json to v1.0.144 (#116)
- *(deps)* Update rust crate serde_json to v1.0.145 (#117)
- *(deps)* Update rust crate serde to v1.0.223 (#118)
- *(deps)* Update rust crate bytesize to v2.1.0 (#119)
- *(deps)* Update rust crate serde to v1.0.225 (#121)
- *(deps)* Update rust crate clap to v4.5.48 (#122)
- *(deps)* Update rust crate serde to v1.0.226 (#123)
- *(deps)* Update rust crate russh to 0.55.0 (#150)
- Fixes #152 Gracefully handle cases where ssh config doesn't exist

### ⚙️ Miscellaneous Tasks

- Migrate from ssh2 to russh (#136)
- Clean up empty feature
- Update macos runner images

## [0.2.2] - 2025-08-27

### 🐛 Bug Fixes

- *(deps)* Update rust crate clap to v4.5.41 (#90)
- *(deps)* Update rust crate serde_json to v1.0.141 (#91)
- *(deps)* Update rust crate rand to v0.9.2 (#92)
- *(deps)* Update rust crate clap to v4.5.42 (#93)
- *(deps)* Update rust crate serde_json to v1.0.142 (#94)
- *(deps)* Update rust crate clap to v4.5.43 (#95)
- *(deps)* Update rust crate whoami to v1.6.1 (#97)
- *(deps)* Update rust crate clap to v4.5.44 (#100)
- *(deps)* Update rust crate clap to v4.5.45 (#101)
- *(deps)* Update rust crate ssh2-config to 0.6.0 (#103)
- *(deps)* Update rust crate serde_json to v1.0.143 (#104)
- *(deps)* Update rust crate regex to v1.11.2 (#105)
- *(deps)* Update rust crate clap to v4.5.46 (#106)
- Reorder derive attributes for consistency and clarity; update Cargo.toml features section

### ⚙️ Miscellaneous Tasks

- Add configuration for git-cliff and cargo-release; add just recipes
- Add repository field to Cargo.toml

## [0.2.1] - 2025-07-09

### 🐛 Bug Fixes

- *(deps)* Update rust crate bytesize to v1.3.1 (#47)
- *(deps)* Update rust crate bytesize to v1.3.2 (#48)
- *(deps)* Update rust crate clap to v4.5.29 (#49)
- *(deps)* Update rust crate clap to v4.5.30 (#51)
- *(deps)* Update rust crate serde to v1.0.218 (#52)
- *(deps)* Update rust crate serde_json to v1.0.139 (#53)
- *(deps)* Update rust crate log to v0.4.26 (#54)
- *(deps)* Update rust crate clap to v4.5.31 (#57)
- *(deps)* Update rust crate bytesize to v2 (#58)
- *(deps)* Update rust crate bytesize to v2.0.1 (#59)
- *(deps)* Update rust crate serde_json to v1.0.140 (#60)
- *(deps)* Update rust crate serde to v1.0.219 (#61)
- *(deps)* Update rust crate clap to v4.5.32 (#62)
- *(deps)* Update rust crate humantime to v2.2.0 (#63)
- *(deps)* Update rust crate ssh2-config to 0.4.0 (#64)
- *(deps)* Update rust crate whoami to v1.6.0 (#66)
- *(deps)* Update rust crate log to v0.4.27 (#67)
- *(deps)* Update rust crate clap to v4.5.34 (#68)
- *(deps)* Update rust crate clap to v4.5.35 (#70)
- *(deps)* Update rust crate clap to v4.5.36 (#71)
- :bug: Try to fix openssl issue by enforcing building openssl from source
- *(deps)* Update rust crate shellexpand to v3.1.1 (#72)
- :bug: Allow cross-compilation by setting PKG_CONFIG_ALLOW_CROSS environment variable
- *(deps)* Update rust crate rand to v0.9.1 (#73)
- *(deps)* Update rust crate clap to v4.5.37 (#74)
- *(deps)* Update rust crate clap to v4.5.38 (#79)
- *(deps)* Update rust crate clap to v4.5.39 (#81)
- *(deps)* Update rust crate clap to v4.5.40 (#84)
- *(deps)* Update rust crate indicatif to v0.17.12 (#87)
- *(deps)* Update rust crate indicatif to 0.18.0 (#88)
- *(deps)* Update rust crate tabled to 0.20.0 (#75)
- *(deps)* Update rust crate ssh2-config to 0.5.0 (#69)

### ⚙️ Miscellaneous Tasks

- Allow running all tests for full diagnosis
- :construction_worker: Remove limitation on the branch name
- :construction_worker: Ensure all branches trigger CI workflow
- [**breaking**] Fix cross compilation
- *(app)* Maintainence release v0.2.1
- Fix wrong template prefix

### ◀️ Revert

- "fix: :bug: Try to fix openssl issue by enforcing building openssl from source"
- "fix: :bug: Allow cross-compilation by setting PKG_CONFIG_ALLOW_CROSS environment variable"

## [0.2.0] - 2025-02-08

### 🚀 Features

- Add json output format option
- Use regex for parsing target tuple

### 🐛 Bug Fixes

- *(deps)* Update rust crate tabled to 0.17.0 (#24)
- *(deps)* Update rust crate clap to v4.5.22 (#25)
- *(deps)* Update rust crate clap to v4.5.23 (#26)
- *(deps)* Update rust crate clap_complete to v4.5.39 (#27)
- *(deps)* Update rust crate clap_complete to v4.5.40 (#28)
- *(deps)* Update rust crate ssh2-config to 0.3.0 (#29)
- *(deps)* Update rust crate clap to v4.5.24 (#30)
- *(deps)* Update rust crate clap_complete to v4.5.41 (#31)
- *(deps)* Update rust crate clap to v4.5.26 (#32)
- *(deps)* Update rust crate clap_complete to v4.5.42 (#33)
- *(deps)* Update rust crate log to v0.4.24 (#34)
- *(deps)* Update rust crate log to v0.4.25 (#36)
- *(deps)* Update rust crate clap to v4.5.27 (#37)
- *(deps)* Update rust crate clap_complete to v4.5.43 (#38)
- *(deps)* Update rust crate indicatif to v0.17.10 (#40)
- *(deps)* Update rust crate indicatif to v0.17.11 (#41)
- *(deps)* Update rust crate clap_complete to v4.5.44 (#42)
- *(deps)* Update rust crate ssh2 to v0.9.5 (#43)
- *(deps)* Update rust crate size to 0.5.0 (#44)
- *(deps)* Update rust crate clap to v4.5.28 (#45)
- *(deps)* Update tabled
- *(deps)* Update rand
- *(deps)* Add serde and serde_json as dependency
- Update rustfmt configuration to use style_edition
- [**breaking**] Generate shell completions at compilation

### 🚜 Refactor

- Improve code readability and consistency

### 📚 Documentation

- Update README to include JSON output format and shell completions
- Update README to clarify shell autocompletion usage and options

### ⚙️ Miscellaneous Tasks

- Add completion files to release
- *(app)* Bump version to 0.2.0

## [0.1.5] - 2024-11-20

### 🐛 Bug Fixes

- *(deps)* Update rust crate clap to v4.5.17
- *(deps)* Update rust crate whoami to v1.5.2
- *(deps)* Update rust crate clap to v4.5.18
- *(deps)* Update rust crate clap to v4.5.19
- *(deps)* Update rust crate clap to v4.5.20
- *(deps)* Update rust crate indicatif to v0.17.9 (#20)
- *(deps)* Update rust crate clap to v4.5.21 (#21)

### ⚙️ Miscellaneous Tasks

- Maintainence release v0.1.5

## [0.1.4] - 2024-09-02

### 🐛 Bug Fixes

- *(deps)* Update rust crate log to v0.4.22 (#3)
- *(deps)* Update rust crate clap to v4.5.8
- *(deps)* Update rust crate clap to v4.5.9
- *(deps)* Update rust crate clap to v4.5.10
- *(deps)* Update rust crate clap to v4.5.11
- *(deps)* Update rust crate clap to v4.5.12
- *(deps)* Update rust crate clap to v4.5.13
- *(deps)* Update rust crate tabled to 0.16.0
- *(deps)* Update rust crate clap to v4.5.14
- *(deps)* Update rust crate clap to v4.5.15
- *(deps)* Update rust crate clap to v4.5.16

### 📚 Documentation

- Update Homebrew installation instructions for sshping

### ⚙️ Miscellaneous Tasks

- Add sha256 checksums for release artifacts
- Update CI workflow to include main and renovate branches for testing
- Add indicatif to dependencies
- Add progress bar for all tests
- Bump sshping version to 0.1.4
- Update release workflow to generate checksums with actions
- Update release workflow to generate checksums with python

### ◀️ Revert

- Use action to generate checksum

## [0.1.3] - 2024-06-14

### 🚀 Features

- Add TableStyle option for different output styles

### 🐛 Bug Fixes

- Fix bug in Formatter human readable logic

### 🚜 Refactor

- Remove unused char_count in EchoTestSummary
- Remove unnecessary public function visibility
- Simplify Formatter initialization logic
- Use more ergonomic ExitCode in main.rs

### 📚 Documentation

- Update SSH ping results and FAQ
- Add some more description to examples
- Update usage and example for table style
- Update sshping usage with wrapped help
- Add better error handling to future goals

### 🎨 Styling

- Sort and group imports; Wrap long comments

### ⚙️ Miscellaneous Tasks

- Update issue templates
- Configure Renovate (#1)
- Update Renovate configuration to include automergeAll
- Add GitHub Actions workflow for testing building binaries
- Add rustfmt.toml configuration file
- Auto generate changelog for release
- Bump version to 0.1.3
- Update dependencies for wrap_help feature in clap
- Ignore tags in push events; add permission for writing changelog

## [0.1.2] - 2024-06-13

### ⚙️ Miscellaneous Tasks

- Update README with pre-built binaries and installation instructions
- Bump version to 0.1.2

## [0.1.1] - 2024-06-13

### 🚀 Features

- Initialize project files and dependencies
- Update delimit option in CLI to allow specifying delimiters for big numbers
- Time ssh authentication
- Add debug logging for options in main.rs
- Implement echo test
- Add Formatter struct for time and size formatting
- Accepting more size description
- Update main.rs to use Formatter for time and size formatting
- Add chunk size to CLI options for speed test
- Implement speed test
- Add color to CLI styles
- Add value hints to CLI options for better user experience
- Add table formatting and output in main.rs
- Improve error handling in echo and speed tests

### 🐛 Bug Fixes

- Remove redundant unit and fix prompt description
- Improve table border
- Replace users with whoami in hope for support for Windows

### 🚜 Refactor

- Refactor run_echo_test function to improve latency calculation and logging
- Remove unnecessary extern crate
- Update main.rs to include chunk size in speed test options
- Remove unused imports and clean up main.rs
- Remove unused bind_address parameter in main.rs
- Remove ping_summary option from CLI options
- Add summary module for test result summaries
- Refactor summary structs to store formatted strings

### 📚 Documentation

- Add SSH ping functionality and usage documentation
- Remove bind_address parameter
- Update CLI options with additional descriptions and examples
- Update README for usage and examples
- Add installation instructions to README.md

### ⚙️ Miscellaneous Tasks

- Add MIT License
- Update logging level for available authentication methods
- Update ssh2 dependency to use vendored-openssl feature
- Update dependencies
- Update authentication method error message to use static string; Update log level
- Sort import
- Update logger configuration to remove timestamps
- Add rand to dependencies
- Format code
- Add tabled crate
- Update license information in Cargo.toml
- Add feature to opt-in vendored-openssl
- Update version to 0.1.1 in Cargo.toml and Cargo.lock
- Add default feature in Cargo.toml
- Add GitHub Actions workflow for building release binaries
- Fix feature name
- Revert changes to enable include-openssl in pre-built binaries

<!-- generated by git-cliff -->
