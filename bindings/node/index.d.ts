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

export type DiffStatsResult = {
  opCount: number;
  copyOpCount: number;
  insertOpCount: number;
  copiedBytes: number;
  insertedBytes: number;
};

export function diffStats(oldInput: Uint8Array, newInput: Uint8Array): DiffStatsResult;
