# ver-shim-build

This is a companion crate to [`ver-shim`](https://crates.io/crates/ver-shim).

[![Crates.io](https://img.shields.io/crates/v/ver-shim-build?style=flat-square)](https://crates.io/crates/ver-shim-build)
[![Crates.io](https://img.shields.io/crates/d/ver-shim-build?style=flat-square)](https://crates.io/crates/ver-shim-build)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](LICENSE-APACHE)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE-MIT)

[API Docs](https://docs.rs/ver-shim-build/latest/ver_shim_build/)

`ver-shim-build` is a utility for `build.rs` scripts to generate link section content
matching what `ver-shim` expects at runtime. It can collect git information (SHA, branch,
commit timestamp, etc.) and build timestamps, then either:

- Write the section data to a file (for use with `cargo objcopy`), or
- Use `objcopy` directly to patch it into a binary (for artifact dependency workflows)

See the main [`ver-shim` documentation](https://github.com/cbeck88/ver-shim-rs) for full usage instructions.
