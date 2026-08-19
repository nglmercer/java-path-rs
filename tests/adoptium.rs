#![cfg(feature = "network")]

use java_path::{Architecture, JdkProvider, Platform, ReleaseRequest};

#[test]
fn maps_platforms_and_architectures_to_api_tokens() {
    assert_eq!(Platform::Linux.adoptium_os(), Some("linux"));
    assert_eq!(Platform::MacOs.adoptium_os(), Some("mac"));
    assert_eq!(Platform::Windows.adoptium_os(), Some("windows"));
    // Termux uses the Termux package manager, but the API token is still linux.
    assert_eq!(Platform::Termux.adoptium_os(), Some("linux"));
    assert_eq!(Platform::Unknown.adoptium_os(), None);

    assert_eq!(Architecture::X86_64.adoptium_arch(), Some("x64"));
    assert_eq!(Architecture::Aarch64.adoptium_arch(), Some("aarch64"));
    assert_eq!(Architecture::Unknown.adoptium_arch(), None);
}

#[test]
fn knows_which_versions_are_lts() {
    use java_path::AdoptiumProvider;
    assert!(AdoptiumProvider::is_lts(21));
    assert!(AdoptiumProvider::is_lts(17));
    assert!(!AdoptiumProvider::is_lts(22));
}

#[tokio::test]
async fn unsupported_platform_is_rejected_before_any_request() {
    use java_path::AdoptiumProvider;
    let provider = AdoptiumProvider::new().with_base_url("http://127.0.0.1:1");
    let request = ReleaseRequest {
        platform: Platform::Unknown,
        ..ReleaseRequest::default()
    };
    let err = provider.releases(request).await.unwrap_err();
    assert!(err.to_string().contains("unsupported platform"));
}

#[tokio::test]
async fn network_failures_surface_as_network_errors() {
    use java_path::AdoptiumProvider;
    let provider = AdoptiumProvider::new().with_base_url("http://127.0.0.1:1");
    let request = ReleaseRequest::default().major(21);
    let err = provider.releases(request).await.unwrap_err();
    assert!(matches!(err, java_path::Error::Network(_)), "{err}");
}

/// Hits the public Adoptium API; run with `cargo test -- --ignored`.
#[tokio::test]
#[ignore = "requires network access"]
async fn resolves_a_real_temurin_release() {
    use java_path::AdoptiumProvider;
    let provider = AdoptiumProvider::new();
    let release = provider
        .resolve(ReleaseRequest::default().major(21))
        .await
        .unwrap();
    assert_eq!(release.major, 21);
    assert!(release.sha256.is_some());
    assert!(release.url.starts_with("https://"));
}
