//! The two command-line entry points, driven as a process: the drop-in bare
//! form (`fasterhenry deck.inp`, which writes `./Zc.mat`) and the explicit
//! `fasterhenry run deck.inp` of 0.1 (which does not). Everything else about
//! the sweep is covered in-process by `roundtrip.rs`; these tests are about
//! argument parsing, the default output path, and what an unrecognized flag
//! does.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The binary under test, built by cargo for this integration test.
const BINARY: &str = env!("CARGO_BIN_EXE_fasterhenry");

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

/// An empty directory of this test's own, so a `Zc.mat` written relative to
/// the working directory is unambiguous.
fn scratch(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("fasterhenry-cli-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("scratch directory");
    directory
}

/// Runs the binary in `directory` with `args`.
fn fasterhenry(directory: &Path, args: &[&str]) -> Output {
    Command::new(BINARY)
        .args(args)
        .current_dir(directory)
        .output()
        .expect("the binary runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// `fasterhenry <deck>` — no subcommand, no `--zc-mat` — writes `Zc.mat` in
/// the working directory, byte for byte what `run --zc-mat` writes, and the
/// JSON result still goes to stdout.
#[test]
fn bare_invocation_writes_zc_mat_like_the_run_subcommand() {
    let directory = scratch("bare-writes-zc-mat");
    let deck = fixture("spiral.inp");
    let deck = deck.to_str().unwrap();

    let bare = fasterhenry(&directory, &[deck]);
    assert!(bare.status.success(), "{}", stderr(&bare));
    let written = directory.join("Zc.mat");
    assert!(
        written.is_file(),
        "a bare invocation must write ./Zc.mat without being asked"
    );

    // The same sweep through the 0.1 form, asked for explicitly.
    let asked = fasterhenry(&directory, &["run", deck, "--zc-mat", "asked.mat"]);
    assert!(asked.status.success(), "{}", stderr(&asked));
    assert_eq!(
        std::fs::read(&written).unwrap(),
        std::fs::read(directory.join("asked.mat")).unwrap(),
        "the bare default and --zc-mat must write the same file"
    );

    // The documented variables are in there: `freqs` and one `Zc_k` per
    // frequency of the deck's four-point sweep. (`roundtrip.rs` owns the
    // byte layout; this only checks the names reached the file.)
    let bytes = std::fs::read(&written).unwrap();
    for name in [
        &b"freqs"[..],
        &b"Zc_1"[..],
        &b"Zc_2"[..],
        &b"Zc_3"[..],
        &b"Zc_4"[..],
    ] {
        assert!(
            bytes.windows(name.len()).any(|window| window == name),
            "{} missing from Zc.mat",
            String::from_utf8_lossy(name)
        );
    }

    // stdout is still the JSON result, as it is for `run`.
    let json: serde_json::Value = serde_json::from_slice(&bare.stdout).expect("stdout is JSON");
    assert_eq!(json["frequencies_hz"].as_array().unwrap().len(), 4);

    std::fs::remove_dir_all(&directory).unwrap();
}

/// A bare invocation takes a JSON problem document as readily as a deck, and
/// the spiral is the same problem in both — so it writes the same `Zc.mat`.
#[test]
fn bare_invocation_accepts_a_json_problem_document() {
    let directory = scratch("bare-json-document");
    let document = fixture("spiral.json");
    let output = fasterhenry(&directory, &[document.to_str().unwrap()]);
    assert!(output.status.success(), "{}", stderr(&output));
    let from_document = std::fs::read(directory.join("Zc.mat")).expect("Zc.mat written");

    let deck = fixture("spiral.inp");
    let output = fasterhenry(&directory, &[deck.to_str().unwrap()]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        from_document,
        std::fs::read(directory.join("Zc.mat")).unwrap(),
        "the deck and the document are the same problem (see roundtrip.rs)"
    );

    std::fs::remove_dir_all(&directory).unwrap();
}

/// `--zc-mat` still chooses the path in the bare form; the default only
/// applies when nothing was said.
#[test]
fn zc_mat_overrides_the_bare_default_path() {
    let directory = scratch("bare-zc-mat-override");
    let deck = fixture("spiral.inp");
    let output = fasterhenry(
        &directory,
        &[deck.to_str().unwrap(), "--zc-mat", "elsewhere.mat"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(directory.join("elsewhere.mat").is_file());
    assert!(
        !directory.join("Zc.mat").exists(),
        "--zc-mat replaces the default, it does not add to it"
    );
    std::fs::remove_dir_all(&directory).unwrap();
}

/// The 0.1 surface is unchanged: `fasterhenry run <deck>` writes JSON to
/// stdout and nothing else — no `Zc.mat` appears unless `--zc-mat` asks.
#[test]
fn run_subcommand_writes_no_zc_mat_unless_asked() {
    let directory = scratch("run-is-unchanged");
    let deck = fixture("spiral.inp");
    let output = fasterhenry(&directory, &["run", deck.to_str().unwrap()]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        !directory.join("Zc.mat").exists(),
        "`run` must not start writing Zc.mat on its own"
    );
    assert_eq!(
        std::fs::read_dir(&directory).unwrap().count(),
        0,
        "`run` wrote a file it did not write in 0.1"
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout is JSON");
    assert_eq!(json["frequencies_hz"].as_array().unwrap().len(), 4);

    let asked = fasterhenry(
        &directory,
        &["run", deck.to_str().unwrap(), "--zc-mat", "Zc.mat"],
    );
    assert!(asked.status.success(), "{}", stderr(&asked));
    assert!(directory.join("Zc.mat").is_file());

    std::fs::remove_dir_all(&directory).unwrap();
}

/// Nothing is silently ignored on either path: an unrecognized option or an
/// extra positional is an error that names the offending argument, and no
/// sweep runs.
#[test]
fn unrecognized_arguments_are_errors_naming_them() {
    let directory = scratch("unrecognized-arguments");
    let deck = fixture("spiral.inp");
    let deck = deck.to_str().unwrap();

    for args in [
        vec![deck, "--bogus"],
        vec!["--bogus", deck],
        vec![deck, "--bogus=1"],
        vec![deck, "extra-positional"],
        vec!["run", deck, "--bogus"],
    ] {
        let output = fasterhenry(&directory, &args);
        assert!(
            !output.status.success(),
            "{args:?} should have been rejected"
        );
        let message = stderr(&output);
        let named = if args.contains(&"extra-positional") {
            "extra-positional"
        } else {
            "--bogus"
        };
        assert!(
            message.contains(named),
            "the error should name {named}: {message}"
        );
        assert!(output.stdout.is_empty(), "no sweep should have run");
    }
    assert!(!directory.join("Zc.mat").exists(), "no output on an error");

    // A deck that is simply missing is a plain error too, not a usage dump.
    let output = fasterhenry(&directory, &["no-such-deck.inp"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("no-such-deck.inp"));

    std::fs::remove_dir_all(&directory).unwrap();
}

/// `--solver` (issue #44) reaches both forms, and a forced iterative run
/// still announces itself on stderr.
#[test]
fn solver_override_reaches_both_forms() {
    let directory = scratch("solver-override");
    let deck = fixture("spiral.inp");
    let deck = deck.to_str().unwrap();
    let one_frequency = ["--freq", "1e6", "1e6", "1"];

    for form in [vec![deck], vec!["run", deck]] {
        for (choice, announces) in [("dense", false), ("iterative", true)] {
            let mut args = form.clone();
            args.extend_from_slice(&["--solver", choice]);
            args.extend_from_slice(&one_frequency);
            let output = fasterhenry(&directory, &args);
            assert!(output.status.success(), "{}", stderr(&output));
            assert_eq!(
                stderr(&output).contains("solver: iterative"),
                announces,
                "--solver {choice} in {form:?}: {}",
                stderr(&output)
            );
        }
    }
    std::fs::remove_dir_all(&directory).unwrap();
}

/// Both forms answer `--version` and `--help`, and `fasterhenry help` still
/// lists the `run` subcommand.
#[test]
fn help_and_version_describe_both_forms() {
    let directory = scratch("help-and-version");

    let version = fasterhenry(&directory, &["--version"]);
    assert!(version.status.success());
    let text = String::from_utf8_lossy(&version.stdout).into_owned();
    assert!(text.starts_with("fasterhenry "), "{text}");

    // The bare form's help is the option list, and it points at `run`.
    let help = fasterhenry(&directory, &["--help"]);
    assert!(help.status.success());
    let text = String::from_utf8_lossy(&help.stdout).into_owned();
    assert!(
        text.contains("Usage: fasterhenry [OPTIONS] <INPUT>"),
        "{text}"
    );
    assert!(text.contains("fasterhenry run <INPUT>"), "{text}");

    // `help` keeps the 0.1 subcommand listing, and points back at the bare
    // form.
    let help = fasterhenry(&directory, &["help"]);
    assert!(help.status.success());
    let text = String::from_utf8_lossy(&help.stdout).into_owned();
    assert!(text.contains("run "), "{text}");
    assert!(text.contains("./Zc.mat"), "{text}");

    // And no arguments at all is still help (on stderr, as clap's
    // `arg_required_else_help` writes it), not a sweep.
    let nothing = fasterhenry(&directory, &[]);
    assert!(!nothing.status.success());
    assert!(
        stderr(&nothing).contains("Usage: fasterhenry [OPTIONS] <INPUT>"),
        "{}",
        stderr(&nothing)
    );

    std::fs::remove_dir_all(&directory).unwrap();
}

/// The parse itself, without a process: the first argument picks the form,
/// and only the bare one defaults `--zc-mat`.
#[test]
fn the_first_argument_picks_the_form() {
    use fasterhenry_cli::cli::{Invocation, DEFAULT_ZC_MAT};

    let bare = Invocation::try_parse_from(["fasterhenry", "deck.inp"]).unwrap();
    assert!(matches!(bare, Invocation::Bare(_)));
    assert_eq!(bare.args().input, Path::new("deck.inp"));
    assert_eq!(bare.zc_mat(), Some(Path::new(DEFAULT_ZC_MAT)));

    let run = Invocation::try_parse_from(["fasterhenry", "run", "deck.inp"]).unwrap();
    assert!(matches!(run, Invocation::Run(_)));
    assert_eq!(run.args().input, Path::new("deck.inp"));
    assert_eq!(run.zc_mat(), None, "`run` keeps --zc-mat opt-in");

    // An option before the input is still the bare form.
    let bare =
        Invocation::try_parse_from(["fasterhenry", "--solver", "dense", "deck.inp"]).unwrap();
    assert!(matches!(bare, Invocation::Bare(_)));

    // `--zc-mat` wins over the bare default in either form.
    for form in [
        vec!["fasterhenry", "deck.inp", "--zc-mat", "given.mat"],
        vec!["fasterhenry", "run", "deck.inp", "--zc-mat", "given.mat"],
    ] {
        let invocation = Invocation::try_parse_from(form).unwrap();
        assert_eq!(invocation.zc_mat(), Some(Path::new("given.mat")));
    }

    // An unrecognized option is a parse error either way.
    for form in [
        vec!["fasterhenry", "deck.inp", "--bogus"],
        vec!["fasterhenry", "run", "deck.inp", "--bogus"],
    ] {
        let error = Invocation::try_parse_from(form).unwrap_err();
        assert!(error.to_string().contains("--bogus"), "{error}");
    }
}

/// A deck whose `hole point` lies wholly outside its ground plane still
/// solves (issue #105): the run succeeds, stdout is the very JSON result the
/// deck without the clause produces, and stderr carries one `warning:` line
/// naming the clause, the plane and the clause's own physical line.
#[test]
fn a_hole_off_its_plane_solves_with_a_line_numbered_warning() {
    let directory = scratch("hole-off-plane-warning");
    let original = std::fs::read_to_string(fixture("plane_fasthenry.inp")).unwrap();
    // The fixture's `G` statement starts on line 19; its `hole rect` clause
    // is the continuation on line 21. Add a point far off the plane on a
    // continuation line of its own right after it, which is line 22.
    let anchor = "+ hole rect (0.5, 4.5, 0, 1.5, 5.5, 0)\n";
    assert!(original.contains(anchor), "fixture changed");
    let line = original[..original.find(anchor).unwrap()].lines().count() + 2;
    assert_eq!(line, 22);
    let warned = original.replace(anchor, &format!("{anchor}+ hole point (50, 3, 0)\n"));
    std::fs::write(directory.join("plain.inp"), &original).unwrap();
    std::fs::write(directory.join("warned.inp"), &warned).unwrap();

    let plain = fasterhenry(&directory, &["run", "plain.inp"]);
    assert!(plain.status.success(), "{}", stderr(&plain));
    assert!(
        !stderr(&plain).contains("warning:"),
        "the unchanged deck warns about nothing: {}",
        stderr(&plain)
    );

    let output = fasterhenry(&directory, &["run", "warned.inp"]);
    assert!(output.status.success(), "{}", stderr(&output));
    // Byte-identical but for the wall-clock timings.
    let result = |stdout: &[u8]| {
        let mut json: serde_json::Value = serde_json::from_slice(stdout).expect("stdout is JSON");
        json["provenance"]
            .as_object_mut()
            .expect("provenance")
            .remove("timing");
        json
    };
    assert_eq!(
        result(&output.stdout),
        result(&plain.stdout),
        "a hole that removes nothing must not change the result"
    );
    let text = stderr(&output);
    let warnings: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("warning:"))
        .collect();
    assert_eq!(warnings.len(), 1, "{text}");
    let warning = warnings[0];
    assert!(
        warning.starts_with(&format!("warning: line {line}: ")),
        "{warning}"
    );
    assert!(warning.contains("'hole point'"), "{warning}");
    assert!(warning.contains("ground plane 'Gplane'"), "{warning}");
}
