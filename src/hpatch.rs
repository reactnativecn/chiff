use crate::{Patch, PatchOp, PatchOutputMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HpatchCoverSelectionPolicy {
    ChiffStructured,
    HdiffNative,
    MergedCosted,
}

impl HpatchCoverSelectionPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ChiffStructured => "chiff_structured",
            Self::HdiffNative => "hdiff_native",
            Self::MergedCosted => "merged_costed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HpatchCover {
    pub old_pos: u64,
    pub new_pos: u64,
    pub len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HpatchCompatiblePlan {
    pub old_size: u64,
    pub new_size: u64,
    pub covers: Vec<HpatchCover>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HpatchCompatiblePlanStats {
    pub cover_count: usize,
    pub covered_bytes: u64,
    pub uncovered_new_bytes: u64,
}

impl HpatchCompatiblePlan {
    pub const fn output_mode(&self) -> PatchOutputMode {
        PatchOutputMode::HpatchCompatible
    }

    pub fn stats(&self) -> HpatchCompatiblePlanStats {
        let covered_bytes = self
            .covers
            .iter()
            .fold(0u64, |total, cover| total.saturating_add(cover.len));

        HpatchCompatiblePlanStats {
            cover_count: self.covers.len(),
            covered_bytes,
            uncovered_new_bytes: self.new_size.saturating_sub(covered_bytes),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HpatchCompatiblePlanError {
    InvalidCopyRange {
        offset: usize,
        len: usize,
        old_len: usize,
    },
    SizeOverflow,
}

/// Converts `chiff`'s exact-copy patch IR into HDiffPatch cover-line semantics.
///
/// This is not a serialized `.hpatch` payload yet. It is the stable boundary
/// needed by HDiffPatch's `ICoverLinesListener`, whose cover records are
/// `{oldPos, newPos, length}`.
pub fn build_hpatch_compatible_plan(
    old_size: usize,
    patch: &Patch,
) -> Result<HpatchCompatiblePlan, HpatchCompatiblePlanError> {
    let mut covers = Vec::new();
    let mut new_pos = 0usize;

    for op in &patch.ops {
        match op {
            PatchOp::Copy { offset, len } => {
                let end = offset.checked_add(*len).ok_or(
                    HpatchCompatiblePlanError::InvalidCopyRange {
                        offset: *offset,
                        len: *len,
                        old_len: old_size,
                    },
                )?;
                if end > old_size {
                    return Err(HpatchCompatiblePlanError::InvalidCopyRange {
                        offset: *offset,
                        len: *len,
                        old_len: old_size,
                    });
                }

                if *len > 0 {
                    covers.push(HpatchCover {
                        old_pos: u64::try_from(*offset)
                            .map_err(|_| HpatchCompatiblePlanError::SizeOverflow)?,
                        new_pos: u64::try_from(new_pos)
                            .map_err(|_| HpatchCompatiblePlanError::SizeOverflow)?,
                        len: u64::try_from(*len)
                            .map_err(|_| HpatchCompatiblePlanError::SizeOverflow)?,
                    });
                }
                new_pos = new_pos
                    .checked_add(*len)
                    .ok_or(HpatchCompatiblePlanError::SizeOverflow)?;
            }
            PatchOp::Insert(bytes) => {
                new_pos = new_pos
                    .checked_add(bytes.len())
                    .ok_or(HpatchCompatiblePlanError::SizeOverflow)?;
            }
        }
    }

    Ok(HpatchCompatiblePlan {
        old_size: u64::try_from(old_size).map_err(|_| HpatchCompatiblePlanError::SizeOverflow)?,
        new_size: u64::try_from(new_pos).map_err(|_| HpatchCompatiblePlanError::SizeOverflow)?,
        covers,
    })
}
