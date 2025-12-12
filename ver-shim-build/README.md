# ver-shim-build

Build script helper for [`ver-shim`](https://crates.io/crates/ver-shim).

[![Crates.io](https://img.shields.io/crates/v/ver-shim-build?style=flat-square)](https://crates.io/crates/ver-shim-build)
[![Crates.io](https://img.shields.io/crates/d/ver-shim-build?style=flat-square)](https://crates.io/crates/ver-shim-build)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](LICENSE-APACHE)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE-MIT)

[API Docs](https://docs.rs/ver-shim-build/latest/ver_shim_build/)

This crate generates link section content matching what `ver-shim` expects at runtime.
It collects git information (SHA, branch, commit timestamp, etc.) and build timestamps,
then writes the data to a file or patches it directly into a binary.

## Example

```rust
// build.rs
fn main() {
    ver_shim_build::LinkSection::new()
        .with_all_git()
        .with_build_timestamp()
        .write_to_out_dir();
}
```

## See Also

- [`ver-shim`](https://crates.io/crates/ver-shim) - Runtime library for reading version data
- [`ver-shim-tool`](https://crates.io/crates/ver-shim-tool) - CLI tool (if you don't need build.rs integration)
- [Main documentation](https://github.com/cbeck88/ver-shim-rs) - Full usage instructions and examples
