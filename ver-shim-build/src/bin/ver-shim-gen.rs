use conf::Conf;
use std::path::PathBuf;
use ver_shim_build::LinkSection;

/// Generate ver-shim data file for use with objcopy.
#[derive(Debug, Conf)]
struct Args {
    /// Include git SHA (git rev-parse HEAD)
    #[conf(long)]
    git_sha: bool,

    /// Include git describe (git describe --always --dirty)
    #[conf(long)]
    git_describe: bool,

    /// Include git branch (git rev-parse --abbrev-ref HEAD)
    #[conf(long)]
    git_branch: bool,

    /// Include git commit timestamp
    #[conf(long)]
    git_commit_timestamp: bool,

    /// Include git commit date
    #[conf(long)]
    git_commit_date: bool,

    /// Include git commit message (first line)
    #[conf(long)]
    git_commit_msg: bool,

    /// Include all git information
    #[conf(long)]
    all_git: bool,

    /// Include build timestamp
    #[conf(long)]
    build_timestamp: bool,

    /// Include build date
    #[conf(long)]
    build_date: bool,

    /// Include all build time information
    #[conf(long)]
    all_build_time: bool,

    /// Custom string to include
    #[conf(long)]
    custom: Option<String>,

    /// Output directory (writes ver_shim_data file to this directory)
    #[conf(short, long, default_value = "target")]
    output: PathBuf,
}

fn main() {
    let args = Args::parse();

    let mut section = LinkSection::new();

    // Git options
    if args.all_git {
        section = section.with_all_git();
    } else {
        if args.git_sha {
            section = section.with_git_sha();
        }
        if args.git_describe {
            section = section.with_git_describe();
        }
        if args.git_branch {
            section = section.with_git_branch();
        }
        if args.git_commit_timestamp {
            section = section.with_git_commit_timestamp();
        }
        if args.git_commit_date {
            section = section.with_git_commit_date();
        }
        if args.git_commit_msg {
            section = section.with_git_commit_msg();
        }
    }

    // Build time options
    if args.all_build_time {
        section = section.with_all_build_time();
    } else {
        if args.build_timestamp {
            section = section.with_build_timestamp();
        }
        if args.build_date {
            section = section.with_build_date();
        }
    }

    // Custom string
    if let Some(custom) = args.custom {
        section = section.with_custom(custom);
    }

    // Write to output
    section.write_to(&args.output);

    eprintln!(
        "ver-shim-gen: wrote {}",
        args.output.join("ver_shim_data").display()
    );
}
