use chiff::{
    build_hpatch_approximate_plan, build_hpatch_compatible_plan, HpatchCompatiblePlan,
    HpatchCompatiblePlanError, HpatchCompatiblePlanStats, HpatchCover, HpatchCoverSelectionPolicy,
    OptimizationCompatibility, Patch, PatchOp, PatchOutputMode,
};

const HERMES_MAGIC: u64 = 0x1F19_03C1_03BC_1FC6;

fn hermes_with_string_storage_and_body(storage_fill: u8, body_fill: u8) -> Vec<u8> {
    let header_len = 128usize;
    let function_headers_len = 12usize;
    let string_storage_len = 64usize;
    let bytecode_size = 64usize;
    let structured_end = header_len + function_headers_len + string_storage_len;
    let debug_info_offset = structured_end + bytecode_size;
    let file_length = debug_info_offset + 8;

    let mut bytes = vec![0; file_length];
    bytes[0..8].copy_from_slice(&HERMES_MAGIC.to_le_bytes());
    bytes[8..12].copy_from_slice(&99u32.to_le_bytes());
    bytes[32..36].copy_from_slice(&(file_length as u32).to_le_bytes());
    bytes[40..44].copy_from_slice(&1u32.to_le_bytes());
    bytes[60..64].copy_from_slice(&(string_storage_len as u32).to_le_bytes());
    bytes[108..112].copy_from_slice(&(debug_info_offset as u32).to_le_bytes());

    let function_header_offset = header_len;
    let bytecode_offset = structured_end as u32;
    bytes[function_header_offset..function_header_offset + 4]
        .copy_from_slice(&bytecode_offset.to_le_bytes());
    bytes[function_header_offset + 4..function_header_offset + 8]
        .copy_from_slice(&(bytecode_size as u32).to_le_bytes());

    bytes[header_len + function_headers_len..structured_end].fill(storage_fill);
    bytes[structured_end..debug_info_offset].fill(body_fill);
    bytes[debug_info_offset..file_length].fill(0xFE);

    bytes
}

#[test]
fn hpatch_compatible_plan_exports_cover_coordinates() {
    let patch = Patch {
        ops: vec![
            PatchOp::Copy { offset: 5, len: 3 },
            PatchOp::Insert(b"xx".to_vec()),
            PatchOp::Copy { offset: 20, len: 4 },
        ],
    };

    assert_eq!(
        build_hpatch_compatible_plan(32, &patch),
        Ok(HpatchCompatiblePlan {
            old_size: 32,
            new_size: 9,
            covers: vec![
                HpatchCover {
                    old_pos: 5,
                    new_pos: 0,
                    len: 3,
                },
                HpatchCover {
                    old_pos: 20,
                    new_pos: 5,
                    len: 4,
                },
            ],
        })
    );
}

#[test]
fn hpatch_compatible_plan_reports_cover_cost_floor() {
    let patch = Patch {
        ops: vec![
            PatchOp::Copy { offset: 0, len: 3 },
            PatchOp::Insert(b"123".to_vec()),
            PatchOp::Copy { offset: 6, len: 3 },
        ],
    };

    let plan = build_hpatch_compatible_plan(9, &patch).unwrap();

    assert_eq!(plan.output_mode(), PatchOutputMode::HpatchCompatible);
    assert_eq!(
        plan.stats(),
        HpatchCompatiblePlanStats {
            cover_count: 2,
            covered_bytes: 6,
            uncovered_new_bytes: 3,
        }
    );
}

#[test]
fn hpatch_compatible_plan_rejects_out_of_bounds_cover() {
    let patch = Patch {
        ops: vec![PatchOp::Copy { offset: 8, len: 4 }],
    };

    assert_eq!(
        build_hpatch_compatible_plan(10, &patch),
        Err(HpatchCompatiblePlanError::InvalidCopyRange {
            offset: 8,
            len: 4,
            old_len: 10,
        })
    );
}

#[test]
fn hpatch_approximate_plan_exports_hermes_section_and_body_hints() {
    let old_bytes = hermes_with_string_storage_and_body(0x11, 0x22);
    let new_bytes = hermes_with_string_storage_and_body(0x33, 0x44);

    let plan = build_hpatch_approximate_plan(&old_bytes, &new_bytes);

    assert_eq!(plan.output_mode(), PatchOutputMode::HpatchCompatible);
    assert_eq!(plan.old_size, old_bytes.len() as u64);
    assert_eq!(plan.new_size, new_bytes.len() as u64);
    assert_eq!(
        plan.covers,
        vec![
            HpatchCover {
                old_pos: 0,
                new_pos: 0,
                len: 128,
            },
            HpatchCover {
                old_pos: 140,
                new_pos: 140,
                len: 64,
            },
            HpatchCover {
                old_pos: 204,
                new_pos: 204,
                len: 64,
            },
        ]
    );
}

#[test]
fn hpatch_approximate_plan_stays_empty_for_non_hermes_inputs() {
    let plan = build_hpatch_approximate_plan(b"old text", b"new text");

    assert_eq!(plan.old_size, 8);
    assert_eq!(plan.new_size, 8);
    assert!(plan.covers.is_empty());
}

#[test]
fn output_modes_keep_native_only_optimizations_out_of_hpatch_compatible_lane() {
    assert_eq!(
        PatchOutputMode::HpatchCompatible.as_str(),
        "hpatch_compatible"
    );
    assert_eq!(PatchOutputMode::NativeChiff.as_str(), "native_chiff");
    assert!(PatchOutputMode::HpatchCompatible.patch_side_compatible());
    assert!(!PatchOutputMode::NativeChiff.patch_side_compatible());

    assert!(OptimizationCompatibility::OriginalByteCover
        .is_allowed_in(PatchOutputMode::HpatchCompatible));
    assert!(
        OptimizationCompatibility::OriginalByteCover.is_allowed_in(PatchOutputMode::NativeChiff)
    );
    assert!(!OptimizationCompatibility::NativeOnly.is_allowed_in(PatchOutputMode::HpatchCompatible));
    assert!(OptimizationCompatibility::NativeOnly.is_allowed_in(PatchOutputMode::NativeChiff));

    assert_eq!(
        HpatchCoverSelectionPolicy::ChiffStructured.as_str(),
        "chiff_structured"
    );
    assert_eq!(
        HpatchCoverSelectionPolicy::ChiffApproximate.as_str(),
        "chiff_approximate"
    );
    assert_eq!(
        HpatchCoverSelectionPolicy::HdiffNative.as_str(),
        "hdiff_native"
    );
    assert_eq!(
        HpatchCoverSelectionPolicy::MergedCosted.as_str(),
        "merged_costed"
    );
}
