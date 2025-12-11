//! Update section command for patching artifact dependency binaries.

use std::fs;
use std::path::Path;
use std::process::Command;

use ver_shim::SECTION_NAME;

use crate::cargo_helpers::{self, cargo_directive};
use crate::find_objcopy;
use crate::LinkSection;

/// Builder for updating sections in an artifact dependency binary.
///
/// Created by calling `LinkSection::patch_into()`.
#[must_use]
pub struct UpdateSectionCommand {
    pub(crate) link_section: LinkSection,
    pub(crate) dep_name: String,
    pub(crate) bin_name: String,
    pub(crate) new_name: Option<String>,
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
        let out_dir = cargo_helpers::out_dir();
        let section_file = self.link_section.write_section_to_dir(&out_dir);

        let bin_path = cargo_helpers::find_artifact_binary(&self.dep_name, &self.bin_name);
        eprintln!("ver-shim-build: artifact binary = {}", bin_path.display());

        // Emit rerun-if-changed for the artifact binary
        // See: https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed
        cargo_directive(&format!("cargo::rerun-if-changed={}", bin_path.display()));

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
        let target_dir = cargo_helpers::target_profile_dir();
        self.write_to_dir(target_dir);
    }
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
