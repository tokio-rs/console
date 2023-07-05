#!/usr/bin/env bash
usage="Releases a tokio-console crate.

USAGE:
    $(basename "$0") [FLAGS] <CRATE> <VERSION>

FLAGS:
    -h, --help      Show this help text and exit.
    -v, --verbose   Enable verbose output.
    -d, --dry-run   Do not change any files or commit a git tag."

set -euo pipefail

bindir=$( cd "${BASH_SOURCE[0]%/*}" && pwd )
rootdir=$( cd "$bindir"/.. && pwd )

# shellcheck source=_util.sh
. "$bindir"/_util.sh

cd "$rootdir"

verify() {
    status "Verifying" "if $crate v$version can be released"

    local branch
    branch=$(git rev-parse --abbrev-ref HEAD)
    if [[ "$branch" != "main" ]]; then
        err "you are not on the 'main' branch"
        if ! confirm "       are you sure you want to release from the '$branch' branch?"; then
            echo "okay, exiting"
            exit 1
        fi
    fi

    if ! cargo --list | grep -q "hack"; then
        err "missing cargo-hack executable"
        if confirm "       install it?"; then
            cargo install cargo-hack
        else
            echo "okay, exiting"
            exit 1
        fi
    fi

    status "Checking" "if $crate builds across feature combinations"

    local cargo_hack=(cargo hack -p "$crate" --feature-powerset --no-dev-deps)

    if [[ "$verbose" ]]; then
        cargo_hack+=("$verbose" check)
    else
        cargo_hack+=(check --quiet)
    fi

    "${cargo_hack[@]}"
    local cargo_hack_status="$?"

    if [[ "$cargo_hack_status" != "0" ]] ; then
        err "$crate did not build with all feature combinations (cargo hack exited with $cargo_hack_status)!"
        exit 1
    fi


    if git tag -l | grep -Fxq "$tag" ; then
        err "git tag \`$tag\` already exists"
        exit 1
    fi
}

update_version() {
    # check the current version of the crate
    local curr_version
    curr_version=$(cargo pkgid -p "$crate" | sed -n 's/.*#\(.*\)/\1/p')
    if [[ "$curr_version" == "$version" ]]; then
        err "crate $crate is already at version $version!"
        if ! confirm "       are you sure you want to release $version?"; then
            echo "okay, exiting"
            exit 0
        fi
    else
        status "Updating" "$crate from $curr_version to $version"
        sed -i \
            "/\[package\]/,/\[.*dependencies\]/{s/^version = \"$curr_version\"/version = \"$version\"/}" \
            "$cargo_toml"
    fi
}

publish() {
    status "Publishing" "$crate v$version"
    cd "$path"
    local cargo_package=(cargo package)
    local cargo_publish=(cargo publish)

    if [[ "$verbose" ]]; then
        cargo_package+=("$verbose")
        cargo_publish+=("$verbose")
    fi

    if [[ "$dry_run" ]]; then
        cargo_publish+=("$dry_run")
    fi

    "${cargo_package[@]}"
    "${cargo_publish[@]}"

    status "Tagging" "$tag"
    local git_tag=(git tag "$tag")
    local git_push_tags=(git push --tags)
    if [[ "$dry_run" ]]; then
        echo "# " "${git_tag[@]}"
        echo "# " "${git_push_tags[@]}"
    else
        "${git_tag[@]}"
        "${git_push_tags[@]}"
    fi
}

update_changelog() {
    # shellcheck source=update-changelog
    . "$bindir"/update-changelog.sh
    changelog_status="$?"

    if [[ $changelog_status -ne 0 ]]; then
        err "failed to update changelog"
        exit "$changelog_status"
    fi
}


verbose=''
dry_run=''

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
    -d|--dry-run)
        dry_run="--dry-run"
        ;;
    -*)
        err "unknown flag $arg"
        echo "$usage"
        exit 1
        ;;
    *) # crate or version
        if [[ -z "${crate+crate}" ]]; then
            crate="$arg"
        elif [[ -z "${version+version}" ]]; then
            version="$arg"
        else
            err "unknown positional argument \"$arg\""
            echo "$usage"
            exit 1
        fi
        ;;
    esac
done

if [[ "$verbose" ]]; then
    set -x
fi

if [[ -z "${version+version}" ]]; then
    err "no version specified!"
    errexit=1
fi

if [[ "${crate+crate}" ]]; then
    tag="$crate-v$version"
else
    err "no crate specified!"
    errexit=1
fi

if [[ "${errexit+errexit}" ]]; then
    echo "$usage"
    exit 1
fi

path=$(crate_path "$crate")

cargo_toml="${path}/Cargo.toml"
changelog="${path}/CHANGELOG.md"

files=("$cargo_toml" "$changelog")

is_uncommitted=''
for file in "${files[@]}"; do
    if ! git diff-index --quiet HEAD -- "$file"; then
        err "would overwrite uncommitted changes to $file!"
        is_uncommitted=1
    fi
done

if [[ "$is_uncommitted" ]]; then
    exit 1
fi

verify
update_version
update_changelog

staged="$(git diff-index --cached --name-only HEAD --)"
if [[ "$staged" ]]; then
    err "skipping commit, as it would include the following unrelated staged files:"
    echo "$staged"
    exit 1
fi

status "Ready" "to prepare release commit!"
echo ""

git add "${files[@]}"
git diff --staged

if [[ "$dry_run" ]]; then
    git reset HEAD -- "${files[@]}"
    git checkout HEAD -- "${files[@]}"
fi

echo ""

if confirm "commit and push?"; then
    git_commit=(git commit -sS -m "chore($crate): prepare to release $crate $version")

    if [[ "$dry_run" ]]; then

        echo ""
        echo "# " "${git_commit[@]}"
        echo "# " "${git_push[@]}"
    else
        "${git_commit[@]}"
    fi
else
    echo "okay, exiting"
    exit 1
fi

if confirm "publish the crate?"; then

    echo ""
    publish
else
    echo "okay, exiting"
    exit 1
fi

cd "$rootdir"
git add "Cargo.lock"
git_push=(git push -u origin --force-with-lease)
git_amend=(git commit --amend --reuse-message HEAD)
if [[ "$dry_run" ]]; then
    echo ""
    echo "# git add Cargo.lock"
    echo "# " "${git_amend[@]}"
    echo "# " "${git_push[@]}"
else
    "${git_amend[@]}"
    "${git_push[@]}"
fi
