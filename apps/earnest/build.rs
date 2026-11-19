//! Gives the binary its engine version: the package version and a fingerprint of the checker's
//! sources, so a cached result is never served by a build whose code differs.

#[path = "src/sources.rs"]
mod sources;

use std::env;
use std::io::{self, Write};
use std::path::Path;

fn main() -> io::Result<()> {
    let manifest = env::var("CARGO_MANIFEST_DIR").map_err(io::Error::other)?;
    let version = env::var("CARGO_PKG_VERSION").map_err(io::Error::other)?;
    let repository = Path::new(&manifest).join("../..").canonicalize()?;
    let engine = sources::engine(&version, &repository)?;
    let mut cargo = io::stdout().lock();
    for root in sources::roots(&repository)? {
        writeln!(cargo, "cargo::rerun-if-changed={}", root.display())?;
    }
    writeln!(cargo, "cargo::rerun-if-changed=build.rs")?;
    writeln!(cargo, "cargo::rustc-env=EARNEST_ENGINE={engine}")
}
