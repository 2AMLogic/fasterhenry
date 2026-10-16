//! Output-destination validation: the checks the binary makes before it
//! reads, solves or writes anything (issue #175).
//!
//! `--json`, `--zc-mat` (or the bare form's implicit `./Zc.mat`) and
//! `--spice` each name a separate artifact. None of them may be the input
//! the sweep reads — that would overwrite the source geometry — and no two
//! of them may be the same file — the later format would silently replace
//! the earlier one while the run still succeeded.
//!
//! "The same file" is decided on resolved paths, not on the strings given:
//!
//! - a relative path is taken against the working directory, and `.`
//!   components are dropped;
//! - an existing path is canonicalized, which resolves every symlink on it
//!   (and `..` through them, as the filesystem does);
//! - a dangling symlink is followed to the file a write would create;
//! - a path that does not exist yet is its nearest existing ancestor,
//!   canonicalized, with the remaining components appended;
//! - on unix, two existing paths with the same device and inode — hard
//!   links, or two spellings a case-insensitive filesystem folds together —
//!   are the same file whatever their names.
//!
//! This catches accidental collisions; it is not a guard against a path
//! being swapped between the check and the write. An existing output file
//! that is none of the above is still replaced, as before.

use std::path::{Component, Path, PathBuf};

/// One path the run touches, with the role it plays there (`input`,
/// `--json`, …), for the diagnostic.
#[derive(Clone, Copy, Debug)]
pub struct Destination<'a> {
    /// What the path is for, as the user would recognize it: `input`,
    /// `--json`, `--zc-mat`, `--spice`, or the bare form's default
    /// `Zc.mat`.
    pub role: &'a str,
    /// The path as given on the command line.
    pub path: &'a Path,
}

/// The identity of a path for collision purposes: where it resolves to, and
/// (on unix, when the file exists) its device and inode.
#[derive(Debug, PartialEq, Eq)]
struct Identity {
    resolved: PathBuf,
    file_id: Option<(u64, u64)>,
}

impl Identity {
    fn of(path: &Path) -> Self {
        let resolved = resolve(path);
        let file_id = file_id(&resolved);
        Self { resolved, file_id }
    }

    fn same_file_as(&self, other: &Self) -> bool {
        self.resolved == other.resolved
            || matches!((self.file_id, other.file_id), (Some(a), Some(b)) if a == b)
    }
}

/// Checks that no output in `outputs` is the same file as `input`, and that
/// no two outputs are the same file. Run before anything is read or
/// written; on failure, the message names both conflicting roles and the
/// paths as given.
pub fn validate_destinations(input: &Path, outputs: &[Destination<'_>]) -> Result<(), String> {
    let input_identity = Identity::of(input);
    let identities: Vec<Identity> = outputs
        .iter()
        .map(|output| Identity::of(output.path))
        .collect();
    for (output, identity) in outputs.iter().zip(&identities) {
        if identity.same_file_as(&input_identity) {
            return Err(format!(
                "{} output {} is the input {}: refusing to overwrite the input; \
                 choose another output path",
                output.role,
                output.path.display(),
                input.display()
            ));
        }
    }
    for (i, (first, first_identity)) in outputs.iter().zip(&identities).enumerate() {
        for (second, second_identity) in outputs.iter().zip(&identities).skip(i + 1) {
            if first_identity.same_file_as(second_identity) {
                return Err(format!(
                    "{} output {} and {} output {} are the same file: one would replace \
                     the other; give each output its own path",
                    first.role,
                    first.path.display(),
                    second.role,
                    second.path.display()
                ));
            }
        }
    }
    Ok(())
}

/// How many symlinks [`resolve`] follows through dangling links before it
/// gives up and compares the path as it stands.
const MAX_LINK_HOPS: usize = 40;

/// The absolute, symlink-resolved form of `path`, whether or not it exists.
fn resolve(path: &Path) -> PathBuf {
    resolve_hops(path, MAX_LINK_HOPS)
}

fn resolve_hops(path: &Path, hops: usize) -> PathBuf {
    let absolute = absolute(path);
    if let Ok(canonical) = std::fs::canonicalize(&absolute) {
        return canonical;
    }
    // A dangling symlink: a write follows it and creates its target.
    if hops > 0 {
        if let Ok(target) = std::fs::read_link(&absolute) {
            let target = match absolute.parent() {
                Some(parent) if target.is_relative() => parent.join(target),
                _ => target,
            };
            return resolve_hops(&target, hops - 1);
        }
    }
    // Not there (yet): resolve the existing part, append the rest.
    match (absolute.parent(), absolute.file_name()) {
        (Some(parent), Some(name)) => resolve_hops(parent, hops).join(name),
        _ => absolute,
    }
}

/// `path` against the working directory, with `.` components dropped and
/// `..` kept (canonicalization resolves those against the real tree).
fn absolute(path: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        match std::env::current_dir() {
            Ok(directory) => directory.join(path),
            Err(_) => path.to_path_buf(),
        }
    };
    joined
        .components()
        .filter(|component| !matches!(component, Component::CurDir))
        .collect()
}

#[cfg(unix)]
fn file_id(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path)
        .ok()
        .map(|metadata| (metadata.dev(), metadata.ino()))
}

#[cfg(not(unix))]
fn file_id(_path: &Path) -> Option<(u64, u64)> {
    None
}

#[cfg(test)]
mod tests {
    use super::{validate_destinations, Destination};
    use std::path::{Path, PathBuf};

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "fasterhenry-cli-outputs-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        // Canonical, so a symlinked temp directory (macOS) is not a factor.
        std::fs::canonicalize(&directory).unwrap()
    }

    fn output<'a>(role: &'a str, path: &'a Path) -> Destination<'a> {
        Destination { role, path }
    }

    #[test]
    fn distinct_paths_pass_and_equal_ones_fail() {
        let directory = scratch("distinct");
        let input = directory.join("deck.inp");
        std::fs::write(&input, "deck").unwrap();
        let json = directory.join("out.json");
        let mat = directory.join("out.mat");
        let existing = directory.join("old.cir");
        std::fs::write(&existing, "old").unwrap();
        validate_destinations(
            &input,
            &[
                output("--json", &json),
                output("--zc-mat", &mat),
                output("--spice", &existing),
            ],
        )
        .unwrap();

        let error = validate_destinations(&input, &[output("--json", &input)]).unwrap_err();
        assert!(
            error.contains("--json") && error.contains("input"),
            "{error}"
        );

        let error =
            validate_destinations(&input, &[output("--json", &mat), output("--spice", &mat)])
                .unwrap_err();
        assert!(
            error.contains("--json") && error.contains("--spice"),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn new_paths_through_dot_and_dotdot_components_collide() {
        let directory = scratch("dots");
        std::fs::create_dir(directory.join("sub")).unwrap();
        let input = directory.join("deck.inp");
        std::fs::write(&input, "deck").unwrap();
        let plain = directory.join("new.mat");
        let dotted = directory.join(".").join("sub").join("..").join("new.mat");
        let error = validate_destinations(
            &input,
            &[output("--zc-mat", &plain), output("--json", &dotted)],
        )
        .unwrap_err();
        assert!(error.contains("same file"), "{error}");
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlink_is_its_target() {
        let directory = scratch("dangling");
        let input = directory.join("deck.inp");
        std::fs::write(&input, "deck").unwrap();
        let link = directory.join("link.json");
        std::os::unix::fs::symlink("target.json", &link).unwrap();
        let target = directory.join("target.json");
        let error = validate_destinations(
            &input,
            &[output("--json", &link), output("--spice", &target)],
        )
        .unwrap_err();
        assert!(error.contains("same file"), "{error}");
        let _ = std::fs::remove_dir_all(&directory);
    }
}
