use std::{env, io::Error};

use clap::CommandFactory;
use clap_complete::{generate_to, Shell};

mod completer {
    include!("src/completer.rs");
}
include!("src/cli.rs");

fn main() -> Result<(), Error> {
    println!("cargo::rerun-if-changed=src/cli.rs");
    println!("cargo::rerun-if-changed=src/completer.rs");
    let outdir = match env::var_os("OUT_DIR") {
        None => return Ok(()),
        Some(outdir) => outdir,
    };

    let mut cmd = Options::command();
    // New directory for completions in target/<profile>/completions
    let completion_path = PathBuf::from(outdir)
        .ancestors()
        .nth(3)
        .unwrap()
        .join("completions")
        .into_os_string();
    // Make sure the completion directory exists
    std::fs::create_dir_all(&completion_path)?;
    for &shell in Shell::value_variants() {
        generate_to(shell, &mut cmd, "sshping", &completion_path)?;
    }

    Ok(())
}
