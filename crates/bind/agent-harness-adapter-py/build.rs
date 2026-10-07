// Link the extension as Python expects: on macOS with `-undefined
// dynamic_lookup`, so a plain `cargo build` / `cargo test` of the workspace
// links it without libpython (maturin builds the wheels the same way).
fn main() {
    pyo3_build_config::add_extension_module_link_args();
}
