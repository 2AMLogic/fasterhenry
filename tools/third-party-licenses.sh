#!/usr/bin/env bash
# Generate (or verify) THIRD_PARTY_LICENSES.md — the attribution bundle that has
# to ship with any distributed fasterhenry binary.
#
#   ./tools/third-party-licenses.sh            regenerate the committed bundle
#   ./tools/third-party-licenses.sh --check    fail if anything is out of date
#
# `--check` is what CI runs, and it is the reason the bundle cannot go stale: a
# PR that adds or bumps a dependency changes the generated file, and a PR whose
# committed copy does not match what the generator produces fails.
#
# Three things are checked, all of them by this one script so that "run it
# locally exactly as CI does" stays true:
#
#   1. THIRD_PARTY_LICENSES.md matches a fresh generation.
#   2. about.toml's `accepted` list matches deny.toml's `licenses.allow` list.
#      A license allowed through the cargo-deny gate but unknown to cargo-about
#      would fail the generator; one accepted here but not there would attribute
#      a license the gate rejects. Neither is a state to discover at release.
#   3. No workflow distributes a binary artifact without shipping the bundle
#      (see `check_release_paths` for exactly what counts as distributing).
#
# cargo-about is pinned by version AND checksum, like cargo-deny in
# .github/workflows/ci.yml: a tool that documents everyone's licenses should not
# itself arrive as an unverified binary over the network.

set -euo pipefail

# The one version pin. The per-target checksums live in `install_cargo_about`,
# next to the target they belong to; bump them together with this (upstream
# publishes a `.sha256` next to each release asset).
CARGO_ABOUT_VERSION="0.9.2"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUNDLE="$REPO_ROOT/THIRD_PARTY_LICENSES.md"
TEMPLATE="$REPO_ROOT/tools/third-party-licenses.hbs"
CONFIG="$REPO_ROOT/about.toml"
# Under target/, which is gitignored: a cached tool binary is never a repo file.
TOOL_DIR="$REPO_ROOT/target/tools/cargo-about-$CARGO_ABOUT_VERSION"

mode="generate"
case "${1-}" in
    "") ;;
    --check) mode="check" ;;
    -h | --help)
        sed -n '2,25p' "${BASH_SOURCE[0]}"
        exit 0
        ;;
    *)
        echo "error: unknown argument '$1' (expected --check or no argument)" >&2
        exit 2
        ;;
esac

die() {
    echo "error: $*" >&2
    exit 1
}

# --- the tool -----------------------------------------------------------------

# The hex sha256 of a file. Only the "print the digest" form of either tool is
# used: macOS ships an `sha256sum` that is not GNU coreutils and rejects GNU's
# `--check --strict`, so the comparison is done here rather than by the tool.
sha256_of() {
    if command -v sha256sum > /dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    elif command -v shasum > /dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    else
        die "neither sha256sum nor shasum is available to verify the download"
    fi
}

# Echoes the path to a cargo-about binary of exactly $CARGO_ABOUT_VERSION,
# fetching and verifying it on first use. CARGO_ABOUT_BIN overrides the lookup
# for an operator who already has one; the version is still asserted, because a
# different cargo-about is a different rendering and so a spurious diff.
resolve_cargo_about() {
    local bin="${CARGO_ABOUT_BIN-$TOOL_DIR/cargo-about}"
    if [ ! -x "$bin" ]; then
        install_cargo_about "$bin"
    fi
    local got
    got="$("$bin" --version | awk '{print $2}')"
    [ "$got" = "$CARGO_ABOUT_VERSION" ] ||
        die "$bin is cargo-about $got, need $CARGO_ABOUT_VERSION (unset CARGO_ABOUT_BIN to let this script fetch the pinned one)"
    echo "$bin"
}

install_cargo_about() {
    local dest="$1" target sha
    [ "$dest" = "$TOOL_DIR/cargo-about" ] ||
        die "CARGO_ABOUT_BIN=$dest does not exist or is not executable"

    # Target triple and its tarball's sha256, together so they cannot be paired
    # up wrongly. sha256 of the upstream `.tar.gz` for cargo-about 0.9.2.
    case "$(uname -s)/$(uname -m)" in
        Linux/x86_64)
            target="x86_64-unknown-linux-musl"
            sha="9099a59e820c38a68b9d65f300662a567d56562f9a10f6aa4c7e86c17c2566af"
            ;;
        Linux/aarch64 | Linux/arm64)
            target="aarch64-unknown-linux-musl"
            sha="af5169282fb6f84e13471493f405437e43ac517744c9ae12fbe2cdf0a6f0e5a8"
            ;;
        Darwin/arm64)
            target="aarch64-apple-darwin"
            sha="ae72f0df0c399a1e96336f696fa55b1b28679fd725632eba8cf8e4568467cc3e"
            ;;
        *)
            # Upstream publishes no prebuilt binary for this host (an Intel Mac,
            # say). Build the pinned version from source instead — slower, same
            # version, so the output is still the committed bundle.
            echo "no prebuilt cargo-about for $(uname -s)/$(uname -m); building $CARGO_ABOUT_VERSION from source" >&2
            cargo install --locked --quiet --version "$CARGO_ABOUT_VERSION" \
                --root "$TOOL_DIR/cargo-install" cargo-about
            mkdir -p "$TOOL_DIR"
            cp "$TOOL_DIR/cargo-install/bin/cargo-about" "$dest"
            return
            ;;
    esac

    local tarball="cargo-about-$CARGO_ABOUT_VERSION-$target"
    # Not mktemp: a fixed path under the (gitignored) cache dir, so a failed run
    # leaves the half-downloaded tarball where the next one overwrites it
    # instead of needing a trap to clean up.
    local download="$TOOL_DIR/download"
    mkdir -p "$download"
    echo "fetching cargo-about $CARGO_ABOUT_VERSION ($target)" >&2
    curl -fsSL --retry 3 -o "$download/cargo-about.tar.gz" \
        "https://github.com/EmbarkStudios/cargo-about/releases/download/$CARGO_ABOUT_VERSION/$tarball.tar.gz"
    local got
    got="$(sha256_of "$download/cargo-about.tar.gz")"
    [ "$got" = "$sha" ] || die "checksum mismatch for $tarball.tar.gz
  expected $sha
  got      $got"
    tar -xzf "$download/cargo-about.tar.gz" -C "$TOOL_DIR" --strip-components=1 \
        "$tarball/cargo-about"
    rm -rf "$download"
}

# --- the three checks ---------------------------------------------------------

# Renders the bundle to $1. --offline keeps it deterministic (license text comes
# only from the local crate sources, never from clearlydefined.io); --fail turns
# an undeterminable license into an error instead of a silent omission;
# --workspace covers both crates, since the CLI is what gets distributed.
render() {
    local out="$1" bin
    bin="$(resolve_cargo_about)"
    # cargo-about reads the graph through `cargo metadata`, which needs each
    # dependency unpacked in the registry cache; --offline forbids fetching, so
    # populate it first.
    cargo fetch --locked --quiet
    "$bin" generate \
        --config "$CONFIG" \
        --workspace \
        --offline \
        --fail \
        --output-file "$out" \
        "$TEMPLATE"
    # Several crates ship their LICENSE with CRLF line endings, which lands
    # verbatim in the rendered file. Normalize to LF: the bundle is committed and
    # compared byte-for-byte, so a checkout that touches line endings (git's
    # core.autocrlf, an editor) would otherwise make `--check` fail forever on a
    # tree nobody edited. A line ending is not a license term.
    tr -d '\r' < "$out" > "$out.lf"
    mv "$out.lf" "$out"
}

# The quoted SPDX ids inside a named TOML array, one per line, sorted.
toml_string_array() {
    local file="$1" key="$2"
    awk -v key="$key" '
        $0 ~ "^" key " = \\[" { inside = 1; next }
        inside && /^\]/       { inside = 0 }
        inside && match($0, /"[^"]+"/) {
            print substr($0, RSTART + 1, RLENGTH - 2)
        }
    ' "$file" | sort
}

check_allowlist_parity() {
    local accepted allowed
    accepted="$(toml_string_array "$CONFIG" accepted)"
    allowed="$(toml_string_array "$REPO_ROOT/deny.toml" allow)"
    [ -n "$accepted" ] || die "about.toml: could not read the 'accepted' array"
    [ -n "$allowed" ] || die "deny.toml: could not read the 'allow' array"
    if [ "$accepted" != "$allowed" ]; then
        echo "error: about.toml 'accepted' and deny.toml 'allow' disagree:" >&2
        diff <(echo "$allowed") <(echo "$accepted") \
            --label "deny.toml allow" --label "about.toml accepted" -u >&2 || true
        die "keep the two allowlists identical (see the comment at the top of about.toml)"
    fi
    echo "allowlists agree: $(echo "$allowed" | tr '\n' ' ')"
}

# Any binary-artifact release path must ship the bundle. There is no such path
# today (release.yml publishes source crates to crates.io and nothing else), so
# this is the guard that keeps "ships the bundle" from being forgotten when one
# is added: a workflow that hands a built artifact to a user must mention
# THIRD_PARTY_LICENSES.md somewhere in the same file.
#
# It is a tripwire, not a proof. A determined author can satisfy it with a
# comment — the point is that adding an upload step *fails* until somebody has
# looked at the obligation, which is the failure mode that actually happens
# (nobody remembers §4(a) at 2am), not that the grep is unfoolable.
check_release_paths() {
    # Unconditional: every one of these publishes to users wherever it appears.
    local always='softprops/action-gh-release|gh release (create|upload)|docker/build-push-action|docker push|cargo[- ]dist'
    # Conditional: uploading an artifact is only distribution when the workflow
    # is release-triggered. A benchmark or log artifact on a PR run is not.
    local on_release='actions/upload-artifact|actions/upload-pages-artifact'
    local wf failed=0
    for wf in "$REPO_ROOT"/.github/workflows/*.yml "$REPO_ROOT"/.github/workflows/*.yaml; do
        # Unmatched glob stays literal; both extensions are checked because
        # GitHub accepts either.
        [ -e "$wf" ] || continue
        local hits=""
        hits="$(grep -nE "$always" "$wf" || true)"
        # POSIX class, not `\s`: BSD and GNU grep disagree about the latter.
        if grep -qE '^[[:space:]]*(tags|release):' "$wf"; then
            hits="$hits${hits:+$'\n'}$(grep -nE "$on_release" "$wf" || true)"
        fi
        hits="$(printf '%s' "$hits" | sed '/^$/d')"
        [ -n "$hits" ] || continue
        if ! grep -q 'THIRD_PARTY_LICENSES.md' "$wf"; then
            echo "error: $(basename "$wf") distributes an artifact but never mentions THIRD_PARTY_LICENSES.md:" >&2
            printf '%s\n' "$hits" >&2
            failed=1
        fi
    done
    if [ "$failed" -ne 0 ]; then
        die "a workflow that ships a built artifact must ship THIRD_PARTY_LICENSES.md with it (Apache-2.0 §4(a))"
    fi
    echo "release paths: no binary artifact is distributed without the bundle"
}

# --- run ----------------------------------------------------------------------

check_allowlist_parity
check_release_paths

if [ "$mode" = "generate" ]; then
    render "$BUNDLE"
    echo "wrote $BUNDLE"
    exit 0
fi

[ -f "$BUNDLE" ] || die "$BUNDLE does not exist — run ./tools/third-party-licenses.sh"
fresh="$(mktemp)"
trap 'rm -f "$fresh"' EXIT
render "$fresh"
if ! diff -u "$BUNDLE" "$fresh" --label "THIRD_PARTY_LICENSES.md (committed)" \
    --label "THIRD_PARTY_LICENSES.md (regenerated)"; then
    die "THIRD_PARTY_LICENSES.md is out of date — run ./tools/third-party-licenses.sh and commit the result"
fi
echo "THIRD_PARTY_LICENSES.md is up to date"
