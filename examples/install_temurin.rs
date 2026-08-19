//! Download and install Temurin 21 into the default install directory.

use java_path::{InstallEvent, JavaInstaller};

#[tokio::main]
async fn main() -> Result<(), java_path::Error> {
    let java = JavaInstaller::adoptium()
        .version(21)
        .on_event(|event| match event {
            InstallEvent::Downloading { downloaded, total } => {
                if let Some(total) = total {
                    eprint!("\rdownloading {downloaded}/{total} bytes");
                }
            }
            other => eprintln!("\n{other:?}"),
        })
        .install()
        .await?;

    println!("installed {} at {}", java.version, java.home.display());
    Ok(())
}
