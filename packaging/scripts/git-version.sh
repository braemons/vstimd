#!/bin/sh
# The single definition of the vstimd version: the most recent `v*` git tag.
#
# Cargo has no equivalent of Python's dynamic versioning (setuptools-scm):
# `package.version` must be a literal in Cargo.toml, and a build script cannot
# change it because the version is resolved into the dependency graph before
# build scripts run. So the manifests carry the sentinel 0.0.0 (see the root
# Cargo.toml) and every artifact's real version is stamped from here instead.
#
# Deliberately has no fallback. A wrong-but-plausible version silently baked
# into a package is worse than a failed build, so the only two ways to get a
# version are a reachable tag or an explicit VSTIMD_VERSION.
#
# Output is normalised for dpkg and rpm, both of which reject '-' inside a
# version and both of which sort '~' *before* the empty string:
#
#   v0.1.0                    -> 0.1.0
#   v0.1.0-alpha4             -> 0.1.0~alpha4          (sorts before 0.1.0)
#   v0.1.0-alpha4-2-gabc123   -> 0.1.0~alpha4+2.gabc123 (sorts after the tag)
#   ...with uncommitted changes appending +dirty
set -eu

# Reject anything dpkg or rpm would refuse, here rather than 20 minutes into a
# container build. The character set is the intersection of what the two allow
# once '-' is ruled out: dpkg additionally requires a leading digit, and rpm
# rejects the '-' that dpkg permits only as the revision separator. Underscores
# and '^' are each legal in exactly one of the two, so both are excluded.
#
# $2 is what the value was derived from, for the error message.
emit_checked() {
    if ! printf '%s' "$1" | grep -Eq '^[0-9][A-Za-z0-9.+~]*$'; then
        echo "git-version.sh: '$1' is not a usable package version." >&2
        echo "  Derived from: $2" >&2
        echo "  A version must start with a digit and use only [A-Za-z0-9.+~]." >&2
        echo "  Rename the tag (v1.2.3 or v1.2.3-alpha1) or pass VSTIMD_VERSION." >&2
        exit 1
    fi
    printf '%s\n' "$1"
}

# An explicit override always wins. The packaging containers and the release
# workflow use this: neither has a .git dir, so the value is computed once on
# the host and passed in. Still validated — a typo'd override would otherwise
# fail just as obscurely as a bad tag.
if [ -n "${VSTIMD_VERSION:-}" ]; then
    emit_checked "$VSTIMD_VERSION" "VSTIMD_VERSION"
    exit 0
fi

if ! git rev-parse --git-dir >/dev/null 2>&1; then
    echo "git-version.sh: no .git directory and VSTIMD_VERSION is unset." >&2
    echo "  Building outside a git checkout? Pass the version explicitly:" >&2
    echo "    make <target> VSTIMD_VERSION=0.1.0" >&2
    exit 1
fi

if ! describe=$(git describe --tags --match 'v[0-9]*' --dirty=+dirty 2>/dev/null); then
    echo "git-version.sh: no v* tag is reachable from HEAD." >&2
    echo "  Tag a release (git tag v0.1.0), fetch tags into a shallow clone" >&2
    echo "  (git fetch --tags), or pass VSTIMD_VERSION=<version> explicitly." >&2
    exit 1
fi

version=$describe

# Peel the dirty marker off the end first so the distance regex below still
# anchors on '$'.
dirty=
case "$version" in
    *+dirty) dirty=+dirty; version=${version%+dirty} ;;
esac

version=${version#v}

# `git describe` appends -<commits since tag>-g<hash> when HEAD is not the tag
# itself. Re-spell it as a '+' build suffix, which sorts *after* the bare tag.
distance=
if printf '%s' "$version" | grep -Eq -- '-[0-9]+-g[0-9a-f]+$'; then
    distance=$(printf '%s' "$version" | sed -E 's/.*-([0-9]+)-g([0-9a-f]+)$/+\1.g\2/')
    version=$(printf '%s' "$version" | sed -E 's/-[0-9]+-g[0-9a-f]+$//')
fi

# Semver's pre-release '-' becomes '~' so 0.1.0~alpha4 sorts before 0.1.0.
# Any further '-' becomes '.', since neither format allows it inside a version.
version=$(printf '%s' "$version" | sed -e 's/-/~/' -e 's/-/./g')

emit_checked "$version$distance$dirty" "git tag $describe"
