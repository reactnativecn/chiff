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

export type DiffStatsResult = {
  opCount: number;
  copyOpCount: number;
  insertOpCount: number;
  copiedBytes: number;
  insertedBytes: number;
};

export function diffStats(oldInput: Uint8Array, newInput: Uint8Array): DiffStatsResult;
