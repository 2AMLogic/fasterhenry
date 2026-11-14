#!/usr/bin/env bash
# Self-test for tools/assert-gate-ran.sh.
#
#   ./tools/test_assert_gate_ran.sh
#
# The script under test is the single assertion standing between a silently
# skipped validation gate and a green job (#59, #362), so the cases that matter
# most here are the ones where it must FAIL: a count that does not match, and a
# log it could not read. The latter is the fail-closed requirement from
# .loom/docs/ci-principles.md — an absent log is "not verified", never "zero
# markers found, therefore compare and move on".
#
# Exits 0 when every case passes, 1 on the first failure (each is named).

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$HERE/assert-gate-ran.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failures=0

# run <expected-status> <name> -- <args...>
# Invokes the script, compares its exit status, and stashes output in $out.
out=""
run() {
    local want="$1" name="$2"
    shift 3 # want, name, --
    local status=0
    out="$("$SCRIPT" "$@" 2>&1)" || status=$?
    if [ "$status" -ne "$want" ]; then
        echo "FAIL $name: exit $status, expected $want"
        printf '  output: %s\n' "$out"
        failures=$((failures + 1))
        return 1
    fi
    return 0
}

expect_output() {
    local name="$1" needle="$2"
    case "$out" in
        *"$needle"*) ;;
        *)
            echo "FAIL $name: output does not contain '$needle'"
            printf '  output: %s\n' "$out"
            failures=$((failures + 1))
            ;;
    esac
}

# --- the passing case ---------------------------------------------------------

cat > "$work/one.log" << 'EOF'
running 1 test
SPIRAL GATE PASSED (max rel err 1.2e-3)
test result: ok. 1 passed
EOF

if run 0 "exact count matches" -- "$work/one.log" "SPIRAL GATE PASSED" 1 "spiral gates"; then
    expect_output "exact count matches" "spiral gates: 1 of 1 ran"
fi

# --- the failing cases that are the whole point -------------------------------

# A gate that did not run: the marker is absent, the count is 0, the step fails.
cat > "$work/skipped.log" << 'EOF'
running 1 test
test result: ok. 0 passed; 1 ignored
EOF

if run 1 "missing marker fails" -- "$work/skipped.log" "SPIRAL GATE PASSED" 1 "spiral gates"; then
    expect_output "missing marker fails" "::error::spiral gates: 0 of 1 ran"
fi

# One of two gates ran -- the drift that an `if grep -q SKIPPED` cannot see.
cat > "$work/partial.log" << 'EOF'
CONTACT GATE PASSED (solver: mesh)
EOF

if run 1 "undercount fails" -- "$work/partial.log" "CONTACT GATE PASSED" 2 "plane contact gates"; then
    expect_output "undercount fails" "::error::plane contact gates: 1 of 2 ran"
fi

# More markers than expected is also a mismatch: the expected number is the
# assertion, not a floor.
cat > "$work/extra.log" << 'EOF'
SLOT GATE PASSED
SLOT GATE PASSED
EOF

run 1 "overcount fails" -- "$work/extra.log" "SLOT GATE PASSED" 1 "plane slot gates" &&
    expect_output "overcount fails" "2 of 1 ran"

# --- fail closed on a log that cannot be read ---------------------------------

if run 1 "missing log fails closed" -- "$work/nope.log" "SPIRAL GATE PASSED" 1 "spiral gates"; then
    expect_output "missing log fails closed" "was NOT verified"
fi

# A count of 0 must not turn an unreadable log into a pass.
if run 1 "missing log is not a zero count" -- "$work/nope.log" "ANY MARKER" 0 "zero-expected gates"; then
    expect_output "missing log is not a zero count" "was NOT verified"
fi

printf '' > "$work/unreadable.log"
chmod 000 "$work/unreadable.log"
# root ignores the mode bits, so this case is only meaningful unprivileged.
if [ ! -r "$work/unreadable.log" ]; then
    if run 1 "unreadable log fails closed" -- "$work/unreadable.log" "SPIRAL GATE PASSED" 1 "spiral gates"; then
        expect_output "unreadable log fails closed" "was NOT verified"
    fi
else
    echo "SKIP unreadable log fails closed: running as a user that bypasses file modes"
fi
chmod 644 "$work/unreadable.log"

# --- the marker is a basic regular expression ---------------------------------

cat > "$work/spice.log" << 'EOF'
ngspice stamp vs JSON: 1 ports checked
ngspice stamp vs JSON: 2 ports checked
EOF

if run 0 "bracket expression counts both runs" -- \
    "$work/spice.log" "ngspice stamp vs JSON: [12] ports checked" 2 "SPICE comparisons"; then
    expect_output "bracket expression counts both runs" "SPICE comparisons: 2 of 2 ran"
fi

# --- usage errors are distinguishable from a failed gate (exit 2, not 1) ------

run 2 "too few arguments" -- "$work/one.log" "SPIRAL GATE PASSED" 1
run 2 "too many arguments" -- "$work/one.log" "SPIRAL GATE PASSED" 1 "label" "extra"
run 2 "non-numeric count" -- "$work/one.log" "SPIRAL GATE PASSED" one "label"
run 2 "negative count" -- "$work/one.log" "SPIRAL GATE PASSED" -1 "label"

if run 0 "--help" -- --help; then
    expect_output "--help" "<log> <marker> <expected-count> <label>"
fi

# --- result -------------------------------------------------------------------

if [ "$failures" -ne 0 ]; then
    echo "$failures case(s) failed"
    exit 1
fi
echo "all assert-gate-ran.sh cases passed"
