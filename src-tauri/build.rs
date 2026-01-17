use std::{env, fs, path::PathBuf};

fn main() {
    ensure_sidecar_placeholder();
    tauri_build::build()
}

fn ensure_sidecar_placeholder() {
    if env::var("PROFILE").ok().as_deref() == Some("release") {
        return;
    }

    let target = env::var("TAURI_ENV_TARGET_TRIPLE")
        .or_else(|_| env::var("TARGET"))
        .or_else(|_| env::var("HOST"))
        .unwrap_or_default();
    if target.is_empty() {
        return;
    }

    let mut filename = format!("native_bridge-{}", target);
    if target.contains("windows") {
        filename.push_str(".exe");
    }

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let path = PathBuf::from(manifest_dir).join(filename);
    if path.exists() {
        return;
    }

    if fs::write(&path, "").is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o755));
        }
        println!(
            "cargo:warning=Created placeholder sidecar for dev at {}",
            path.display()
        );
    }
}
