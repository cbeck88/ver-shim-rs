# ver-shim

`ver-shim` is a library for injecting build-time information (git hashes, timestamps, etc.)
into the binary *without* injecting code, or triggering frequent cargo rebuilds.

[![Crates.io](https://img.shields.io/crates/v/ver-shim?style=flat-square)](https://crates.io/crates/ver-shim)
[![Crates.io](https://img.shields.io/crates/d/ver-shim?style=flat-square)](https://crates.io/crates/ver-shim)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](LICENSE-APACHE)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE-MIT)
[![Build Status](https://img.shields.io/github/actions/workflow/status/cbeck88/ver-shim-rs/ci.yml?branch=master&style=flat-square)](https://github.com/cbeck88/ver-shim-rs/actions/workflows/ci.yml?query=branch%3Amaster)

[API Docs](https://docs.rs/ver-shim/latest/ver_shim/)

This is particularly helpful if:

* You have multiple binaries in your workspace and rebuilding them all is slow
* You are using build options like LTO which push a lot of work to link time

When I used the popular `vergen` crate to embed this data, I often found myself frustrated
because actions like `git commit`, `git tag` or `git checkout -b` would cause the next `cargo build`
to rebuild many things, but that would cause momentary confusion and make
me think that I'd accidentally changed code and committed or tagged the wrong thing.

See also the ["relink don't rebuild"](https://rust-lang.github.io/rust-project-goals/2025h2/relink-dont-rebuild.html)
project goal.

## How does it work?

`ver-shim` declares a linker section called `.ver_shim_data` with a specific size in bytes.
This is filled in with requested version data only at the end of the build process, after all the
time consuming steps are done. If the version data (git, timestamps) changes, the binary doesn't
have to be recompiled -- this section just needs to be overwritten again.

If it is never filled in, then the section is all 0s, and at
runtime your program safely reports that it doesn't have build information available and otherwise works correctly.

## Quickstart

Use `ver_shim` anywhere in your project, and call its functions

```rust
fn git_sha() -> Option<&'static str>;
fn git_describe() -> Option<&'static str>;
fn git_branch() -> Option<&'static str>;
fn git_commit_timestamp() -> Option<&'static str>;
fn git_commit_date() -> Option<&'static str>;
fn git_commit_msg() -> Option<&'static str>;
fn build_timestamp() -> Option<&'static str>;
fn build_date() -> Option<&'static str>;
fn custom() -> Option<&'static str>;
```

This crate doesn't change when the git data changes, so depending on it doesn't trigger any rebuilds.

Then, use the `ver-shim-build` crate to fill in the linker section.

There are basically two recommendable approaches.

### Approach #1: `build.rs` + special build command

In `build.rs` for your binary crate:

```rust
fn main() {
    ver_shim_build::LinkSection::new()
        .with_all_git()
        .write_to_target_dir();
}
```

To build a release artifact:

```sh
cargo objcopy --release --bin my_bin -- --update-section .ver_shim_data=target/ver_shim_data target/release/my_bin.bin
```

For ergonomics, put this in:

* A justfile
* The `[alias]` table of [`.cargo/config.toml`](https://doc.rust-lang.org/cargo/reference/config.html#alias)
* A pre-existing release script.

This command uses cargo to build `my_bin` normally in release mode, and then use the `objcopy` tool to patch the link section with bytes from `target/ver_shim_data`, and produce the patched output at `target/release/my_bin.bin`.

(If you store the patched output in `target/release`, it's better to use a modified name, such as with `.bin`, because if `target/release/my_bin` changes, that will cause unnecessary cargo rebuilds later. If you store the patched output somewhere else then there's less reason to change the name.)

For this to work, you must:

* `cargo install cargo-binutils`
* `rustup component add llvm-tools`

This is quite portable in that `cargo-binutils` uses the same llvm tools that `rustc` itself
was built with.

`cargo objcopy` respects other flags of `cargo build`, like turning features on or off, and `cargo-binutils` is generally well maintained.

### Approach #2: Use a post-build crate and artifact dependencies

Create a new crate in the same workspace, with a `build.rs` and an empty `lib.rs`.

It should declare a *build dependency* on your binary crate, with an artifact dependency on the bin.

```toml
my-crate = { path = "../my-crate", artifact = "bin" }
```

The `build.rs` should look something like this:

```rust
use ver_shim_build::LinkSection;

fn main() {
    LinkSection::new()
        .with_all_git()
        .patch_into("my-crate", "bin_name")
        .write_to_target_profile_dir();
}
```

When cargo runs this `build.rs`, it runs essentially the same `objcopy` command to patch the linker section,
and produce another binary, by default `bin_name.bin`, in `target/release` or `target/debug` according to the build profile.
This `build.rs` only runs when its input (the unpatched binary) changes, or when the git information changes.

Artifact dependencies are an unstable feature of cargo, so you will have to use nightly for this approach to work.

### Summary

The two approaches are illustrated by examples in this repo.

* [`ver-shim-example-objcopy`](./ver-shim-example-objcopy)
  * Toolchain: **stable**
  * Extra crate: **no**
  * Command: `cargo objcopy --bin ver-shim-example-objcopy -- --update-section .ver_shim_data=target/ver_shim_data  target/debug/ver-shim-example-objcopy.bin`
* [`ver-shim-example-build`](./ver-shim-example-build)
  * Toolchain: **nightly**
  * Extra crate: **yes**
  * Command: `cargo +nightly build` (auto-patches to `target/debug/ver-shim-example.bin`)

There are other patterns worth mentioning, such as [`cargo xtask`](https://github.com/matklad/cargo-xtask).
However, that's a generalization of the `cargo objcopy` approach, where instead of using the command from `cargo-binutils`,
you roll your own cargo command and use it to make cargo do whatever you want.

## Reproducible builds

Reproducible builds is the idea that, if you publish an open source project, and binary distributions of it, you should ensure that
it is possible for someone else to confirm that the build is "good" and wasn't maliciously tampered with.

Lots of projects publish a binary you can download, and a hash of it, so that you can confirm the download wasn't corrupted.
However, this doesn't rule out the possibility that the person who built and hashed the binary was compromised.

Reproducible builds demands something further -- if I check out your repo on my machine, and I run your release build command, I should
get a byte-for-byte identical binary, and compute the same hash as you did.

For example, some security-conscious projects like Signal or Tor work to ensure that their builds are reproducible. Even if the
code in the open-source repo is good, a malicious actor could tamper with the binary sometime before or after it gets into an app repository / App Store,
and then the users would be compromised. Reproducible builds empower *users* to detect this discrepancy without even having to trust
Signal or Tor themselves -- the users can be sure on their own exactly what code they are running. This also helps to dissuade "wrench attacks"
against Signal or Tor developers, which an attacker might otherwise conduct in order to try to force the developers to release compromised code,
in the hopes that it would go undetected and allow them to compromise specific users. A similar analysis applies to e.g. Debian package maintainers.

This touches on things like `vergen` and `ver-shim` because injecting a build timestamp into the binary makes it not reproducible -- the current time
will be different if you build again later, so the hashes won't match.

`ver-shim` respects an env var `VER_SHIM_BUILD_TIME`, which can be set to a unix timestamp or an RFC3339 date time. If set, it uses this time as the build
time rather than the actual current time. You can make setting this part of your release process and published the value used with each release, so that
you can have build times in your binary (convenient) while still enabling outsiders to reproduce the build.

This is similar to `SOURCE_DATE_EPOCH` in `vergen`.
However, one thing I like about the `ver-shim` approach is that it also helps with the task of debugging non-reproducible builds.

In a large project it can be very complicated to figure out why two engineers got a different binary at the same commit. I once traced this down to the
[`ahash/const-random` feature](https://docs.rs/crate/ahash/latest/features#const-random), which was intentionally injecting random numbers into the build,
and being enabled transitively by a dependency.

When using `ver-shim`, you can easily dump the `.ver_shim_data` sections from the two binaries and compare them, or, zero them both out and then compute hashes.
If there are still differences, you have working binaries that you can use with other tools from that point.

## Additional configuration

The size of the section created by `ver-shim` is configurable and defaults to 512 bytes. It can be changed by setting `VER_SHIM_BUFFER_SIZE` while building `ver-shim`.
It must be larger than 32 bytes and no more than 64KB.

## Misc Notes

### multiple copies

It is important for the correctness of the crate that only one version of `ver-shim` is used at a time. Otherwise the custom section will have two copies
of the buffer, and only one of them actually gets written by objcopy. To force this to be the case, the `links` attribute is used with `ver-shim`, with the
name of the custom linker section.

(Note that `llvm-objcopy` also has some protections, and [won't allow a section to be enlarged via `--update-section`](https://reviews.llvm.org/D112116).)

### zero copies

It's possible that the binary ends up with 0 copies of the linker section. This happens if you depend on `ver-shim` but then don't actually invoke any of its functions.
If nothing in the program, after optimizations, references the linker section, it will likely be garbage collected and removed by the linker. This would be fine except
that the `objcopy --update-section` command will fail if the section doesn't exist when `objcopy` runs.

The simplest fixes for this case are probably:

* Actually use the `ver-shim` data. Add a `--version` flag to your program or something, which can actually be invoked and won't be optimized away.
* Make your dependency on `ver-shim` optional and don't enable it if you won't actually use it.
* If you invoke `objcopy` from a `build.rs`, then before you do, check if the section actually exists, and skip the `--update-section` if it doesn't.

### will you support all the data that `vergen` does?

Most likely not.

* The rust toolchain already embeds much of this in the `.comment` section:

  ```
      String dump of section '.comment':
       [     0]  Linker: LLD 21.1.2 (/checkout/src/llvm-project/llvm 8c30b9c5098bdff1d3d9a2d460ee091cd1171e60)
       [    5f]  rustc version 1.91.1 (ed61e7d7e 2025-11-07)
       [    8b]  GCC: (Ubuntu 13.3.0-6ubuntu2~24.04) 13.3.0
  ```

  and information about the ABI appears in `.note`.

  ```
      OS: Linux, ABI: 3.2.0
  ```

  which can be read easily using `readelf -n` and `readelf -p .comment`.

* My main motivation was to avoid the hit to build times that occurs when data that "logically" isn't already a part of the code,
  like git state, build timestamp, is injected into the code, and `cargo` rebuilds everything out of an abundance of caution.

  If your compiler changes, or your opt level changes, or your cargo features change, cargo already has to rebuild, whether or
  not you additionally inject this stuff as text strings into the source. So there's no advantage to the link-section approach
  over what `vergen` is doing. You might as well use `vergen` for the other stuff.

## Licensing and distribution

MIT or Apache 2 at your option
