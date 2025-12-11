# ver-shim-build

This is a companion crate to [`ver-shim`](https://crates.io/crates/ver-shim).

`ver-shim-build` is a utility for `build.rs` scripts to generate link section content
matching what `ver-shim` expects at runtime. It can collect git information (SHA, branch,
commit timestamp, etc.) and build timestamps, then either:

- Write the section data to a file (for use with `cargo objcopy`), or
- Use `objcopy` directly to patch it into a binary (for artifact dependency workflows)

See the main [`ver-shim` documentation](https://github.com/cbeck88/ver-shim-rs) for full usage instructions.
