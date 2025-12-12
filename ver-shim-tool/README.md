# ver-shim-tool

CLI tool for injecting version data into binaries using [`ver-shim`](https://crates.io/crates/ver-shim).

[![Crates.io](https://img.shields.io/crates/v/ver-shim-tool?style=flat-square)](https://crates.io/crates/ver-shim-tool)
[![Crates.io](https://img.shields.io/crates/d/ver-shim-tool?style=flat-square)](https://crates.io/crates/ver-shim-tool)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](LICENSE-APACHE)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE-MIT)

## Installation

```sh
cargo install ver-shim-tool
rustup component add llvm-tools
```

## Example Usage

### Patch a binary directly

```sh
cargo build --release
ver-shim --all-git --build-timestamp patch target/release/my-bin
```

This produces a patched binary at `target/release/my-bin.bin`.

### Generate section data file

For use with `cargo objcopy` or other tools:

```sh
ver-shim --all-git --build-timestamp -o target/ver_shim_data
cargo objcopy --release --bin my-bin -- --update-section .ver_shim_data=target/ver_shim_data my-bin.bin
```

## Options

This tool exposes CLI parameters for the functionality in [`ver-shim-build`](https://crates.io/crates/ver-shim-build).
Run `ver-shim --help` for the full list of options.

## Reproducible Builds

For reproducible builds, two environment variables are supported:

- **`VER_SHIM_IDEMPOTENT`**: If set, build timestamp/date are never included (always None).
  This is the simplest option for fully reproducible builds.

- **`VER_SHIM_BUILD_TIME`**: Override the build timestamp with a fixed value.
  Accepts unix timestamps or RFC 3339 datetimes.

`VER_SHIM_IDEMPOTENT` takes precedence if both are set.

## See Also

- [`ver-shim`](https://crates.io/crates/ver-shim) - Runtime library for reading version data
- [`ver-shim-build`](https://crates.io/crates/ver-shim-build) - Build script helper (used by this tool)
- [Main documentation](https://github.com/cbeck88/ver-shim-rs) - Full usage instructions and examples
