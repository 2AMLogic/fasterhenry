# fasterhenry-cli

Command-line front end for [`fasterhenry`](https://crates.io/crates/fasterhenry),
a clean-room, MIT-licensed Rust engine for **PEEC** (partial-element equivalent
circuit) extraction of resistance and inductance of 3-D conductor geometries.

This crate installs a binary named **`fasterhenry`** (the crate is
`fasterhenry-cli`; the library crate owns the `fasterhenry` package name).

> **Status: 0.0.x, pre-release.** The command-line interface is unstable and
> will change without notice between patch versions. The engine behind it is
> the dense M0 core.

## Install

```bash
cargo install fasterhenry-cli
```

## Usage

```text
$ fasterhenry --help
Command-line front end for fasterhenry: FastHenry-format .inp decks (public format) and JSON in, JSON out

Usage: fasterhenry

Options:
  -h, --help     Print help
  -V, --version  Print version
```

```text
$ fasterhenry --version
fasterhenry 0.0.1
```

The planned interface reads a FastHenry-format `.inp` deck (the public file
format) or a JSON problem description and writes the extracted
resistance/inductance results as JSON. Run `fasterhenry --help` for the options
your installed version actually supports; this README tracks the released
binary, not the plan.

## License

MIT — see `LICENSE`. Changes are recorded in `CHANGELOG.md`. Source and issues:
<https://github.com/2AMLogic/fasterhenry>.
