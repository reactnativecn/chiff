use chiff::{analyze_diff, detect_input_format};
use std::{env, fs, process};

fn main() {
    let mut args = env::args().skip(1);
    let Some(old_path) = args.next() else {
        print_usage_and_exit();
    };
    let Some(new_path) = args.next() else {
        print_usage_and_exit();
    };
    if args.next().is_some() {
        print_usage_and_exit();
    }

    let old = fs::read(&old_path).unwrap_or_else(|error| {
        eprintln!("failed to read old file {}: {}", old_path, error);
        process::exit(1);
    });
    let new = fs::read(&new_path).unwrap_or_else(|error| {
        eprintln!("failed to read new file {}: {}", new_path, error);
        process::exit(1);
    });

    let analysis = analyze_diff(&old, &new);

    println!("old_format={:?}", detect_input_format(&old));
    println!("new_format={:?}", detect_input_format(&new));
    println!("selected_engine={}", analysis.engine_decision.kind.as_str());
    println!(
        "selected_engine_reason={}",
        analysis.engine_decision.reason.as_str()
    );
    println!(
        "old_structured_hermes_support={}",
        analysis.old_structured_hermes_support.as_str()
    );
    println!(
        "new_structured_hermes_support={}",
        analysis.new_structured_hermes_support.as_str()
    );
    println!("op_count={}", analysis.stats.op_count);
    println!("copy_op_count={}", analysis.stats.copy_op_count);
    println!("insert_op_count={}", analysis.stats.insert_op_count);
    println!("copied_bytes={}", analysis.stats.copied_bytes);
    println!("inserted_bytes={}", analysis.stats.inserted_bytes);
}

fn print_usage_and_exit() -> ! {
    eprintln!("usage: cargo run --example diff_stats -- <old-file> <new-file>");
    process::exit(2);
}
