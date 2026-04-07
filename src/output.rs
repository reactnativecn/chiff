#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchOutputMode {
    HpatchCompatible,
    NativeChiff,
}

impl PatchOutputMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HpatchCompatible => "hpatch_compatible",
            Self::NativeChiff => "native_chiff",
        }
    }

    pub const fn patch_side_compatible(self) -> bool {
        matches!(self, Self::HpatchCompatible)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationCompatibility {
    OriginalByteCover,
    NativeOnly,
}

impl OptimizationCompatibility {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OriginalByteCover => "original_byte_cover",
            Self::NativeOnly => "native_only",
        }
    }

    pub const fn is_allowed_in(self, mode: PatchOutputMode) -> bool {
        match self {
            Self::OriginalByteCover => true,
            Self::NativeOnly => matches!(mode, PatchOutputMode::NativeChiff),
        }
    }
}
