//! LLVM tools wrapper for section manipulation.

use std::env::consts::EXE_SUFFIX;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cargo_helpers::cargo_warning;
use crate::rustc;

/// Wrapper for LLVM tools (llvm-readobj, llvm-objcopy).
pub struct LlvmTools {
    bin_dir: PathBuf,
}

impl LlvmTools {
    /// Creates a new `LlvmTools` instance by locating the LLVM tools directory.
    pub fn new() -> Result<Self, String> {
        let bin_dir = rustc::llvm_tools_bin_dir()?;
        Ok(Self { bin_dir })
    }

    /// Gets the size of a section in a binary.
    ///
    /// Returns `Some(size)` if the section exists, `None` if it doesn't.
    /// Panics on errors (e.g., llvm-readobj fails to execute or parse).
    pub fn get_section_size(&self, bin: impl AsRef<Path>, section_name: &str) -> Option<usize> {
        let bin = bin.as_ref();
        let readobj_path = self.bin_dir.join(format!("llvm-readobj{}", EXE_SUFFIX));

        let output = Command::new(&readobj_path)
            .arg("--sections")
            .arg(bin)
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

    /// Updates a section in a binary, or copies the binary if the section doesn't exist.
    ///
    /// If the section exists, uses llvm-objcopy to update it with the contents of `section_file`.
    /// If the section doesn't exist, copies the input binary to the output path unchanged
    /// and emits a warning.
    ///
    /// Returns `true` if the section was updated, `false` if the binary was copied.
    /// Panics on errors.
    pub fn update_section_or_copy(
        &self,
        input: impl AsRef<Path>,
        output: impl AsRef<Path>,
        section_name: &str,
        section_file: impl AsRef<Path>,
    ) -> bool {
        let input = input.as_ref();
        let output = output.as_ref();
        let section_file = section_file.as_ref();

        // Check if the section exists
        let section_size = self.get_section_size(input, section_name);

        if section_size.is_none() {
            cargo_warning(&format!(
                "section '{}' not found in {}, copying without modification",
                section_name,
                input.display()
            ));
            fs::copy(input, output).unwrap_or_else(|e| {
                panic!(
                    "ver-shim-build: failed to copy {} to {}: {}",
                    input.display(),
                    output.display(),
                    e
                )
            });
            return false;
        }

        // Run objcopy to update the section
        let objcopy_path = self.bin_dir.join(format!("llvm-objcopy{}", EXE_SUFFIX));
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
}
