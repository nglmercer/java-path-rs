//! Pick the best Java 21 JDK for this machine.

use java_path::SelectExt;

fn main() -> Result<(), java_path::Error> {
    let installs = java_path::discover()?;
    let java = installs.select().major(21).jdk().current_arch().best()?;
    println!("{} -> {}", java.version, java.home.display());
    Ok(())
}
