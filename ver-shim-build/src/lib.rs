//! Build script helper for injecting version data into binaries.
//!
//! This crate provides utilities for use in `build.rs` scripts to inject
//! git version information into artifact dependency binaries:
//!
//! - Git SHA (`git rev-parse HEAD`)
//! - Git describe (`git describe --always --dirty`)
//! - Git branch (`git rev-parse --abbrev-ref HEAD`)
//!
//! # Requirements
//!
//! This crate requires Cargo's unstable [artifact dependencies] feature (bindeps).
//! You must use nightly Cargo and enable it in `.cargo/config.toml`:
//!
//! ```toml
//! [unstable]
//! bindeps = true
//! ```
//!
//! [artifact dependencies]: https://doc.rust-lang.org/cargo/reference/unstable.html#artifact-dependencies
//!
//! # Example
//!
//! In your `build.rs`:
//! ```ignore
//! use ver_shim_build::LinkSection;
//!
//! fn main() {
//!     // Include all git info and write to target/debug/my-bin.bin
//!     LinkSection::new()
//!         .with_all_git()
//!         .patch_into("my-dep", "my-bin")
//!         .write_to_target_profile_dir();
//!
//!     // Or include only specific git info
//!     LinkSection::new()
//!         .with_git_describe()
//!         .with_git_branch()
//!         .patch_into("my-dep", "my-bin")
//!         .write_to_target_profile_dir();
//!
//!     // Or with a custom output name
//!     LinkSection::new()
//!         .with_all_git()
//!         .patch_into("my-dep", "my-bin")
//!         .with_new_name("my-custom-name")
//!         .write_to_target_profile_dir();
//!
//!     // Or just write the section data file (for use with cargo-objcopy)
//!     LinkSection::new()
//!         .with_all_git()
//!         .write_to_out_dir();
//! }
//! ```

/// Helper to find llvm objcopy, based on code in cargo-binutils.
pub mod find_objcopy;

use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use heck::ToShoutySnakeCase;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use ver_shim::{BUFFER_SIZE, HEADER_SIZE, Member, NUM_MEMBERS, SECTION_NAME};

/// Builder for configuring which git information to include in version sections.
///
/// Use this to select which git info to collect, then either:
/// - Call `write_to()` or `write_to_out_dir()` to just write the section data file
/// - Call `patch_into()` to get an `UpdateSectionCommand` for patching a binary
#[derive(Default)]
#[must_use]
pub struct LinkSection {
    include_git_sha: bool,
    include_git_describe: bool,
    include_git_branch: bool,
    include_git_commit_timestamp: bool,
    include_git_commit_date: bool,
    include_git_commit_msg: bool,
    include_build_timestamp: bool,
    include_build_date: bool,
    fail_on_error: bool,
    custom: Option<String>,
}

impl LinkSection {
    /// Creates a new empty `LinkSection`
    pub fn new() -> Self {
        Self::default()
    }

    /// Includes the git SHA (`git rev-parse HEAD`) in the section data.
    pub fn with_git_sha(mut self) -> Self {
        self.include_git_sha = true;
        self
    }

    /// Includes the git describe output (`git describe --always --dirty`) in the section data.
    pub fn with_git_describe(mut self) -> Self {
        self.include_git_describe = true;
        self
    }

    /// Includes the git branch name (`git rev-parse --abbrev-ref HEAD`) in the section data.
    pub fn with_git_branch(mut self) -> Self {
        self.include_git_branch = true;
        self
    }

    /// Includes the git commit timestamp (RFC 3339 format) in the section data.
    pub fn with_git_commit_timestamp(mut self) -> Self {
        self.include_git_commit_timestamp = true;
        self
    }

    /// Includes the git commit date (YYYY-MM-DD format) in the section data.
    pub fn with_git_commit_date(mut self) -> Self {
        self.include_git_commit_date = true;
        self
    }

    /// Includes the git commit message (first line, max 100 chars) in the section data.
    pub fn with_git_commit_msg(mut self) -> Self {
        self.include_git_commit_msg = true;
        self
    }

    /// Includes all git information in the section data.
    pub fn with_all_git(mut self) -> Self {
        self.include_git_sha = true;
        self.include_git_describe = true;
        self.include_git_branch = true;
        self.include_git_commit_timestamp = true;
        self.include_git_commit_date = true;
        self.include_git_commit_msg = true;
        self
    }

    /// Includes the build timestamp (RFC 3339 format, UTC) in the section data.
    pub fn with_build_timestamp(mut self) -> Self {
        self.include_build_timestamp = true;
        self
    }

    /// Includes the build date (YYYY-MM-DD format, UTC) in the section data.
    pub fn with_build_date(mut self) -> Self {
        self.include_build_date = true;
        self
    }

    /// Includes all build time information (timestamp and date) in the section data.
    pub fn with_all_build_time(mut self) -> Self {
        self.include_build_timestamp = true;
        self.include_build_date = true;
        self
    }

    /// Enables fail-on-error mode.
    ///
    /// By default, if git commands fail (e.g., `git` not found, not in a git repository,
    /// building from a source tarball without `.git`), a `cargo:warning` is emitted and
    /// the corresponding data is skipped. This allows builds to succeed even without git.
    ///
    /// When `fail_on_error()` is called, git failures will instead cause a panic,
    /// failing the build.
    pub fn fail_on_error(mut self) -> Self {
        self.fail_on_error = true;
        self
    }

    /// Sets a custom application-specific string to embed in the binary.
    ///
    /// This can be any string your application wants to store. The total size of all
    /// data (including git info, timestamps, and custom string) must fit within the
    /// buffer size (default 512 bytes). If you need more space, set the
    /// `VER_SHIM_BUFFER_SIZE` environment variable when building.
    ///
    /// As with any build script, you must emit `cargo:rerun-if-...` directives as
    /// needed if you read files or environment variables to build your custom string.
    ///
    /// Access this at runtime with `ver_shim::custom()`.
    pub fn with_custom(mut self, s: impl Into<String>) -> Self {
        self.custom = Some(s.into());
        self
    }

    /// Writes the section data file to the specified directory.
    ///
    /// This is useful for `cargo objcopy` workflows where you want to manually
    /// run objcopy with the generated section file.
    ///
    /// Returns the path to the written file.
    pub fn write_to(self, dir: impl AsRef<Path>) -> PathBuf {
        self.write_section_to_dir(dir.as_ref())
    }

    /// Writes the section data file to `OUT_DIR`.
    ///
    /// This is a convenience method for use in build scripts.
    ///
    /// Returns the path to the written file.
    pub fn write_to_out_dir(self) -> PathBuf {
        let out_dir = get_out_dir();
        self.write_section_to_dir(&out_dir)
    }

    /// Writes the section data file to the `target/` directory.
    /// Returns the path to the written file (e.g., `target/ver_shim_data`).
    ///
    /// This is useful for `cargo objcopy` workflows where you want to run:
    /// ```bash
    /// cargo objcopy --release --bin my_bin -- --update-section .ver_shim_data=target/ver_shim_data my_bin.bin
    /// ```
    ///
    /// The target directory is determined by checking `CARGO_TARGET_DIR` first,
    /// then inferring from `OUT_DIR`. The result should typically be `target/ver_shim_data`.
    ///
    /// When cross-compiling, it might end up in `target/<triple>/ver_shim_data`, due to
    /// how the inference works.
    ///
    /// To adjust this, you can set `CARGO_TARGET_DIR` in `.cargo/config.toml`:
    /// ```toml
    /// [env]
    /// CARGO_TARGET_DIR = { value = "target", relative = true }
    /// ```
    pub fn write_to_target_dir(self) -> PathBuf {
        let target_dir = get_target();
        self.write_section_to_dir(&target_dir)
    }

    /// Transitions to an `UpdateSectionCommand` for patching an artifact dependency binary.
    ///
    /// # Arguments
    /// * `dep_name` - The name of the dependency as specified in Cargo.toml
    /// * `bin_name` - The name of the binary within the dependency
    pub fn patch_into(self, dep_name: &str, bin_name: &str) -> UpdateSectionCommand {
        UpdateSectionCommand {
            link_section: self,
            dep_name: dep_name.to_string(),
            bin_name: bin_name.to_string(),
            new_name: None,
        }
    }

    fn any_git_enabled(&self) -> bool {
        self.include_git_sha
            || self.include_git_describe
            || self.include_git_branch
            || self.include_git_commit_timestamp
            || self.include_git_commit_date
            || self.include_git_commit_msg
    }

    fn any_build_time_enabled(&self) -> bool {
        self.include_build_timestamp || self.include_build_date
    }

    fn check_enabled(&self) {
        if !self.any_git_enabled() && !self.any_build_time_enabled() && self.custom.is_none() {
            panic!(
                "ver-shim-build: no version info enabled. Call with_git_sha(), with_git_describe(), \
                 with_git_branch(), with_git_commit_timestamp(), with_git_commit_date(), \
                 with_git_commit_msg(), with_all_git(), with_build_timestamp(), with_build_date(), \
                 or with_custom() before writing."
            );
        }
    }

    fn write_section_to_dir(&self, dir: &Path) -> PathBuf {
        self.check_enabled();

        // Emit rerun-if-changed directives for git state (only if git data requested)
        if self.any_git_enabled() {
            emit_git_rerun_if_changed();
        }

        // Collect the data for each member
        let mut member_data: [Option<String>; NUM_MEMBERS] = Default::default();

        if self.include_git_sha
            && let Some(git_sha) = get_git_sha(self.fail_on_error)
        {
            eprintln!("ver-shim-build: git SHA = {}", git_sha);
            member_data[Member::GitSha as usize] = Some(git_sha);
        }

        if self.include_git_describe
            && let Some(git_describe) = get_git_describe(self.fail_on_error)
        {
            eprintln!("ver-shim-build: git describe = {}", git_describe);
            member_data[Member::GitDescribe as usize] = Some(git_describe);
        }

        if self.include_git_branch
            && let Some(git_branch) = get_git_branch(self.fail_on_error)
        {
            eprintln!("ver-shim-build: git branch = {}", git_branch);
            member_data[Member::GitBranch as usize] = Some(git_branch);
        }

        if (self.include_git_commit_timestamp || self.include_git_commit_date)
            && let Some(timestamp) = get_git_commit_timestamp(self.fail_on_error)
        {
            if self.include_git_commit_timestamp {
                let rfc3339 = timestamp.to_rfc3339();
                eprintln!("ver-shim-build: git commit timestamp = {}", rfc3339);
                member_data[Member::GitCommitTimestamp as usize] = Some(rfc3339);
            }
            if self.include_git_commit_date {
                let date = timestamp.date_naive().to_string();
                eprintln!("ver-shim-build: git commit date = {}", date);
                member_data[Member::GitCommitDate as usize] = Some(date);
            }
        }

        if self.include_git_commit_msg
            && let Some(msg) = get_git_commit_msg(self.fail_on_error)
        {
            eprintln!("ver-shim-build: git commit msg = {}", msg);
            member_data[Member::GitCommitMsg as usize] = Some(msg);
        }

        if self.any_build_time_enabled() {
            // Emit rerun-if-env-changed for reproducible build time override
            println!("cargo::rerun-if-env-changed=VER_SHIM_BUILD_TIME");
            let build_time = get_build_time();
            if self.include_build_timestamp {
                let rfc3339 = build_time.to_rfc3339();
                eprintln!("ver-shim-build: build timestamp = {}", rfc3339);
                member_data[Member::BuildTimestamp as usize] = Some(rfc3339);
            }
            if self.include_build_date {
                let date = build_time.date_naive().to_string();
                eprintln!("ver-shim-build: build date = {}", date);
                member_data[Member::BuildDate as usize] = Some(date);
            }
        }

        if let Some(ref custom) = self.custom {
            eprintln!("ver-shim-build: custom = {}", custom);
            member_data[Member::Custom as usize] = Some(custom.clone());
        }

        // Build the section buffer
        let buffer = build_section_buffer(&member_data);

        // Write to file
        let section_file = dir.join("ver_shim_data");
        fs::write(&section_file, &buffer).expect("ver-shim-build: failed to write section file");

        section_file
    }
}

/// Builds the section buffer from member data.
///
/// Format:
/// - First `NUM_MEMBERS * 2` bytes: header with end offsets (u16, little-endian, relative to HEADER_SIZE)
/// - Remaining bytes: concatenated string data
///
/// For member N:
/// - start = HEADER_SIZE + end[N-1] if N > 0, else HEADER_SIZE
/// - end = HEADER_SIZE + end[N]
/// - If start == end, the member is not present.
///
/// Using relative offsets means a zero-initialized buffer reads as "all members absent".
fn build_section_buffer(member_data: &[Option<String>; NUM_MEMBERS]) -> Vec<u8> {
    let mut buffer = vec![0u8; BUFFER_SIZE];

    // Data starts after the header; track position relative to HEADER_SIZE
    let mut relative_offset: usize = 0;

    for (idx, data) in member_data.iter().enumerate() {
        if let Some(s) = data {
            let bytes = s.as_bytes();
            let absolute_start = HEADER_SIZE + relative_offset;
            let absolute_end = absolute_start + bytes.len();

            if absolute_end > BUFFER_SIZE {
                panic!(
                    "ver-shim-build: section data too large ({} bytes, max {}). \
                     Set VER_SHIM_BUFFER_SIZE env var to increase the buffer size.",
                    absolute_end, BUFFER_SIZE
                );
            }

            // Write the data
            buffer[absolute_start..absolute_end].copy_from_slice(bytes);

            relative_offset += bytes.len();
        }

        // Write the end offset for this member (relative to HEADER_SIZE)
        // If member is not present, end == previous end, so start == end indicates "not present"
        let header_offset = idx * 2;
        buffer[header_offset..header_offset + 2]
            .copy_from_slice(&(relative_offset as u16).to_le_bytes());
    }

    buffer
}

/// Builder for updating sections in an artifact dependency binary.
///
/// Created by calling `LinkSection::patch_into()`.
#[must_use]
pub struct UpdateSectionCommand {
    link_section: LinkSection,
    dep_name: String,
    bin_name: String,
    new_name: Option<String>,
}

impl UpdateSectionCommand {
    /// Sets a custom filename for the output binary.
    ///
    /// If not called, the default name is `{bin_name}.bin`.
    pub fn with_new_name(mut self, name: &str) -> Self {
        self.new_name = Some(name.to_string());
        self
    }

    /// Writes the patched binary to the specified directory.
    ///
    /// The binary is first written to `OUT_DIR`, then copied to the specified directory.
    pub fn write_to_dir(self, dir: impl AsRef<Path>) {
        let out_dir = get_out_dir();
        let section_file = self.link_section.write_section_to_dir(&out_dir);

        let bin_path = find_artifact_binary(&self.dep_name, &self.bin_name);
        eprintln!("ver-shim-build: artifact binary = {}", bin_path.display());

        // Emit rerun-if-changed for the artifact binary
        // See: https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed
        println!("cargo::rerun-if-changed={}", bin_path.display());

        // Determine output filename (default to {bin_name}.bin to avoid collisions with cargo)
        let default_name = format!("{}.bin", self.bin_name);
        let output_name = self.new_name.as_deref().unwrap_or(&default_name);

        // Write patched binary to OUT_DIR first
        let out_dir_binary = out_dir.join(output_name);
        run_objcopy(&bin_path, &out_dir_binary, SECTION_NAME, &section_file);
        eprintln!(
            "ver-shim-build: wrote version data to {}",
            out_dir_binary.display()
        );

        // Copy to the specified directory
        let target_binary = dir.as_ref().join(output_name);
        fs::copy(&out_dir_binary, &target_binary).unwrap_or_else(|e| {
            panic!(
                "ver-shim-build: failed to copy {} to {}: {}",
                out_dir_binary.display(),
                target_binary.display(),
                e
            )
        });
        eprintln!("ver-shim-build: copied to {}", target_binary.display());
    }

    /// Writes the patched binary to the target profile directory (e.g., `target/debug/`).
    ///
    /// NOTE: Copying things to target dir is not expressly supported by cargo devs.
    /// If you clobber a binary that cargo generates, it may trigger unnecessary rebuilds later.
    /// However, it typically works fine.
    ///
    /// See also:
    /// - <https://github.com/rust-lang/cargo/issues/9661#issuecomment-1769481293>
    /// - <https://github.com/rust-lang/cargo/issues/9661#issuecomment-2159267601>
    /// - <https://github.com/rust-lang/cargo/issues/13663>
    pub fn write_to_target_profile_dir(self) {
        let target_dir = get_target_profile_dir();
        self.write_to_dir(target_dir);
    }
}

/// Gets the target profile directory (e.g., `target/debug/` or `target/release/`).
///
/// Derives this from OUT_DIR which is like `target/debug/build/<pkg>/out`.
/// For cross-compilation, it's `target/<triple>/debug/build/<pkg>/out`.
fn get_target_profile_dir() -> PathBuf {
    let out_dir = get_out_dir();
    // OUT_DIR is target/[<triple>/]debug/build/<pkg>/out, go up 3 levels to get target/[<triple>/]debug
    out_dir
        .ancestors()
        .nth(3)
        .expect("ver-shim-build: could not find target dir from OUT_DIR")
        .to_path_buf()
}

// ============================================================================
// Helper functions
// ============================================================================

/// Gets OUT_DIR from environment.
fn get_out_dir() -> PathBuf {
    // OUT_DIR is set by Cargo for build scripts to write generated files.
    // See: https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set - must be run from build.rs");
    PathBuf::from(out_dir)
}

/// Gets the target directory (e.g., `target/`).
///
/// Checks CARGO_TARGET_DIR first, then tries to infer it from the value of
/// OUT_DIR. If this inference fails, users can set CARGO_TARGET_DIR in
/// `.cargo/config.toml`:
/// ```toml
/// [env]
/// CARGO_TARGET_DIR = { value = "target", relative = true }
/// ```
fn get_target() -> PathBuf {
    // Check CARGO_TARGET_DIR first (user override)
    if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(target_dir);
    }

    // Infer from OUT_DIR (target/debug/build/<pkg>/out -> go up 4 levels)
    let out_dir = get_out_dir();
    out_dir
        .ancestors()
        .nth(4)
        .expect(
            "ver-shim-build: could not find target dir from OUT_DIR. \
             Set CARGO_TARGET_DIR in .cargo/config.toml:\n\n\
             [env]\n\
             CARGO_TARGET_DIR = { value = \"target\", relative = true }",
        )
        .to_path_buf()
}

/// Emits cargo rerun-if-changed directives for git state files.
/// This ensures the build script reruns when the git HEAD or refs change.
/// Matches vergen's behavior: watches .git/HEAD and .git/<ref_path>.
///
/// See: https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed
fn emit_git_rerun_if_changed() {
    // Find the git directory
    let git_dir = match find_git_dir() {
        Some(dir) => dir,
        None => return,
    };

    // Always watch .git/HEAD
    let head_path = git_dir.join("HEAD");
    if head_path.exists() {
        println!("cargo::rerun-if-changed={}", head_path.display());

        // If HEAD points to a ref, also watch that ref file
        if let Ok(head_contents) = fs::read_to_string(&head_path) {
            let head_contents = head_contents.trim();
            if let Some(ref_path) = head_contents.strip_prefix("ref: ") {
                let ref_file = git_dir.join(ref_path);
                if ref_file.exists() {
                    println!("cargo::rerun-if-changed={}", ref_file.display());
                }
            }
        }
    }
}

/// Finds the .git directory by walking up from the current directory.
fn find_git_dir() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let git_dir = dir.join(".git");
        if git_dir.is_dir() {
            return Some(git_dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Gets the current git SHA using `git rev-parse HEAD`.
fn get_git_sha(fail_on_error: bool) -> Option<String> {
    run_git_command(&["rev-parse", "HEAD"], fail_on_error)
}

/// Gets the git describe output using `git describe --always --dirty`.
fn get_git_describe(fail_on_error: bool) -> Option<String> {
    run_git_command(&["describe", "--always", "--dirty"], fail_on_error)
}

/// Gets the current git branch using `git rev-parse --abbrev-ref HEAD`.
fn get_git_branch(fail_on_error: bool) -> Option<String> {
    run_git_command(&["rev-parse", "--abbrev-ref", "HEAD"], fail_on_error)
}

/// Gets the git commit timestamp as a chrono DateTime.
fn get_git_commit_timestamp(fail_on_error: bool) -> Option<DateTime<FixedOffset>> {
    // Get the author date in ISO 8601 strict format
    let timestamp_str = run_git_command(&["log", "-1", "--format=%aI"], fail_on_error)?;
    match DateTime::parse_from_rfc3339(&timestamp_str) {
        Ok(dt) => Some(dt),
        Err(e) => {
            let msg = format!(
                "ver-shim-build: failed to parse git timestamp '{}': {}",
                timestamp_str, e
            );
            if fail_on_error {
                panic!("{}", msg);
            } else {
                println!("cargo:warning={}", msg);
                None
            }
        }
    }
}

/// Gets the first line of the git commit message, truncated to 100 chars.
fn get_git_commit_msg(fail_on_error: bool) -> Option<String> {
    let msg = run_git_command(&["log", "-1", "--format=%s"], fail_on_error)?;
    // Truncate to 100 chars to leave room in the buffer
    Some(if msg.len() > 100 {
        let mut end = 100;
        while !msg.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        msg[..end].to_string()
    } else {
        msg
    })
}

/// Gets the build time, either from VER_SHIM_BUILD_TIME env var or Utc::now().
///
/// If VER_SHIM_BUILD_TIME is set, it tries to parse it as:
/// 1. An integer (unix timestamp in seconds)
/// 2. An RFC 3339 datetime string
///
/// This supports reproducible builds by allowing a fixed build time.
fn get_build_time() -> DateTime<Utc> {
    if let Ok(val) = std::env::var("VER_SHIM_BUILD_TIME") {
        // Try parsing as unix timestamp (integer) first
        if let Ok(ts) = val.parse::<i64>() {
            let dt = Utc.timestamp_opt(ts, 0).single().unwrap_or_else(|| {
                panic!(
                    "ver-shim-build: VER_SHIM_BUILD_TIME '{}' is not a valid unix timestamp",
                    val
                )
            });
            eprintln!(
                "ver-shim-build: using VER_SHIM_BUILD_TIME={} (unix timestamp), overriding Utc::now()",
                val
            );
            return dt;
        }

        // Try parsing as RFC 3339
        if let Ok(dt) = DateTime::parse_from_rfc3339(&val) {
            eprintln!(
                "ver-shim-build: using VER_SHIM_BUILD_TIME={} (RFC 3339), overriding Utc::now()",
                val
            );
            return dt.with_timezone(&Utc);
        }

        panic!(
            "ver-shim-build: VER_SHIM_BUILD_TIME '{}' is not a valid unix timestamp or RFC 3339 datetime",
            val
        );
    }

    Utc::now()
}

/// Runs a git command and returns stdout as a trimmed string.
///
/// If `fail_on_error` is true, panics on failure. Otherwise, emits a cargo warning
/// and returns None, allowing builds to succeed without git.
fn run_git_command(args: &[&str], fail_on_error: bool) -> Option<String> {
    let cmd = format!("git {}", args.join(" "));
    let output = match Command::new("git").args(args).output() {
        Ok(output) => output,
        Err(e) => {
            let msg = format!("ver-shim-build: failed to execute '{}': {}", cmd, e);
            if fail_on_error {
                panic!("{}", msg);
            } else {
                println!("cargo:warning={}", msg);
                return None;
            }
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = format!(
            "ver-shim-build: '{}' failed with status {}: {}",
            cmd,
            output.status,
            stderr.trim()
        );
        if fail_on_error {
            panic!("{}", msg);
        } else {
            println!("cargo:warning={}", msg);
            return None;
        }
    }

    match String::from_utf8(output.stdout) {
        Ok(s) => Some(s.trim().to_string()),
        Err(_) => {
            let msg = format!("ver-shim-build: '{}' output is not valid UTF-8", cmd);
            if fail_on_error {
                panic!("{}", msg);
            } else {
                println!("cargo:warning={}", msg);
                None
            }
        }
    }
}

/// Finds the artifact binary path using Cargo's environment variables.
///
/// Artifact dependencies expose binaries via environment variables like
/// `CARGO_BIN_FILE_<DEP>_<NAME>` and `CARGO_BIN_DIR_<DEP>`.
/// See: https://doc.rust-lang.org/cargo/reference/unstable.html#artifact-dependencies
fn find_artifact_binary(dep_name: &str, bin_name: &str) -> PathBuf {
    // Convert dep name to SHOUTY_SNAKE_CASE for env var lookup.
    // Cargo converts dependency names to uppercase with dashes replaced by underscores.
    let dep_upper = dep_name.to_shouty_snake_case();

    // Try CARGO_BIN_FILE_<DEP>_<NAME> with original bin name case first
    // (cargo uses original case for bin name, not upper case)
    let file_env_var_original = format!("CARGO_BIN_FILE_{}_{}", dep_upper, bin_name);
    if let Ok(path) = std::env::var(&file_env_var_original) {
        let path = PathBuf::from(path);
        if path.exists() {
            return path;
        }
        panic!(
            "ver-shim-build: {} is set to '{}' but file does not exist",
            file_env_var_original,
            path.display()
        );
    }

    // Try CARGO_BIN_FILE_<DEP> (default binary, no name suffix)
    let file_env_var_default = format!("CARGO_BIN_FILE_{}", dep_upper);
    if let Ok(path) = std::env::var(&file_env_var_default) {
        let path = PathBuf::from(path);
        if path.exists() {
            return path;
        }
        panic!(
            "ver-shim-build: {} is set to '{}' but file does not exist",
            file_env_var_default,
            path.display()
        );
    }

    // Try CARGO_BIN_DIR_<DEP> and search for the binary
    let dir_env_var = format!("CARGO_BIN_DIR_{}", dep_upper);
    if let Ok(dir) = std::env::var(&dir_env_var) {
        let dir_path = PathBuf::from(&dir);
        // The binary might have a hash suffix, so look for any file starting with the bin name
        if let Ok(entries) = fs::read_dir(&dir_path) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                // Match bin_name with underscores (cargo converts - to _)
                let bin_name_underscore = bin_name.replace('-', "_");
                if file_name_str.starts_with(&bin_name_underscore) {
                    return entry.path();
                }
            }
        }
        panic!(
            "ver-shim-build: {} is set to '{}' but no binary matching '{}' found in that directory",
            dir_env_var, dir, bin_name
        );
    }

    // No env var found
    panic!(
        "ver-shim-build: could not find artifact binary for dep='{}', bin='{}'\n\
         Expected one of:\n\
         - {} (not set)\n\
         - {} (not set)\n\
         - {} (not set)\n\
         \n\
         Make sure you have an artifact dependency in Cargo.toml:\n\
         [build-dependencies]\n\
         {} = {{ path = \"...\", artifact = \"bin\" }}",
        dep_name, bin_name, file_env_var_original, file_env_var_default, dir_env_var, dep_name
    );
}

/// Runs objcopy to update the section in the binary.
fn run_objcopy(input: &Path, output: &Path, section_name: &str, section_file: &Path) {
    let objcopy_path = find_objcopy::find().unwrap_or_else(|e| {
        panic!(
            "ver-shim-build: could not find llvm-objcopy: {}\n\
             Please install llvm-tools: rustup component add llvm-tools",
            e
        )
    });

    let update_arg = format!("{}={}", section_name, section_file.display());

    let status = Command::new(&objcopy_path)
        .arg("--update-section")
        .arg(&update_arg)
        .arg(input)
        .arg(output)
        .status()
        .unwrap_or_else(|e| {
            panic!(
                "ver-shim-build: failed to execute objcopy at '{}': {}",
                objcopy_path.display(),
                e
            )
        });

    if !status.success() {
        panic!("ver-shim-build: objcopy failed with status {}", status);
    }
}
