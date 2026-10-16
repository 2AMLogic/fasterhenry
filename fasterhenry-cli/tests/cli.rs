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

/// A one-frequency copy of the spiral deck in `directory`, for the
/// destination-collision tests: returns its path and its bytes.
fn deck_in(directory: &Path, name: &str) -> (PathBuf, Vec<u8>) {
    let bytes = std::fs::read(fixture("spiral.inp")).unwrap();
    let path = directory.join(name);
    std::fs::write(&path, &bytes).unwrap();
    (path, bytes)
}

/// The run was refused before it touched anything: nonzero exit, nothing on
/// stdout, and a diagnostic naming every `needle`.
fn assert_refused(output: &Output, needles: &[&str]) {
    assert!(!output.status.success(), "should have been refused");
    assert!(output.stdout.is_empty(), "no sweep should have run");
    let message = stderr(output);
    for needle in needles {
        assert!(message.contains(needle), "missing {needle:?}: {message}");
    }
}

const ONE_FREQUENCY: [&str; 4] = ["--freq", "1e6", "1e6", "1"];

/// Every output flag pointed at the input deck itself — directly, through
/// `.` components, and (on unix) through a symlink or a hard link — is
/// refused before anything is written, with both roles and paths named, and
/// the deck's bytes survive (issue #175).
#[test]
fn an_output_that_is_the_input_is_refused() {
    let directory = scratch("output-is-input");
    std::fs::create_dir(directory.join("sub")).unwrap();
    let (deck, original) = deck_in(&directory, "deck.inp");
    let mut aliases = vec![
        "deck.inp".to_string(),
        "./deck.inp".to_string(),
        "sub/../deck.inp".to_string(),
        deck.to_str().unwrap().to_string(),
    ];
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("deck.inp", directory.join("symlink.inp")).unwrap();
        std::fs::hard_link(&deck, directory.join("hardlink.inp")).unwrap();
        aliases.push("symlink.inp".to_string());
        aliases.push("hardlink.inp".to_string());
    }
    for alias in &aliases {
        for flag in ["--json", "--zc-mat", "--spice"] {
            for form in [vec!["deck.inp"], vec!["run", "deck.inp"]] {
                let mut args = form.clone();
                args.extend_from_slice(&[flag, alias]);
                args.extend_from_slice(&ONE_FREQUENCY);
                let output = fasterhenry(&directory, &args);
                assert_refused(&output, &[flag, alias, "input", "deck.inp"]);
                assert_eq!(
                    std::fs::read(&deck).unwrap(),
                    original,
                    "{args:?} changed the deck"
                );
                assert!(
                    !directory.join("Zc.mat").exists(),
                    "{args:?} wrote Zc.mat before refusing"
                );
            }
        }
    }
    std::fs::remove_dir_all(&directory).unwrap();
}

/// A bare invocation's implicit `./Zc.mat` counts: a deck (or JSON document)
/// named `Zc.mat` in the working directory is refused, as is another output
/// flag pointed at `Zc.mat` (issue #175).
#[test]
fn the_implicit_zc_mat_collides_like_an_explicit_one() {
    let directory = scratch("implicit-zc-mat");
    let document = std::fs::read(fixture("spiral.json")).unwrap();
    std::fs::write(directory.join("Zc.mat"), &document).unwrap();
    let output = fasterhenry(&directory, &["Zc.mat"]);
    assert_refused(&output, &["default Zc.mat", "input"]);
    assert_eq!(std::fs::read(directory.join("Zc.mat")).unwrap(), document);
    // `run` writes no Zc.mat, so the same input is fine there.
    let output = fasterhenry(&directory, &["run", "Zc.mat"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(std::fs::read(directory.join("Zc.mat")).unwrap(), document);
    std::fs::remove_file(directory.join("Zc.mat")).unwrap();

    let (_, _) = deck_in(&directory, "deck.inp");
    for (flag, alias) in [("--json", "Zc.mat"), ("--spice", "./Zc.mat")] {
        let mut args = vec!["deck.inp", flag, alias];
        args.extend_from_slice(&ONE_FREQUENCY);
        let output = fasterhenry(&directory, &args);
        assert_refused(&output, &["default Zc.mat", flag, alias]);
        assert!(!directory.join("Zc.mat").exists());
    }
    std::fs::remove_dir_all(&directory).unwrap();
}

/// Two output flags naming one file — directly, through `.` components, or
/// (on unix) through a symlink or hard link to an existing file — are
/// refused before either is written; an existing file at the shared path
/// keeps its bytes (issue #175).
#[test]
fn two_outputs_naming_one_file_are_refused() {
    let directory = scratch("two-outputs-one-file");
    std::fs::create_dir(directory.join("sub")).unwrap();
    deck_in(&directory, "deck.inp");
    let existing = b"previous contents".to_vec();
    std::fs::write(directory.join("out"), &existing).unwrap();
    let mut pairs = vec![
        ("out", "out"),
        ("out", "./out"),
        ("out", "sub/../out"),
        ("new", "./sub/../new"),
    ];
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("out", directory.join("symlink")).unwrap();
        std::os::unix::fs::symlink("new", directory.join("dangling")).unwrap();
        std::fs::hard_link(directory.join("out"), directory.join("hardlink")).unwrap();
        pairs.push(("out", "symlink"));
        pairs.push(("out", "hardlink"));
        pairs.push(("new", "dangling"));
    }
    for (first, second) in pairs {
        for (first_flag, second_flag) in [
            ("--json", "--zc-mat"),
            ("--json", "--spice"),
            ("--zc-mat", "--spice"),
        ] {
            let mut args = vec!["run", "deck.inp", first_flag, first, second_flag, second];
            args.extend_from_slice(&ONE_FREQUENCY);
            let output = fasterhenry(&directory, &args);
            assert_refused(
                &output,
                &[first_flag, first, second_flag, second, "same file"],
            );
            assert_eq!(std::fs::read(directory.join("out")).unwrap(), existing);
            assert!(!directory.join("new").exists(), "{args:?} wrote new");
        }
    }
    std::fs::remove_dir_all(&directory).unwrap();
}

/// Distinct destinations all get written, and an unrelated existing output
/// file is still replaced, as it always was (issue #175).
#[test]
fn distinct_outputs_are_written_and_replace_unrelated_files() {
    let directory = scratch("distinct-outputs");
    let (deck, original) = deck_in(&directory, "deck.inp");
    std::fs::create_dir(directory.join("sub")).unwrap();
    for name in ["out.json", "sub/out.cir", "Zc.mat"] {
        std::fs::write(directory.join(name), b"stale").unwrap();
    }
    let mut args = vec!["deck.inp", "--json", "./out.json", "--spice", "sub/out.cir"];
    args.extend_from_slice(&ONE_FREQUENCY);
    let output = fasterhenry(&directory, &args);
    assert!(output.status.success(), "{}", stderr(&output));
    for name in ["out.json", "sub/out.cir", "Zc.mat"] {
        assert_ne!(
            std::fs::read(directory.join(name)).unwrap(),
            b"stale",
            "{name} was not replaced"
        );
    }
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("out.json")).unwrap()).unwrap();
    assert_eq!(json["frequencies_hz"].as_array().unwrap().len(), 1);
    assert_eq!(std::fs::read(&deck).unwrap(), original);
    std::fs::remove_dir_all(&directory).unwrap();
}

/// Runs `--spice` on a copy of the spiral fixture renamed to `file_name`
/// and returns the exported netlist.
fn spice_export_of_renamed_spiral(scratch_name: &str, file_name: &str) -> String {
    let directory = scratch(scratch_name);
    let deck = directory.join(file_name);
    std::fs::copy(fixture("spiral.inp"), &deck).expect("copy the fixture");
    let netlist = directory.join("out.cir");
    let output = Command::new(BINARY)
        .arg("run")
        .arg(&deck)
        .arg("--freq")
        .args(["1e6", "1e6", "1"])
        .arg("--spice")
        .arg(&netlist)
        .current_dir(&directory)
        .output()
        .expect("the binary runs");
    assert!(output.status.success(), "{}", stderr(&output));
    std::fs::read_to_string(&netlist).expect("the netlist is written")
}

/// The `.subckt` and `.ends` lines of a one-port export, asserting each
/// directive appears exactly once and names `expected` as a single token.
fn assert_single_subckt(text: &str, expected: &str) {
    let subckt: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with(".subckt"))
        .collect();
    let ends: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with(".ends"))
        .collect();
    assert_eq!(subckt, vec![format!(".subckt {expected} p1 n1")], "{text}");
    assert_eq!(ends, vec![format!(".ends {expected}")], "{text}");
    // The comment header carries the name too and stays one line.
    assert_eq!(
        text.lines().filter(|line| line.starts_with('*')).count(),
        2,
        "{text}"
    );
    // The impedance stamp is intact.
    assert!(text.contains("L1 p1 a0_0 "), "{text}");
    assert!(text.contains("H1_1 a0_0 a0_1 vsense1 "), "{text}");
    assert!(text.contains("vsense1 a0_1 n1 0"), "{text}");
}

/// A plain stem passes through unchanged.
#[test]
fn spice_subckt_name_keeps_a_simple_stem() {
    let text = spice_export_of_renamed_spiral("spice-simple", "spiral.inp");
    assert_single_subckt(&text, "spiral");
}

/// Spaces, punctuation and Unicode in the file name collapse into one
/// `[A-Za-z0-9_]` token, the same on `.subckt` and `.ends`.
#[test]
fn spice_subckt_name_normalizes_unsafe_file_names() {
    for (index, (file_name, expected)) in [
        ("my coil.inp", "my_coil"),
        ("v1.2-final(x).inp", "v1_2_final_x_"),
        ("bobine\u{e9}.inp", "bobine_"),
        ("\u{7dda}\u{5708}.inp", "__"),
    ]
    .into_iter()
    .enumerate()
    {
        let text = spice_export_of_renamed_spiral(&format!("spice-unsafe-{index}"), file_name);
        assert_single_subckt(&text, expected);
    }
}

/// Tabs and newlines in the file name cannot add tokens or netlist lines,
/// so filename text cannot inject directives.
#[cfg(unix)]
#[test]
fn spice_subckt_name_neutralizes_control_characters() {
    for (index, (file_name, expected)) in [
        ("a\tb.inp", "a_b"),
        ("x\n.ends\n.subckt evil.inp", "x__ends__subckt_evil"),
    ]
    .into_iter()
    .enumerate()
    {
        let text = spice_export_of_renamed_spiral(&format!("spice-control-{index}"), file_name);
        assert_single_subckt(&text, expected);
    }
}

/// `segwid1=` on a nonuniform plane (issue #155) is a line-numbered error
/// natively and is ignored with a line-numbered warning under
/// `--fasthenry-compat`; `segwid1=` above the cell spacing on a uniform plane
/// solves with a line-numbered warning.
#[test]
fn segwid_reaches_the_executable_with_line_numbered_diagnostics() {
    let directory = scratch("segwid-diagnostics");
    let deck = |plane_tail: &str| {
        format!(
            "\
.units mm
.default sigma=5.8e4
Gp x1=0 y1=0 z1=0 x2=10 y2=0 z2=0 x3=10 y3=10 z3=0
+ thick=0.04 {plane_tail}
Nt x=5 y=5 z=0.5
Nb x=5 y=5 z=0
Ev Nt Nb w=0.2 h=0.2
.external Nt Nb
.freq fmin=1 fmax=1 ndec=1
.end
"
        )
    };
    std::fs::write(
        directory.join("nonuniform.inp"),
        deck("seg1=4 seg2=4 segwid1=0.5 file=NONE"),
    )
    .unwrap();
    std::fs::write(directory.join("wide.inp"), deck("seg1=4 seg2=4 segwid2=9")).unwrap();

    let native = fasterhenry(&directory, &["run", "nonuniform.inp"]);
    assert!(!native.status.success());
    let text = stderr(&native);
    assert!(
        text.contains("line 4") && text.contains("'segwid1'"),
        "{text}"
    );

    let compat = fasterhenry(&directory, &["run", "--fasthenry-compat", "nonuniform.inp"]);
    assert!(compat.status.success(), "{}", stderr(&compat));
    let text = stderr(&compat);
    assert!(
        text.lines()
            .any(|l| l.starts_with("warning: line 4: ") && l.contains("'segwid1' is ignored")),
        "{text}"
    );

    let wide = fasterhenry(&directory, &["run", "wide.inp"]);
    assert!(wide.status.success(), "{}", stderr(&wide));
    let text = stderr(&wide);
    assert!(
        text.lines()
            .any(|l| l.starts_with("warning: line 4: ") && l.contains("segment separation")),
        "{text}"
    );
}

/// A MAT result small enough to sit entirely in the `BufWriter` fails only
/// at the final drain. `/dev/full` opens fine (so `File::create` succeeds)
/// and rejects every write with ENOSPC; before the explicit flush the error
/// was lost when the writer was dropped and the CLI exited 0.
#[cfg(target_os = "linux")]
#[test]
fn a_failing_final_mat_drain_is_a_nonzero_exit_naming_the_path() {
    let device = Path::new("/dev/full");
    if !device.exists() {
        return;
    }
    // The destination opens for writing, so any failure is not at create.
    std::fs::OpenOptions::new()
        .write(true)
        .open(device)
        .expect("/dev/full opens for writing");

    let directory = scratch("mat-drain-failure");
    let deck = fixture("spiral.inp");
    let output = fasterhenry(
        &directory,
        &["run", deck.to_str().unwrap(), "--zc-mat", "/dev/full"],
    );
    let message = stderr(&output);
    assert!(!output.status.success(), "must exit nonzero: {message}");
    assert!(message.contains("/dev/full"), "names the path: {message}");
    assert!(
        message.contains("cannot write") && !message.contains("cannot create"),
        "fails at the write, not at create: {message}"
    );
}
