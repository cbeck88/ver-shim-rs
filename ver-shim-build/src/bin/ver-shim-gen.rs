use conf::{Conf, Subcommands};
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

    /// Output path (writes to this path, or {path}/ver_shim_data if it's a directory).
    /// Mutually exclusive with subcommands.
    #[conf(short, long)]
    output: Option<PathBuf>,

    #[conf(subcommands)]
    command: Option<Command>,
}

#[derive(Debug, Subcommands)]
enum Command {
    /// Patch version info into an existing binary
    Patch {
        /// Input binary to patch
        #[conf(pos)]
        input: PathBuf,

        /// Output path (defaults to input's parent directory)
        #[conf(short, long)]
        output: Option<PathBuf>,
    },
}

fn build_section(args: &Args) -> LinkSection {
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
    if let Some(ref custom) = args.custom {
        section = section.with_custom(custom);
    }

    section
}

fn main() {
    // Unset OUT_DIR to prevent LinkSection from trying to use build.rs paths
    std::env::remove_var("OUT_DIR");

    let args = Args::parse();

    // Error if --output is specified with a subcommand
    if args.output.is_some() && args.command.is_some() {
        eprintln!(
            "error: when using patch command, top-level --output flag is ignored; \
             this is probably not what you intended"
        );
        std::process::exit(1);
    }

    let section = build_section(&args);

    match args.command {
        Some(Command::Patch { ref input, ref output }) => {
            let output_path = output
                .clone()
                .unwrap_or_else(|| input.parent().unwrap().to_path_buf());
            section.patch_into(input).write_to(&output_path);
            eprintln!(
                "ver-shim-gen: patched {} -> {}",
                input.display(),
                output_path.display()
            );
        }
        None => {
            let Some(output) = args.output else {
                eprintln!("error: --output is required when not using a subcommand");
                std::process::exit(1);
            };
            let output_path = section.write_to(&output);
            eprintln!("ver-shim-gen: wrote {}", output_path.display());
        }
    }
}
