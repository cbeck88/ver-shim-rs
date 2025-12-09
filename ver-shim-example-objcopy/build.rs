fn main() {
    // Write the section data to target/ver_shim_data
    // Then build with:
    //   cargo objcopy --release --bin ver-shim-example-objcopy -- \
    //     --update-section .ver_shim_data=target/ver_shim_data ver-shim-example-objcopy.bin
    ver_shim_build::LinkSection::new()
        .with_all_git()
        .with_all_build_time()
        .write_to_target_dir();
}
