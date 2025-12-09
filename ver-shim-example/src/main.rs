fn main() {
    println!(
        "git sha:         {}",
        ver_shim::git_sha().unwrap_or("(not set)")
    );
    println!(
        "git describe:    {}",
        ver_shim::git_describe().unwrap_or("(not set)")
    );
    println!(
        "git branch:      {}",
        ver_shim::git_branch().unwrap_or("(not set)")
    );
    println!(
        "git timestamp:   {}",
        ver_shim::git_commit_timestamp().unwrap_or("(not set)")
    );
    println!(
        "git date:        {}",
        ver_shim::git_commit_date().unwrap_or("(not set)")
    );
    println!(
        "git msg:         {}",
        ver_shim::git_commit_msg().unwrap_or("(not set)")
    );
    println!(
        "build timestamp: {}",
        ver_shim::build_timestamp().unwrap_or("(not set)")
    );
    println!(
        "build date:      {}",
        ver_shim::build_date().unwrap_or("(not set)")
    );
}
