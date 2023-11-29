use std::env;

#[test]
// You can use `TRYCMD=overwrite` to overwrite the expected output.
fn cli_tests() {
    // 72 chars seems to fit reasonably in the default width of
    // RustDoc's code formatting
    // Set the env var COLUMNS to override this.
    env::set_var("COLUMNS", "72");
    // The following might be changed by the test runner, so we
    // explicitly set them to known values.
    env::set_var("LANG", "en_US.UTF-8");
    env::set_var("COLORTERM", "truecolor");

    let t = trycmd::TestCases::new();
    let console = trycmd::cargo::cargo_bin("tokio-console");
    t.register_bin("tokio-console", console);
    let readme_path = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .expect("README.md exists in the root of the repo")
        .join("README.md");
    t.case("tests/cli-ui.toml").case(readme_path);
}
