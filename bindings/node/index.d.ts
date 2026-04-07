export type DetectFormatResult =
  | {
      kind: 'binary' | 'text_utf8';
    }
  | {
      kind: 'hermes_bytecode';
      version: number;
      form: 'execution' | 'delta';
    };

export function detectFormat(input: Uint8Array): DetectFormatResult;
export function selectEngineName(oldInput: Uint8Array, newInput: Uint8Array): 'generic_binary' | 'text' | 'hermes';
export function structuredHermesCompatible(input: Uint8Array): boolean;
export function selectEngineDecisionResult(oldInput: Uint8Array, newInput: Uint8Array): EngineDecisionResult;
export function structuredHermesSupport(input: Uint8Array): StructuredHermesSupportResult;
export function analyzeDiffResult(oldInput: Uint8Array, newInput: Uint8Array): AnalyzeDiffResult;
export function hpatchCompatiblePlanResult(oldInput: Uint8Array, newInput: Uint8Array): HpatchCompatiblePlanResult;
export function hpatchApproximatePlanResult(oldInput: Uint8Array, newInput: Uint8Array): HpatchCompatiblePlanResult;
export function analyzeDirectoryPairResult(oldRoot: string, newRoot: string): DirectoryAnalysisResult;

export type DiffStatsResult = {
  opCount: number;
  copyOpCount: number;
  insertOpCount: number;
  copiedBytes: number;
  insertedBytes: number;
};

export function diffStats(oldInput: Uint8Array, newInput: Uint8Array): DiffStatsResult;

export type AnalyzeDiffResult = {
  engineKind: 'generic_binary' | 'text' | 'hermes';
  engineReason:
    | 'binary_pair'
    | 'text_pair'
    | 'hermes_structured'
    | 'hermes_version_mismatch'
    | 'hermes_form_mismatch'
    | 'hermes_old_invalid_header'
    | 'hermes_old_unsupported_version'
    | 'hermes_new_invalid_header'
    | 'hermes_new_unsupported_version'
    | 'mixed_formats';
  oldStructuredHermesSupport:
    | 'not_hermes'
    | 'invalid_header'
    | 'unsupported_version'
    | 'supported';
  newStructuredHermesSupport:
    | 'not_hermes'
    | 'invalid_header'
    | 'unsupported_version'
    | 'supported';
  opCount: number;
  copyOpCount: number;
  insertOpCount: number;
  copiedBytes: number;
  insertedBytes: number;
};

export type CountEntryResult = {
  key: string;
  count: number;
};

export type CorpusSummaryResult = {
  paired: number;
  copyOps: number;
  insertOps: number;
  copiedBytes: number;
  insertedBytes: number;
  reasonCounts: CountEntryResult[];
  oldSupportCounts: CountEntryResult[];
  newSupportCounts: CountEntryResult[];
};

export type CorpusEntryResult = {
  relativePath: string;
  status: 'paired' | 'missing_in_old' | 'missing_in_new';
  oldFormat: string;
  newFormat: string;
  engineKind: 'generic_binary' | 'text' | 'hermes' | '-';
  engineReason:
    | 'binary_pair'
    | 'text_pair'
    | 'hermes_structured'
    | 'hermes_version_mismatch'
    | 'hermes_form_mismatch'
    | 'hermes_old_invalid_header'
    | 'hermes_old_unsupported_version'
    | 'hermes_new_invalid_header'
    | 'hermes_new_unsupported_version'
    | 'mixed_formats'
    | '-';
  oldStructuredHermesSupport:
    | 'not_hermes'
    | 'invalid_header'
    | 'unsupported_version'
    | 'supported'
    | '-';
  newStructuredHermesSupport:
    | 'not_hermes'
    | 'invalid_header'
    | 'unsupported_version'
    | 'supported'
    | '-';
  opCount: number;
  copyOpCount: number;
  insertOpCount: number;
  copiedBytes: number;
  insertedBytes: number;
};

export type DirectoryAnalysisResult = {
  entries: CorpusEntryResult[];
  summary: CorpusSummaryResult;
};

export type HpatchCoverResult = {
  oldPos: string;
  newPos: string;
  len: string;
};

export type HpatchCompatiblePlanResult = {
  outputMode: 'hpatch_compatible';
  coverPolicy:
    | 'chiff_structured'
    | 'chiff_approximate'
    | 'hdiff_native'
    | 'merged_costed';
  oldSize: string;
  newSize: string;
  coverCount: number;
  coveredBytes: string;
  uncoveredNewBytes: string;
  covers: HpatchCoverResult[];
};

export type EngineDecisionResult = {
  kind: 'generic_binary' | 'text' | 'hermes';
  reason:
    | 'binary_pair'
    | 'text_pair'
    | 'hermes_structured'
    | 'hermes_version_mismatch'
    | 'hermes_form_mismatch'
    | 'hermes_old_invalid_header'
    | 'hermes_old_unsupported_version'
    | 'hermes_new_invalid_header'
    | 'hermes_new_unsupported_version'
    | 'mixed_formats';
};

export type StructuredHermesSupportResult =
  | {
      status: 'not_hermes' | 'invalid_header';
      version?: undefined;
      form?: undefined;
    }
  | {
      status: 'unsupported_version' | 'supported';
      version: number;
      form: 'execution' | 'delta';
    };
