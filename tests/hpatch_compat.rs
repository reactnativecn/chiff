use chiff::{
    build_hpatch_compatible_plan, HpatchCompatiblePlan, HpatchCompatiblePlanError,
    HpatchCompatiblePlanStats, HpatchCover, HpatchCoverSelectionPolicy, OptimizationCompatibility,
    Patch, PatchOp, PatchOutputMode,
};

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
        HpatchCoverSelectionPolicy::HdiffNative.as_str(),
        "hdiff_native"
    );
    assert_eq!(
        HpatchCoverSelectionPolicy::MergedCosted.as_str(),
        "merged_costed"
    );
}
