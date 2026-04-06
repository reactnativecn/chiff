use chiff::{assess_structured_hermes, detect_input_format, diff_bytes, select_engine_decision};
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

    let patch = diff_bytes(&old, &new);
    let stats = patch.stats();

    println!("old_format={:?}", detect_input_format(&old));
    println!("new_format={:?}", detect_input_format(&new));
    let decision = select_engine_decision(&old, &new);
    println!("selected_engine={}", decision.kind.as_str());
    println!("selected_engine_reason={}", decision.reason.as_str());
    println!(
        "old_structured_hermes_support={}",
        assess_structured_hermes(&old).as_str()
    );
    println!(
        "new_structured_hermes_support={}",
        assess_structured_hermes(&new).as_str()
    );
    println!("op_count={}", stats.op_count);
    println!("copy_op_count={}", stats.copy_op_count);
    println!("insert_op_count={}", stats.insert_op_count);
    println!("copied_bytes={}", stats.copied_bytes);
    println!("inserted_bytes={}", stats.inserted_bytes);
}

fn print_usage_and_exit() -> ! {
    eprintln!("usage: cargo run --example diff_stats -- <old-file> <new-file>");
    process::exit(2);
}
