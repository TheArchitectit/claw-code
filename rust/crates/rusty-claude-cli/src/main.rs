use std::env;
use std::path::PathBuf;

use compat_harness::{extract_manifest, UpstreamPaths};
use runtime::BootstrapPlan;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("dump-manifests") => dump_manifests(),
        Some("bootstrap-plan") => print_bootstrap_plan(),
        Some("--help") | Some("-h") => print_help(),
        Some(other) => {
            eprintln!("unknown subcommand: {other}");
            print_help();
            std::process::exit(2);
        }
        None => {
            println!("rusty-claude-cli: Rust compatibility foundation only.");
            println!("Run `rusty-claude-cli --help` for the current scaffold commands.");
        }
    }
}

fn dump_manifests() {
    let workspace_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let paths = UpstreamPaths::from_workspace_dir(&workspace_dir);
    match extract_manifest(&paths) {
        Ok(manifest) => {
            println!("commands: {}", manifest.commands.entries().len());
            println!("tools: {}", manifest.tools.entries().len());
            println!("bootstrap phases: {}", manifest.bootstrap.phases().len());
        }
        Err(error) => {
            eprintln!("failed to extract manifests: {error}");
            std::process::exit(1);
        }
    }
}

fn print_bootstrap_plan() {
    for phase in BootstrapPlan::claude_code_default().phases() {
        println!("- {phase:?}");
    }
}

fn print_help() {
    println!("rusty-claude-cli");
    println!();
    println!("Current scaffold commands:");
    println!("  dump-manifests   Read upstream TS sources and print extracted counts");
    println!("  bootstrap-plan   Print the current bootstrap phase skeleton");
}
