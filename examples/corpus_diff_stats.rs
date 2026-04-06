use chiff::analyze_directory_pair;
use std::{env, path::PathBuf, process};

fn main() {
    let mut args = env::args().skip(1);
    let Some(old_dir) = args.next() else {
        print_usage_and_exit();
    };
    let Some(new_dir) = args.next() else {
        print_usage_and_exit();
    };
    if args.next().is_some() {
        print_usage_and_exit();
    }

    let old_dir = PathBuf::from(old_dir);
    let new_dir = PathBuf::from(new_dir);

    if !old_dir.is_dir() || !new_dir.is_dir() {
        eprintln!("both arguments must be directories");
        process::exit(2);
    }

    let corpus = analyze_directory_pair(&old_dir, &new_dir).unwrap_or_else(|error| {
        eprintln!("failed to analyze corpus: {}", error);
        process::exit(1);
    });

    println!(
        "path\tstatus\told_format\tnew_format\tselected_engine\tselected_engine_reason\told_structured_hermes_support\tnew_structured_hermes_support\top_count\tcopy_ops\tinsert_ops\tcopied_bytes\tinserted_bytes"
    );

    for entry in corpus.entries {
        let old_format = entry
            .old_format
            .map(|format| format!("{:?}", format))
            .unwrap_or_else(|| String::from("-"));
        let new_format = entry
            .new_format
            .map(|format| format!("{:?}", format))
            .unwrap_or_else(|| String::from("-"));
        let (
            op_count,
            copy_ops,
            insert_ops,
            copied_bytes,
            inserted_bytes,
            selected_engine,
            selected_engine_reason,
            old_structured_hermes,
            new_structured_hermes,
        ) = match entry.diff_analysis {
            Some(analysis) => (
                analysis.stats.op_count,
                analysis.stats.copy_op_count,
                analysis.stats.insert_op_count,
                analysis.stats.copied_bytes,
                analysis.stats.inserted_bytes,
                String::from(analysis.engine_decision.kind.as_str()),
                String::from(analysis.engine_decision.reason.as_str()),
                String::from(analysis.old_structured_hermes_support.as_str()),
                String::from(analysis.new_structured_hermes_support.as_str()),
            ),
            None => (
                0,
                0,
                0,
                0,
                0,
                String::from("-"),
                String::from("-"),
                String::from("-"),
                String::from("-"),
            ),
        };

        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            entry.relative_path.display(),
            entry.status.as_str(),
            old_format,
            new_format,
            selected_engine,
            selected_engine_reason,
            old_structured_hermes,
            new_structured_hermes,
            op_count,
            copy_ops,
            insert_ops,
            copied_bytes,
            inserted_bytes
        );
    }

    println!(
        "TOTAL\tpaired={}\t-\t-\t-\t-\t-\t-\t-\t{}\t{}\t{}\t{}",
        corpus.summary.paired,
        corpus.summary.copy_ops,
        corpus.summary.insert_ops,
        corpus.summary.copied_bytes,
        corpus.summary.inserted_bytes
    );

    for (reason, count) in corpus.summary.reason_counts {
        println!("SUMMARY\treason\t{}\t{}", reason, count);
    }
    for (support, count) in corpus.summary.old_support_counts {
        println!("SUMMARY\told_support\t{}\t{}", support, count);
    }
    for (support, count) in corpus.summary.new_support_counts {
        println!("SUMMARY\tnew_support\t{}\t{}", support, count);
    }
}

fn print_usage_and_exit() -> ! {
    eprintln!("usage: cargo run --example corpus_diff_stats -- <old-dir> <new-dir>");
    process::exit(2);
}
