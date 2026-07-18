use std::env;

fn env_value(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_version(version: String) -> String {
    version.strip_prefix('v').unwrap_or(&version).to_string()
}

fn github_tag_version() -> Option<String> {
    if env_value("GITHUB_REF_TYPE").as_deref() == Some("tag") {
        return env_value("GITHUB_REF_NAME").map(normalize_version);
    }

    env_value("GITHUB_REF")
        .and_then(|value| value.strip_prefix("refs/tags/").map(str::to_owned))
        .map(normalize_version)
}

fn main() {
    println!("cargo:rerun-if-env-changed=OBSCURA_VERSION");
    println!("cargo:rerun-if-env-changed=GITHUB_REF_TYPE");
    println!("cargo:rerun-if-env-changed=GITHUB_REF_NAME");
    println!("cargo:rerun-if-env-changed=GITHUB_REF");

    let version = env_value("OBSCURA_VERSION")
        .map(normalize_version)
        .or_else(github_tag_version)
        .unwrap_or_else(|| env::var("CARGO_PKG_VERSION").expect("Cargo sets CARGO_PKG_VERSION"));

    println!("cargo:rustc-env=OBSCURA_BUILD_VERSION={version}");

    // On Windows, embed the app icon (assets/logo.svg rasterized to
    // assets/icon.ico) into the `obscura` executable's resources so Explorer
    // and the taskbar show it. No-op on other targets.
    #[cfg(target_os = "windows")]
    {
        let icon = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/icon.ico");
        if std::path::Path::new(icon).exists() {
            println!("cargo:rerun-if-changed={icon}");
            let mut res = winresource::WindowsResource::new();
            res.set_icon(icon);
            if let Err(e) = res.compile() {
                println!("cargo:warning=failed to embed icon: {e}");
            }
        }
    }
}
