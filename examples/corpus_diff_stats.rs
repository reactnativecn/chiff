use chiff::{detect_input_format, diff_bytes};
use std::{
    collections::BTreeSet,
    env,
    fs,
    path::{Path, PathBuf},
    process,
};

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

    let old_files = collect_relative_files(&old_dir);
    let new_files = collect_relative_files(&new_dir);

    let relative_paths = old_files
        .iter()
        .chain(new_files.iter())
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut total_pairs = 0usize;
    let mut total_copy_ops = 0usize;
    let mut total_insert_ops = 0usize;
    let mut total_copied_bytes = 0usize;
    let mut total_inserted_bytes = 0usize;

    println!(
        "path\tstatus\told_format\tnew_format\top_count\tcopy_ops\tinsert_ops\tcopied_bytes\tinserted_bytes"
    );

    for relative_path in relative_paths {
        let old_path = old_dir.join(&relative_path);
        let new_path = new_dir.join(&relative_path);

        let (status, old_bytes, new_bytes) = match (old_path.is_file(), new_path.is_file()) {
            (true, true) => (
                "paired",
                Some(read_file(&old_path)),
                Some(read_file(&new_path)),
            ),
            (true, false) => ("missing_in_new", Some(read_file(&old_path)), None),
            (false, true) => ("missing_in_old", None, Some(read_file(&new_path))),
            (false, false) => continue,
        };

        let old_format = old_bytes
            .as_deref()
            .map(|bytes| format!("{:?}", detect_input_format(bytes)))
            .unwrap_or_else(|| String::from("-"));
        let new_format = new_bytes
            .as_deref()
            .map(|bytes| format!("{:?}", detect_input_format(bytes)))
            .unwrap_or_else(|| String::from("-"));

        let stats = match (old_bytes.as_deref(), new_bytes.as_deref()) {
            (Some(old), Some(new)) => {
                total_pairs += 1;
                let stats = diff_bytes(old, new).stats();
                total_copy_ops += stats.copy_op_count;
                total_insert_ops += stats.insert_op_count;
                total_copied_bytes += stats.copied_bytes;
                total_inserted_bytes += stats.inserted_bytes;
                (
                    stats.op_count,
                    stats.copy_op_count,
                    stats.insert_op_count,
                    stats.copied_bytes,
                    stats.inserted_bytes,
                )
            }
            _ => (0, 0, 0, 0, 0),
        };

        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            relative_path.display(),
            status,
            old_format,
            new_format,
            stats.0,
            stats.1,
            stats.2,
            stats.3,
            stats.4
        );
    }

    println!(
        "TOTAL\tpaired={}\t-\t-\t-\t{}\t{}\t{}\t{}",
        total_pairs, total_copy_ops, total_insert_ops, total_copied_bytes, total_inserted_bytes
    );
}

fn collect_relative_files(root: &Path) -> BTreeSet<PathBuf> {
    let mut files = BTreeSet::new();
    collect_relative_files_recursive(root, root, &mut files);
    files
}

fn collect_relative_files_recursive(root: &Path, current: &Path, files: &mut BTreeSet<PathBuf>) {
    let entries = fs::read_dir(current).unwrap_or_else(|error| {
        eprintln!("failed to read directory {}: {}", current.display(), error);
        process::exit(1);
    });

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            eprintln!("failed to read directory entry in {}: {}", current.display(), error);
            process::exit(1);
        });
        let path = entry.path();
        if path.is_dir() {
            collect_relative_files_recursive(root, &path, files);
        } else if path.is_file() {
            let relative = path.strip_prefix(root).unwrap_or_else(|error| {
                eprintln!(
                    "failed to compute relative path for {} under {}: {}",
                    path.display(),
                    root.display(),
                    error
                );
                process::exit(1);
            });
            files.insert(relative.to_path_buf());
        }
    }
}

fn read_file(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|error| {
        eprintln!("failed to read {}: {}", path.display(), error);
        process::exit(1);
    })
}

fn print_usage_and_exit() -> ! {
    eprintln!("usage: cargo run --example corpus_diff_stats -- <old-dir> <new-dir>");
    process::exit(2);
}
