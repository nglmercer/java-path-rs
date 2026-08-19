//! List every Java installation found on this machine.

fn main() -> Result<(), java_path::Error> {
    for install in java_path::discover()? {
        println!(
            "{:<8} {:<7} {:<10} {:<22} {}",
            install.version.major,
            format!("{:?}", install.kind),
            install.architecture,
            install.vendor.as_deref().unwrap_or("-"),
            install.home.display()
        );
    }
    Ok(())
}
