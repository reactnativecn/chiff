use crate::{
    assess_structured_hermes, parse_artifact, HermesForm, HermesSection, Patch, PatchOp,
    PatchOutputMode, StructuredHermesSupport,
};

const MIN_APPROXIMATE_COVER_LEN: u32 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HpatchCoverSelectionPolicy {
    ChiffStructured,
    ChiffApproximate,
    HdiffNative,
    MergedCosted,
}

impl HpatchCoverSelectionPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ChiffStructured => "chiff_structured",
            Self::ChiffApproximate => "chiff_approximate",
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

/// Builds an experimental approximate-cover plan for Hermes execution bytecode.
///
/// Unlike [`build_hpatch_compatible_plan`], these covers are not exact copies.
/// They are original-file coordinate hints for HDiffPatch's own covered-region
/// sub-diff encoding, so this must stay behind serialized-size cost checks.
pub fn build_hpatch_approximate_plan(old_input: &[u8], new_input: &[u8]) -> HpatchCompatiblePlan {
    let mut covers = Vec::new();

    let old_support = assess_structured_hermes(old_input);
    let new_support = assess_structured_hermes(new_input);
    if let (
        StructuredHermesSupport::Supported {
            version: old_version,
            form: old_form,
        },
        StructuredHermesSupport::Supported {
            version: new_version,
            form: new_form,
        },
    ) = (old_support, new_support)
    {
        if old_version == new_version && old_form == new_form && old_form == HermesForm::Execution {
            if let (Some(old_artifact), Some(new_artifact)) =
                (parse_artifact(old_input), parse_artifact(new_input))
            {
                push_approximate_cover(&mut covers, 0, 0, 128);

                for new_section in &new_artifact.section_layout.sections {
                    if let Some(old_section) = old_artifact
                        .section_layout
                        .sections
                        .iter()
                        .find(|section| section.kind == new_section.kind)
                    {
                        push_section_approximate_cover(&mut covers, old_section, new_section);
                    }
                }

                if let (Some(old_layout), Some(new_layout)) =
                    (&old_artifact.function_layout, &new_artifact.function_layout)
                {
                    push_approximate_cover(
                        &mut covers,
                        old_layout.bytecode_region_start,
                        new_layout.bytecode_region_start,
                        old_layout
                            .bytecode_region_end
                            .saturating_sub(old_layout.bytecode_region_start)
                            .min(
                                new_layout
                                    .bytecode_region_end
                                    .saturating_sub(new_layout.bytecode_region_start),
                            ),
                    );
                    push_approximate_cover(
                        &mut covers,
                        old_layout.bytecode_region_end,
                        new_layout.bytecode_region_end,
                        old_artifact
                            .header
                            .debug_info_offset
                            .saturating_sub(old_layout.bytecode_region_end)
                            .min(
                                new_artifact
                                    .header
                                    .debug_info_offset
                                    .saturating_sub(new_layout.bytecode_region_end),
                            ),
                    );
                }

                if let (Some(old_layout), Some(new_layout)) = (
                    &old_artifact.debug_info_layout,
                    &new_artifact.debug_info_layout,
                ) {
                    push_approximate_cover(
                        &mut covers,
                        old_layout.header_offset,
                        new_layout.header_offset,
                        old_layout
                            .header_end_offset
                            .saturating_sub(old_layout.header_offset)
                            .min(
                                new_layout
                                    .header_end_offset
                                    .saturating_sub(new_layout.header_offset),
                            ),
                    );
                    push_approximate_cover(
                        &mut covers,
                        old_layout.filename_table_offset,
                        new_layout.filename_table_offset,
                        old_layout
                            .filename_table_end_offset
                            .saturating_sub(old_layout.filename_table_offset)
                            .min(
                                new_layout
                                    .filename_table_end_offset
                                    .saturating_sub(new_layout.filename_table_offset),
                            ),
                    );
                    push_approximate_cover(
                        &mut covers,
                        old_layout.filename_storage_offset,
                        new_layout.filename_storage_offset,
                        old_layout
                            .filename_storage_end_offset
                            .saturating_sub(old_layout.filename_storage_offset)
                            .min(
                                new_layout
                                    .filename_storage_end_offset
                                    .saturating_sub(new_layout.filename_storage_offset),
                            ),
                    );
                    push_approximate_cover(
                        &mut covers,
                        old_layout.file_regions_offset,
                        new_layout.file_regions_offset,
                        old_layout
                            .file_regions_end_offset
                            .saturating_sub(old_layout.file_regions_offset)
                            .min(
                                new_layout
                                    .file_regions_end_offset
                                    .saturating_sub(new_layout.file_regions_offset),
                            ),
                    );

                    push_approximate_cover(
                        &mut covers,
                        old_layout.debug_data_offset,
                        new_layout.debug_data_offset,
                        old_layout
                            .debug_data_end_offset
                            .saturating_sub(old_layout.debug_data_offset)
                            .min(
                                new_layout
                                    .debug_data_end_offset
                                    .saturating_sub(new_layout.debug_data_offset),
                            ),
                    );
                }
            }
        }
    }

    normalize_covers(&mut covers);

    HpatchCompatiblePlan {
        old_size: old_input.len() as u64,
        new_size: new_input.len() as u64,
        covers,
    }
}

fn push_section_approximate_cover(
    covers: &mut Vec<HpatchCover>,
    old_section: &HermesSection,
    new_section: &HermesSection,
) {
    push_approximate_cover(
        covers,
        old_section.offset,
        new_section.offset,
        old_section.len.min(new_section.len),
    );
}

fn push_approximate_cover(covers: &mut Vec<HpatchCover>, old_pos: u32, new_pos: u32, len: u32) {
    if len < MIN_APPROXIMATE_COVER_LEN {
        return;
    }

    covers.push(HpatchCover {
        old_pos: u64::from(old_pos),
        new_pos: u64::from(new_pos),
        len: u64::from(len),
    });
}

fn normalize_covers(covers: &mut Vec<HpatchCover>) {
    covers.sort_by_key(|cover| (cover.new_pos, cover.old_pos, std::cmp::Reverse(cover.len)));

    let mut normalized = Vec::with_capacity(covers.len());
    let mut previous_new_end = 0u64;
    for mut cover in covers.drain(..) {
        if cover.new_pos < previous_new_end {
            let trim = previous_new_end - cover.new_pos;
            if trim >= cover.len {
                continue;
            }
            cover.old_pos = cover.old_pos.saturating_add(trim);
            cover.new_pos = previous_new_end;
            cover.len -= trim;
        }

        if cover.len == 0 {
            continue;
        }

        previous_new_end = cover.new_pos.saturating_add(cover.len);
        normalized.push(cover);
    }

    *covers = normalized;
}
