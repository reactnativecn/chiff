use crate::{analyze_diff, detect_input_format, DiffAnalysis, InputFormat};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusEntryStatus {
    Paired,
    MissingInOld,
    MissingInNew,
}

impl CorpusEntryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Paired => "paired",
            Self::MissingInOld => "missing_in_old",
            Self::MissingInNew => "missing_in_new",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusEntryAnalysis {
    pub relative_path: PathBuf,
    pub status: CorpusEntryStatus,
    pub old_format: Option<InputFormat>,
    pub new_format: Option<InputFormat>,
    pub diff_analysis: Option<DiffAnalysis>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusSummary {
    pub paired: usize,
    pub copy_ops: usize,
    pub insert_ops: usize,
    pub copied_bytes: usize,
    pub inserted_bytes: usize,
    pub reason_counts: BTreeMap<String, usize>,
    pub old_support_counts: BTreeMap<String, usize>,
    pub new_support_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusAnalysis {
    pub old_root: PathBuf,
    pub new_root: PathBuf,
    pub entries: Vec<CorpusEntryAnalysis>,
    pub summary: CorpusSummary,
}

pub fn analyze_directory_pair(old_root: &Path, new_root: &Path) -> io::Result<CorpusAnalysis> {
    if !old_root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("old root is not a directory: {}", old_root.display()),
        ));
    }
    if !new_root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("new root is not a directory: {}", new_root.display()),
        ));
    }

    let old_files = collect_relative_files(old_root)?;
    let new_files = collect_relative_files(new_root)?;
    let relative_paths = old_files
        .iter()
        .chain(new_files.iter())
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut entries = Vec::new();
    let mut summary = CorpusSummary {
        paired: 0,
        copy_ops: 0,
        insert_ops: 0,
        copied_bytes: 0,
        inserted_bytes: 0,
        reason_counts: BTreeMap::new(),
        old_support_counts: BTreeMap::new(),
        new_support_counts: BTreeMap::new(),
    };

    for relative_path in relative_paths {
        let old_path = old_root.join(&relative_path);
        let new_path = new_root.join(&relative_path);

        let old_bytes = old_path
            .is_file()
            .then(|| fs::read(&old_path))
            .transpose()?;
        let new_bytes = new_path
            .is_file()
            .then(|| fs::read(&new_path))
            .transpose()?;

        let (status, diff_analysis) = match (old_bytes.as_deref(), new_bytes.as_deref()) {
            (Some(old), Some(new)) => {
                let analysis = analyze_diff(old, new);
                summary.paired += 1;
                summary.copy_ops += analysis.stats.copy_op_count;
                summary.insert_ops += analysis.stats.insert_op_count;
                summary.copied_bytes += analysis.stats.copied_bytes;
                summary.inserted_bytes += analysis.stats.inserted_bytes;
                *summary
                    .reason_counts
                    .entry(String::from(analysis.engine_decision.reason.as_str()))
                    .or_default() += 1;
                *summary
                    .old_support_counts
                    .entry(String::from(
                        analysis.old_structured_hermes_support.as_str(),
                    ))
                    .or_default() += 1;
                *summary
                    .new_support_counts
                    .entry(String::from(
                        analysis.new_structured_hermes_support.as_str(),
                    ))
                    .or_default() += 1;
                (CorpusEntryStatus::Paired, Some(analysis))
            }
            (Some(_), None) => (CorpusEntryStatus::MissingInNew, None),
            (None, Some(_)) => (CorpusEntryStatus::MissingInOld, None),
            (None, None) => continue,
        };

        entries.push(CorpusEntryAnalysis {
            relative_path,
            status,
            old_format: old_bytes.as_deref().map(detect_input_format),
            new_format: new_bytes.as_deref().map(detect_input_format),
            diff_analysis,
        });
    }

    Ok(CorpusAnalysis {
        old_root: old_root.to_path_buf(),
        new_root: new_root.to_path_buf(),
        entries,
        summary,
    })
}

fn collect_relative_files(root: &Path) -> io::Result<BTreeSet<PathBuf>> {
    let mut files = BTreeSet::new();
    collect_relative_files_recursive(root, root, &mut files)?;
    Ok(files)
}

fn collect_relative_files_recursive(
    root: &Path,
    current: &Path,
    files: &mut BTreeSet<PathBuf>,
) -> io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_relative_files_recursive(root, &path, files)?;
        } else if path.is_file() {
            files.insert(path.strip_prefix(root).unwrap().to_path_buf());
        }
    }

    Ok(())
}
