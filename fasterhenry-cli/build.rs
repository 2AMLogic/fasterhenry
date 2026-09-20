// Bakes the source revision into `--version` via FASTERHENRY_GIT_DESCRIBE.
// Falls back to "unknown" where git is absent (a crates.io tarball build),
// so the version string stays deterministic there.
use std::process::Command;

fn describe() -> String {
    for git in ["git", "/usr/bin/git"] {
        if let Ok(output) = Command::new(git)
            .args(["describe", "--tags", "--always", "--dirty=-dirty"])
            .output()
        {
            if output.status.success() {
                if let Ok(text) = String::from_utf8(output.stdout) {
                    let text = text.trim();
                    if !text.is_empty() {
                        return text.to_string();
                    }
                }
            }
        }
    }
    "unknown".to_string()
}

fn main() {
    println!("cargo:rustc-check-cfg=cfg(FASTERHENRY_GIT_DESCRIBE)");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-env=FASTERHENRY_GIT_DESCRIBE={}", describe());
}
