use std::{env, io::Error};

use clap::CommandFactory;
use clap_complete::{generate_to, Shell};
use clap_mangen::Man;

mod completer {
    include!("src/completer.rs");
}
include!("src/cli.rs");

fn main() -> Result<(), Error> {
    println!("cargo::rerun-if-changed=src/cli.rs");
    println!("cargo::rerun-if-changed=src/completer.rs");
    let outdir = PathBuf::from(std::env::var_os("OUT_DIR").ok_or(std::io::ErrorKind::NotFound)?);
    let profile_root = outdir
        .ancestors()
        .nth(3)
        .ok_or(std::io::ErrorKind::NotFound)?;

    let mut cmd = Options::command();

    // New directory for completions in target/<profile>/completions
    let completion_path = profile_root.join("completions");
    std::fs::create_dir_all(&completion_path)?;

    for &shell in Shell::value_variants() {
        generate_to(shell, &mut cmd, "sshping", &completion_path)?;
    }

    // Manpage generation in target/<profile>/man
    let manpage_path = profile_root.join("man");
    std::fs::create_dir_all(&manpage_path)?;

    let man = Man::new(cmd);
    let mut buffer = vec![];
    man.render(&mut buffer)?;
    std::fs::write(manpage_path.join("sshping.1"), buffer)?;

    Ok(())
}
