// Minimal SSH config parser for sshping
// Parses SSH configuration files to extract host-specific settings

use std::io::Read;
use std::path::PathBuf;

#[derive(Debug, Default, Clone)]
pub struct HostParams {
    pub host_name: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<Vec<PathBuf>>,
}

pub struct SshConfig {
    contents: String,
}

impl SshConfig {
    pub fn default() -> Self {
        Self {
            contents: String::new(),
        }
    }

    pub fn parse<R: Read>(mut self, reader: &mut R) -> Result<Self, std::io::Error> {
        reader.read_to_string(&mut self.contents)?;
        Ok(self)
    }

    pub fn query(&self, host: &str) -> HostParams {
        parse_config_for_host(&self.contents, host)
    }
}

// Parse SSH config file and extract host-specific parameters
fn parse_config_for_host(contents: &str, target_host: &str) -> HostParams {
    let mut params = HostParams::default();
    let mut in_matching_host = false;

    for line in contents.lines() {
        let trimmed = line.trim();
        
        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Check for Host directive
        if let Some(rest) = trimmed.strip_prefix("Host ").or_else(|| trimmed.strip_prefix("host ")) {
            // Check if any pattern matches our target host
            let patterns: Vec<&str> = rest.split_whitespace().collect();
            in_matching_host = patterns.iter().any(|pattern| {
                matches_pattern(target_host, pattern)
            });
            continue;
        }

        // Parse configuration options for matching host
        if in_matching_host {
            if let Some((key, value)) = split_key_value(trimmed) {
                let key_lower = key.to_lowercase();
                let value = value.trim().trim_matches('"').trim_matches('\'');
                
                match key_lower.as_str() {
                    "hostname" => {
                        if params.host_name.is_none() {
                            params.host_name = Some(value.to_string());
                        }
                    }
                    "user" => {
                        if params.user.is_none() {
                            params.user = Some(value.to_string());
                        }
                    }
                    "port" => {
                        if params.port.is_none() {
                            if let Ok(port) = value.parse::<u16>() {
                                params.port = Some(port);
                            }
                        }
                    }
                    "identityfile" => {
                        let path = expand_tilde(value);
                        if let Some(ref mut files) = params.identity_file {
                            files.push(path);
                        } else {
                            params.identity_file = Some(vec![path]);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    params
}

// Split a line into key and value
fn split_key_value(line: &str) -> Option<(&str, &str)> {
    line.find(|c: char| c.is_whitespace())
        .map(|pos| {
            let (key, value) = line.split_at(pos);
            (key.trim(), value.trim())
        })
}

// Check if a hostname matches an SSH config pattern
fn matches_pattern(hostname: &str, pattern: &str) -> bool {
    // Handle negation
    if let Some(negated_pattern) = pattern.strip_prefix('!') {
        return !matches_pattern(hostname, negated_pattern);
    }

    // Exact match
    if pattern == hostname {
        return true;
    }

    // Wildcard match
    if pattern.contains('*') || pattern.contains('?') {
        return glob_match(hostname, pattern);
    }

    false
}

// Simple glob-style pattern matching
fn glob_match(text: &str, pattern: &str) -> bool {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();
    
    match_helper(&text_chars, &pattern_chars, 0, 0)
}

fn match_helper(text: &[char], pattern: &[char], text_idx: usize, pattern_idx: usize) -> bool {
    // Both exhausted - match
    if text_idx == text.len() && pattern_idx == pattern.len() {
        return true;
    }
    
    // Pattern exhausted but text remains - no match
    if pattern_idx == pattern.len() {
        return false;
    }
    
    // Handle wildcards
    if pattern[pattern_idx] == '*' {
        // Try matching zero or more characters
        // First, try matching zero characters (skip the *)
        if match_helper(text, pattern, text_idx, pattern_idx + 1) {
            return true;
        }
        // Then try matching one or more characters
        if text_idx < text.len() && match_helper(text, pattern, text_idx + 1, pattern_idx) {
            return true;
        }
        return false;
    }
    
    if pattern[pattern_idx] == '?' {
        // Match exactly one character
        if text_idx < text.len() {
            return match_helper(text, pattern, text_idx + 1, pattern_idx + 1);
        }
        return false;
    }
    
    // Regular character match
    if text_idx < text.len() && text[text_idx] == pattern[pattern_idx] {
        return match_helper(text, pattern, text_idx + 1, pattern_idx + 1);
    }
    
    false
}

// Expand ~ to home directory
fn expand_tilde(path: &str) -> PathBuf {
    if path.strip_prefix("~/").is_some() {
        let expanded = shellexpand::tilde(path).into_owned().parse().unwrap_or_else(|_| PathBuf::from(path));
        return expanded;
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_pattern() {
        assert!(matches_pattern("example.com", "example.com"));
        assert!(matches_pattern("example.com", "*"));
        assert!(matches_pattern("example.com", "*.com"));
        assert!(matches_pattern("test.example.com", "*.example.com"));
        assert!(matches_pattern("example.com", "example.*"));
        assert!(!matches_pattern("example.com", "other.com"));
        assert!(!matches_pattern("example.com", "*.org"));
        
        // Test negation
        assert!(!matches_pattern("example.com", "!example.com"));
        assert!(matches_pattern("other.com", "!example.com"));
    }

    #[test]
    fn test_parse_simple_config() {
        let config_text = r#"
Host testhost
    HostName example.com
    User myuser
    Port 2222
    IdentityFile ~/.ssh/id_rsa
"#;
        let params = parse_config_for_host(config_text, "testhost");
        assert_eq!(params.host_name, Some("example.com".to_string()));
        assert_eq!(params.user, Some("myuser".to_string()));
        assert_eq!(params.port, Some(2222));
        assert!(params.identity_file.is_some());
    }

    #[test]
    fn test_parse_with_wildcard() {
        let config_text = r#"
Host *.example.com
    User wildcard_user
    Port 3333
"#;
        let params = parse_config_for_host(config_text, "test.example.com");
        assert_eq!(params.user, Some("wildcard_user".to_string()));
        assert_eq!(params.port, Some(3333));
    }

    #[test]
    fn test_parse_case_insensitive() {
        let config_text = r#"
host testhost
    hostname example.com
    user myuser
"#;
        let params = parse_config_for_host(config_text, "testhost");
        assert_eq!(params.host_name, Some("example.com".to_string()));
        assert_eq!(params.user, Some("myuser".to_string()));
    }

    #[test]
    fn test_multiple_identity_files() {
        let config_text = r#"
Host testhost
    IdentityFile ~/.ssh/id_rsa
    IdentityFile ~/.ssh/id_ed25519
"#;
        let params = parse_config_for_host(config_text, "testhost");
        assert_eq!(params.identity_file.as_ref().map(|v| v.len()), Some(2));
    }
}
