fn main() {
    ver_shim_build::LinkSection::new()
        .with_all_git()
        .with_all_build_time()
        .patch_into_bin_dep("ver-shim-example", "ver-shim-example")
        .with_filename("ver-shim-example.bin")
        .write_to_target_profile_dir();
}
