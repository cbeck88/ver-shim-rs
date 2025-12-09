fn main() {
    ver_shim_build::LinkSection::new()
        .with_all_git()
        .with_all_build_time()
        .patch_into("ver-shim-example", "ver-shim-example")
        .write_to_target_profile_dir();
}
