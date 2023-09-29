#!/usr/bin/env bash
usage="Updates the changelog for a Tokio Console crate.

USAGE:
    $(basename "$0") [FLAGS] <CRATE_PATH> <TAG>

FLAGS:
    -h, --help                      Show this help text and exit.
    -v, --verbose                   Enable verbose output.
    -u, --unreleased                Only add unreleased changes to changelog
    --changelog-path <FILE_PATH>    Write the changelog to this path.
                                    default: <CRATE_PATH>/CHANGELOG.md
    -p, --prepend                   Prepend the changelog to the existing file.
                                    default: regenerate the entire file."

set -euo pipefail

bindir=$( cd "${BASH_SOURCE[0]%/*}" && pwd )
rootdir=$( cd "$bindir"/.. && pwd )

# shellcheck source=_util.sh
. "$bindir"/_util.sh

cd "$rootdir"

verbose=''
unreleased=''
changelog_path=''
prepend=''

while [[ $# -gt 0 ]]
do
    arg=$1
    shift
    case "$arg" in
    -h|--help)
        echo "$usage"
        exit 0
        ;;
    -v|--verbose)
        verbose="--verbose"
        ;;
    -u|--unreleased)
        unreleased="--unreleased"
        ;;
    -p|--prepend)
        prepend="--prepend"
        ;;
    --changelog-path)
        changelog_path="$1"
        shift
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

if [[ -z "$changelog_path" ]]; then
    changelog_path="${path}/CHANGELOG.md"
fi

status "Updating" "$changelog_path for tag $tag"

git_cliff=(
    git-cliff
    --include-path "${path}/**"
    --config cliff.toml
    --tag "$tag"
)
if [[ "$verbose" ]]; then
    git_cliff+=("$verbose")
fi

if [[ "$unreleased" ]]; then
    git_cliff+=("$unreleased")
fi

if [[ "$prepend" ]]; then
    git_cliff+=("--prepend")
else
    git_cliff+=("--output")
fi

git_cliff+=("$changelog_path")

export GIT_CLIFF__GIT__TAG_PATTERN="${path}-v[0-9]*"
"${git_cliff[@]}"