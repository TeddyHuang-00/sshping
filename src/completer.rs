use std::{ffi::OsStr, fs::read_to_string, path::PathBuf};

use clap_complete::engine::CompletionCandidate;
use shellexpand::tilde;

pub fn complete_host(current: &OsStr) -> Vec<CompletionCandidate> {
    let mut candidates = Vec::new();
    let Some(current) = current.to_str() else {
        return candidates;
    };

    let Some((prefix, host_prefix)) = split_target_prefix(current) else {
        return candidates;
    };

    for host in read_ssh_hosts() {
        if host.starts_with(host_prefix) {
            candidates.push(CompletionCandidate::new(format!("{prefix}{host}")));
        }
    }

    candidates.sort();
    candidates.dedup_by(|a, b| a.get_value() == b.get_value());
    candidates
}

fn split_target_prefix(current: &str) -> Option<(&str, &str)> {
    if current.starts_with('[') {
        return None;
    }
    if let Some((user, host)) = current.rsplit_once('@') {
        if user.is_empty() {
            return None;
        }
        if host.contains(':') {
            return None;
        }
        return Some((&current[..=user.len()], host));
    }
    if current.contains(':') {
        return None;
    }
    Some(("", current))
}

fn read_ssh_hosts() -> Vec<String> {
    let config = default_ssh_config();
    let Ok(content) = read_to_string(config) else {
        return Vec::new();
    };

    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| parse_host_line(line))
        .flatten()
        .filter(|name| !name.is_empty())
        .collect()
}

fn parse_host_line(line: &str) -> Option<Vec<String>> {
    let mut parts = line.split_ascii_whitespace();
    let key = parts.next()?;
    if !key.eq_ignore_ascii_case("host") {
        return None;
    }
    Some(
        parts
            .filter(|pattern| !pattern.contains('*') && !pattern.contains('?') && !pattern.starts_with('!'))
            .map(ToString::to_string)
            .collect(),
    )
}

fn default_ssh_config() -> PathBuf {
    PathBuf::from(tilde("~/.ssh/config").to_string())
}

