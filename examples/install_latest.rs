//! Resolve and download the *latest* Java, with no version hardcoded.
//!
//! ```bash
//! cargo run --example install_latest --features install            # latest LTS
//! cargo run --example install_latest --features install -- latest  # newest overall
//! cargo run --example install_latest --features install -- dry-run # resolve only
//! ```

use java_path::{AdoptiumProvider, InstallEvent, JavaInstaller, JdkProvider, ReleaseRequest};

#[tokio::main]
async fn main() -> Result<(), java_path::Error> {
    let arg = std::env::args().nth(1).unwrap_or_default();
    let newest_overall = arg == "latest";
    let dry_run = arg == "dry-run";

    let provider = AdoptiumProvider::new();
    println!(
        "available feature versions: {:?}",
        provider.available_releases().await?
    );
    println!(
        "latest LTS:                 {}",
        provider.latest_lts().await?
    );
    println!(
        "latest feature release:     {}",
        provider.latest_feature().await?
    );

    // Resolve first so we can show what would be downloaded before doing it.
    let request = if newest_overall {
        ReleaseRequest::default().latest()
    } else {
        ReleaseRequest::default().latest_lts()
    };
    let release = provider.resolve(request).await?;

    println!(
        "\nresolved {} ({}) -> {}\n  size: {} MB\n  sha256: {}",
        release.release_name,
        if release.lts { "LTS" } else { "non-LTS" },
        release.file_name,
        release.size.unwrap_or(0) / 1_048_576,
        release.sha256.as_deref().unwrap_or("<none published>"),
    );

    if dry_run {
        println!("\ndry run: nothing downloaded");
        return Ok(());
    }

    let installer = JavaInstaller::adoptium().on_event(|event| match event {
        InstallEvent::Downloading { downloaded, total } => {
            if let Some(total) = total {
                eprint!("\r  downloading {:>3}%", downloaded * 100 / total.max(1));
            }
        }
        other => eprintln!("\n  {other:?}"),
    });
    let installer = if newest_overall {
        installer.latest()
    } else {
        installer.latest_lts()
    };

    let java = installer.install().await?;
    println!("\ninstalled {} at {}", java.version, java.home.display());
    Ok(())
}
