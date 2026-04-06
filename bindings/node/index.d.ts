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
