#!/usr/bin/env bash
# Assert that a validation gate actually ran, by counting the positive marker it
# prints on success against an exact expected number.
#
#   ./tools/assert-gate-ran.sh <log> <marker> <expected-count> <label>
#
# Every reference-backed gate in docs/validation.md prints a marker on stdout
# only after its comparison has completed and passed. Counting that marker is
# what makes a gate's *absence* fail the job: the alternative — grepping the log
# for the test's skip reason — cannot fail, because skip reasons are `eprintln!`
# and so never entered a stdout-only log. That is exactly how the spiral and
# slot gates stayed green while skipping on every run until #59, and why this
# script asserts positively and never negatively.
#
# <marker> is a basic regular expression, as `grep` reads it: the SPICE gate
# counts `ngspice stamp vs JSON: [12] ports checked`, where `[12]` matches the
# one- and two-port runs with one pattern.
#
# <label> names what was being counted and opens the `::error::` annotation, so
# it should read as a subject: "spiral_validation PyPEEC gates".
#
# Fails closed, per .loom/docs/ci-principles.md ("a check that cannot run must
# not look like a check that passed"): a log that is missing or unreadable, or a
# `grep` that errors, is reported as *not verified* and exits non-zero. It is
# never treated as "zero markers found, now compare".

set -euo pipefail

usage() {
    sed -n '2,25p' "${BASH_SOURCE[0]}"
}

case "${1-}" in
    -h | --help)
        usage
        exit 0
        ;;
esac

if [ "$#" -ne 4 ]; then
    echo "error: expected 4 arguments (<log> <marker> <expected-count> <label>), got $#" >&2
    usage >&2
    exit 2
fi

log="$1"
marker="$2"
expected="$3"
label="$4"

case "$expected" in
    '' | *[!0-9]*)
        echo "error: expected-count '$expected' is not a non-negative integer" >&2
        exit 2
        ;;
esac

# Fail closed. Without the log there is no evidence either way, and "no
# evidence" must not be reported as "the gate ran".
if [ ! -r "$log" ]; then
    echo "::error::$label: $log is missing or unreadable, so the gate was NOT verified"
    exit 1
fi

# grep exits 0 for a match, 1 for none, and >1 for an error. Only the first two
# are answers; an error is another way of not having verified anything.
set +e
got="$(grep -c -- "$marker" "$log")"
rc=$?
set -e
if [ "$rc" -gt 1 ]; then
    echo "::error::$label: grep exited $rc reading $log, so the gate was NOT verified"
    exit 1
fi

if [ "$got" -ne "$expected" ]; then
    echo "::error::$label: $got of $expected ran (counting \"$marker\" in $log)"
    exit 1
fi

echo "$label: $got of $expected ran"
