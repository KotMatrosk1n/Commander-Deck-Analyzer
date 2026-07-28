use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

const ANALYSIS_IMPLEMENTATION_ENV: &str = "CDA_ANALYSIS_IMPLEMENTATION_SHA256";

fn main() {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo must provide CARGO_MANIFEST_DIR"),
    );
    let source_dir = manifest_dir.join("src");
    let data_dir = manifest_dir.join("data");
    let mut inputs = vec![
        manifest_dir.join("Cargo.toml"),
        manifest_dir.join("Cargo.lock"),
        manifest_dir.join("build.rs"),
    ];
    collect_files(&source_dir, &mut inputs);
    collect_files(&data_dir, &mut inputs);
    inputs.sort();

    let mut hasher = Sha256::new();
    for path in &inputs {
        let relative = path
            .strip_prefix(&manifest_dir)
            .expect("analysis implementation input must remain inside the crate");
        let portable_path = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        hasher.update(portable_path.as_bytes());
        hasher.update([0]);
        hasher.update(
            fs::read(path).unwrap_or_else(|error| {
                panic!("could not fingerprint {}: {error}", path.display())
            }),
        );
        hasher.update([0xff]);
    }

    const BUILD_ENVIRONMENT_KEYS: [&str; 7] = [
        "TARGET",
        "PROFILE",
        "OPT_LEVEL",
        "DEBUG",
        "CARGO_CFG_TARGET_FEATURE",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTFLAGS",
    ];
    for name in BUILD_ENVIRONMENT_KEYS {
        let value = std::env::var(name).unwrap_or_default();
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(value.as_bytes());
        hasher.update([0xff]);
    }

    let mut enabled_features = std::env::vars()
        .filter_map(|(name, value)| {
            (name.starts_with("CARGO_FEATURE_") && value == "1").then_some(name)
        })
        .collect::<Vec<_>>();
    enabled_features.sort();
    for feature in enabled_features {
        hasher.update(feature.as_bytes());
        hasher.update([0xff]);
    }

    let rustc = std::env::var_os("RUSTC").expect("Cargo must provide RUSTC");
    let rustc_version = Command::new(&rustc)
        .args(["--version", "--verbose"])
        .output()
        .expect("rustc version must be available while building");
    if !rustc_version.status.success() {
        panic!("rustc --version --verbose failed while fingerprinting the analysis build");
    }
    hasher.update(&rustc_version.stdout);

    println!(
        "cargo:rustc-env={ANALYSIS_IMPLEMENTATION_ENV}={:x}",
        hasher.finalize()
    );
    println!("cargo:rerun-if-changed={}", source_dir.display());
    println!("cargo:rerun-if-changed={}", data_dir.display());
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=RUSTC");
    for name in BUILD_ENVIRONMENT_KEYS {
        println!("cargo:rerun-if-env-changed={name}");
    }

    tauri_build::build()
}

fn collect_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", directory.display()))
        .map(|entry| entry.expect("analysis implementation directory entry"))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .unwrap_or_else(|error| panic!("could not inspect {}: {error}", path.display()));
        if file_type.is_dir() {
            collect_files(&path, files);
        } else if file_type.is_file() {
            files.push(path);
        }
    }
}
