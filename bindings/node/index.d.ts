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

export type DiffStatsResult = {
  opCount: number;
  copyOpCount: number;
  insertOpCount: number;
  copiedBytes: number;
  insertedBytes: number;
};

export function diffStats(oldInput: Uint8Array, newInput: Uint8Array): DiffStatsResult;

export type EngineDecisionResult = {
  kind: 'generic_binary' | 'text' | 'hermes';
  reason:
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
