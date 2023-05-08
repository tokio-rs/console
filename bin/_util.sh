#!/usr/bin/env bash
# utility functions used in other shell scripts.
#
# currently, this includes:
# - cargo-style stderr logging (`err`, `note`, and `status` functions)
# - confirmation prompts (`confirm` function)
set -euo pipefail

# Log an error to stderr
#
# Args:
#     $1: message to log
err() {
    echo -e "\e[31m\e[1merror:\e[0m" "$@" 1>&2;
}

# Log a note to stderr
#
# Args:
#     $1: message to log
note() {
    echo -e "\e[31m\e[1mnote:\e[0m" "$@" 1>&2;
}

# Log a cargo-style status message to stderr
#
# Args:
#     $1: a "tag" for the log message (should be 12 characters or less in
#     length)
#    $2: message to log
status() {
    local width=12
    local tag="$1"
    local msg="$2"
    printf "\e[32m\e[1m%${width}s\e[0m %s\n" "$tag" "$msg"
}

# Prompt the user to confirm an action
#
# Args:
#    $1: message to display to the user along with the `[y/N]` prompt
#
# Returns:
#    0 if the user confirmed, 1 otherwise
confirm() {
    while read -r -p "$1 [Y/n] " input
    do
        case "$input" in
            [yY][eE][sS]|[yY])
                return 0
                ;;
            [nN][oO]|[nN])
                return 1
                ;;
            *)
                err "invalid input $input"
                ;;
        esac
    done
}

# Returns the path to a Mycelium crate.
#
# Args:
#     $1: crate name
#
# Returns:
#     0 if the crate exists, 0 if it does not exist.
crate_path() {
    local crate="$1"
    local mycoprefix='mycelium-';
    if [[ -d $crate ]]; then
        echo "$crate"
    elif [[ -d "${crate#"$mycoprefix"}" ]]; then
        echo "${crate#"$mycoprefix"}"
    else
        err "unknown crate $crate"
        return 1;
    fi
}
