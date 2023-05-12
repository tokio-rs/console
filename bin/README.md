# tokio-console dev scripts

This directory contains shell scripts useful for Tokio Console development.
Currently, all the scripts in this directory are related to publishing releases.

- `release.sh`: Releases a new version of a Tokio Console crate. This includes
  updating the crate's version in its `Cargo.toml`, updating the changelog,
  running pre-release tests, creating a release tag, and publishing the crate to
  crates.io.

  Invoked with the name of the crate to release, and the version of the new
  release. For example:

  ```console
  $ bin/release.sh tokio-console 0.1.9
  ```

  The script will validate whether a new release can be published prior to
  updating the changelog and crate version. Then, the script will display the
  git diff for the generated release commit, and prompt the user to confirm
  that it is correct prior to publishing the release.

  Releases should be published on the `main` branch. Note that this script
  requires that the user is authenticated to publish releases of the crate in
  question to crates.io.

- `update-changelog.sh`: Updates the generated `CHANGELOG.md` for a given crate
  and version, without committing the changelog update or publishing a tag.

  Invoked with the path to the crate to generate change notes for, and the name
  that will be used for the new release's Git tag. For example:

  ```console
  $ bin/update-changelog.sh tokio-console tokio-console-v0.1.9
  ```

  The `release.sh` script will run this script automatically as part of the
  release process. However, it can also be invoked separately to just update the
  changelog.

- `_util.sh`: Contains utilities used by other shell scripts. This script is not
  run directly.
