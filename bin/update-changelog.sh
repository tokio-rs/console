#!/usr/bin/env bash
usage="Updates the changelog for a Tokio Console crate.

USAGE:
    $(basename "$0") [FLAGS] <CRATE_PATH> <TAG>

FLAGS:
    -h, --help      Show this help text and exit.
    -v, --verbose   Enable verbose output."

set -euo pipefail

bindir=$( cd "${BASH_SOURCE[0]%/*}" && pwd )
rootdir=$( cd "$bindir"/.. && pwd )

# shellcheck source=_util.sh
. "$bindir"/_util.sh

cd "$rootdir"

verbose=''

for arg in "$@"
do
    case "$arg" in
    -h|--help)
        echo "$usage"
        exit 0
        ;;
    -v|--verbose)
        verbose="--verbose"
        ;;
    -*)
        err "unknown flag $arg"
        echo "$usage"
        exit 1
        ;;
    *) # crate or version
        if [[ -z "${path+path}" ]]; then
            path="$arg"
        elif [[ -z "${tag+tag}" ]]; then
            tag="$arg"
        else
            err "unknown positional argument \"$arg\""
            echo "$usage"
            exit 1
        fi
        ;;
    esac
done

if [[ -z "${path+path}" ]]; then
    err "no version specified!"
    errexit=1
fi

if [[ -z "${tag+tag}" ]]; then
    err "no tag specified!"
    errexit=1
fi

if [[ "${errexit+errexit}" ]]; then
    echo "$usage"
    exit 1
fi

if ! [[ -x "$(command -v git-cliff)" ]]; then
    err "missing git-cliff executable"
    if confirm "       install it?"; then
        cargo install git-cliff
    else
        echo "okay, exiting"
        exit 0
    fi
fi

changelog_path="${path}/CHANGELOG.md"

status "Updating" "$changelog_path for tag $tag"

git_cliff=(
    git-cliff
    --include-path "${path}/**"
    --output "$changelog_path"
    --config cliff.toml
    # --tag "$tag"
)
if [[ "$verbose" ]]; then
    git_cliff+=("$verbose")
fi

export GIT_CLIFF__GIT__TAG_PATTERN="${path}-v[0-9]+.[0-9]+.[0-9]+"
"${git_cliff[@]}"