//! Update section command for patching artifact dependency binaries.

use std::env::consts::EXE_SUFFIX;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ver_shim::{BUFFER_SIZE, SECTION_NAME};

use crate::LinkSection;
use crate::cargo_helpers::{self, cargo_rerun_if, cargo_warning};
use crate::rustc;

/// Builder for updating sections in a binary.
///
/// Created by calling `LinkSection::patch_into()` or `LinkSection::patch_into_bin_dep()`.
#[must_use]
pub struct UpdateSectionCommand {
    pub(crate) link_section: LinkSection,
    pub(crate) bin_path: PathBuf,
    pub(crate) new_name: Option<String>,
}

impl UpdateSectionCommand {
    /// Sets a custom filename for the output binary.
    ///
    /// This can only be used when:
    /// - The argument to `write_to()` is a directory, or
    /// - Using `write_to_target_profile_dir()`
    ///
    /// If `write_to()` is called with a file path (not a directory), this will panic.
    ///
    /// If not called, the default name is `{original_name}.bin`.
    pub fn with_filename(mut self, name: &str) -> Self {
        self.new_name = Some(name.to_string());
        self
    }

    /// Writes the patched binary to the specified path.
    ///
    /// If the path is a directory, the output filename will be determined by
    /// `with_filename()` if set, otherwise defaults to `{original_name}.bin`.
    ///
    /// If the path is not a directory, writes directly to that path. In this case,
    /// `with_filename()` must not have been called (will panic if it was).
    ///
    /// If the section doesn't exist in the input binary, a warning is logged and the
    /// binary is copied without modification.
    pub fn write_to(self, path: impl AsRef<Path>) {
        let out_dir = cargo_helpers::out_dir();
        let section_file = self.link_section.write_section_to_path(&out_dir);

        eprintln!("ver-shim-build: input binary = {}", self.bin_path.display());

        // Emit rerun-if-changed for the input binary
        // See: https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed
        cargo_rerun_if(&format!("changed={}", self.bin_path.display()));

        // Determine output path
        let path = path.as_ref();
        let output_path = if path.is_dir() {
            // Directory: use new_name if set, otherwise default to {original_name}.bin
            let original_name = self
                .bin_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            let default_name = format!("{}.bin", original_name);
            let output_name = self.new_name.as_deref().unwrap_or(&default_name);
            path.join(output_name)
        } else {
            // File path: write directly, but panic if with_filename was used
            if self.new_name.is_some() {
                panic!(
                    "ver-shim-build: with_filename() cannot be used when write_to() \
                     is called with a file path (not a directory): {}",
                    path.display()
                );
            }
            path.to_path_buf()
        };

        if run_objcopy(&self.bin_path, &output_path, SECTION_NAME, &section_file) {
            eprintln!(
                "ver-shim-build: wrote patched binary to {}",
                output_path.display()
            );
        } else {
            // Section doesn't exist, copy binary without modification
            fs::copy(&self.bin_path, &output_path).unwrap_or_else(|e| {
                panic!(
                    "ver-shim-build: failed to copy {} to {}: {}",
                    self.bin_path.display(),
                    output_path.display(),
                    e
                )
            });
            eprintln!("ver-shim-build: copied to {}", output_path.display());
        }
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
        self.write_to(target_dir);
    }
}

/// Runs objcopy to update the section in the binary.
///
/// Returns `true` if the section was updated, `false` if the section doesn't exist.
fn run_objcopy(input: &Path, output: &Path, section_name: &str, section_file: &Path) -> bool {
    let bin_dir = rustc::llvm_tools_bin_dir().unwrap_or_else(|e| {
        panic!(
            "ver-shim-build: could not find LLVM tools directory: {}\n\
             Please install llvm-tools: rustup component add llvm-tools",
            e
        )
    });

    let readobj_path = bin_dir.join(format!("llvm-readobj{}", EXE_SUFFIX));
    let objcopy_path = bin_dir.join(format!("llvm-objcopy{}", EXE_SUFFIX));

    // Check if the section exists and get its size
    match get_section_info(input, section_name, &readobj_path) {
        None => {
            cargo_warning(&format!(
                "section '{}' not found in {}, skipping",
                section_name,
                input.display()
            ));
            return false;
        }
        Some(size) => {
            if size != BUFFER_SIZE {
                cargo_warning(&format!(
                    "section '{}' has size {} but expected {}, \
                     binary may have been built with different ver-shim version",
                    section_name, size, BUFFER_SIZE
                ));
            }
        }
    }

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

    true
}

/// Uses llvm-readobj to check if a section exists and get its size.
///
/// Returns `Some(size)` if the section exists, `None` if it doesn't.
fn get_section_info(binary: &Path, section_name: &str, readobj_path: &Path) -> Option<usize> {
    let output = Command::new(readobj_path)
        .arg("--sections")
        .arg(binary)
        .output()
        .unwrap_or_else(|e| {
            panic!(
                "ver-shim-build: failed to execute llvm-readobj at '{}': {}",
                readobj_path.display(),
                e
            )
        });

    if !output.status.success() {
        panic!(
            "ver-shim-build: llvm-readobj failed with status {}",
            output.status
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse llvm-readobj --sections output to find our section
    // Format is like:
    //   Section {
    //     Index: 16
    //     Name: .ver_shim_data (472)
    //     Type: SHT_PROGBITS (0x1)
    //     ...
    //     Size: 512
    //     ...
    //   }
    let mut in_target_section = false;
    for line in stdout.lines() {
        let trimmed = line.trim();

        // Check if we're entering our target section
        // Format: "Name: .ver_shim_data (472)"
        if let Some(name_part) = trimmed.strip_prefix("Name:") {
            // Remove parenthesized suffix and trim: ".ver_shim_data (472)" -> ".ver_shim_data"
            let name = match name_part.find('(') {
                Some(idx) => name_part[..idx].trim(),
                None => name_part.trim(),
            };
            in_target_section = name == section_name;
            continue;
        }

        // If we're in the target section, look for the Size line
        if in_target_section
            && let Some(size_str) = trimmed.strip_prefix("Size:")
        {
            let size = size_str.trim().parse::<usize>().unwrap_or_else(|e| {
                panic!(
                    "ver-shim-build: failed to parse section size '{}': {}",
                    size_str.trim(),
                    e
                )
            });
            return Some(size);
        }
    }

    None
}
