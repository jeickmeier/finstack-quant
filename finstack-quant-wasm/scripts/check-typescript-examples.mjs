// Type-check every documented code sample against the published declarations.
//
// The samples (see `doc-examples.mjs`) are compiled as one TypeScript program
// under `strict`, each as its own module inside this package, so
// `import ... from "finstack-quant-wasm"` resolves to `index.d.ts` the way it
// does for a consumer. `js` samples are checked with `checkJs`. The DOM lib is
// included on purpose: a sample that leans on a global the package also names
// (`Performance`) must fail here, not in a user's editor.
//
// This needs no built package. `tests/facade/doc_examples.test.mjs` executes
// the same samples against the build.
import { join } from 'node:path';
import ts from 'typescript';
import { PACKAGE_DIR, collectExamples } from './doc-examples.mjs';

const examples = collectExamples();
const directory = join(PACKAGE_DIR, '__doc_examples__');
const virtual = new Map(
  examples.map((example) => [join(directory, `${example.name}.${example.language}`), example])
);
// The package has no `@types/node` dependency; these are the Node built-ins the
// Node-specific samples use (reading the `.wasm` bytes, the test runner).
virtual.set(join(directory, 'node-builtins.d.ts'), {
  code: `
declare module 'node:fs' {
  export function readFileSync(path: string | URL): Uint8Array;
}
declare module 'node:test' {
  export default function test(name: string, fn: () => void | Promise<void>): void;
}
declare module 'node:assert/strict' {
  const assert: {
    (value: unknown, message?: string): asserts value;
    equal(actual: unknown, expected: unknown, message?: string): void;
    deepEqual(actual: unknown, expected: unknown, message?: string): void;
    throws(fn: () => unknown, expected?: unknown): void;
  };
  export default assert;
}
interface ImportMeta {
  url: string;
  resolve(specifier: string): string;
}
`,
});

const options = {
  target: ts.ScriptTarget.ES2022,
  module: ts.ModuleKind.NodeNext,
  moduleResolution: ts.ModuleResolutionKind.NodeNext,
  lib: ['lib.es2022.d.ts', 'lib.dom.d.ts'],
  strict: true,
  noEmit: true,
  allowJs: true,
  checkJs: true,
  skipLibCheck: true,
  types: [],
};

const host = ts.createCompilerHost(options);
const { fileExists, readFile, getSourceFile } = host;
host.fileExists = (path) => virtual.has(path) || fileExists(path);
host.readFile = (path) => virtual.get(path)?.code ?? readFile(path);
host.getSourceFile = (path, languageVersion, ...rest) => {
  const example = virtual.get(path);
  return example
    ? ts.createSourceFile(path, example.code, languageVersion, true)
    : getSourceFile(path, languageVersion, ...rest);
};

const program = ts.createProgram([...virtual.keys()], options, host);
const failures = [];
for (const diagnostic of ts.getPreEmitDiagnostics(program)) {
  const example = diagnostic.file && virtual.get(diagnostic.file.fileName);
  const message = ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n  ');
  if (!example?.source) {
    failures.push(`${diagnostic.file?.fileName ?? 'tsc'}: TS${diagnostic.code} ${message}`);
    continue;
  }
  const { line } = diagnostic.file.getLineAndCharacterOfPosition(diagnostic.start ?? 0);
  const text = example.code.split('\n')[line]?.trim() ?? '';
  failures.push(
    `${example.source}:${example.line} (${example.language} sample, line ${line + 1}): ` +
      `TS${diagnostic.code} ${message}\n    ${text}`
  );
}

if (failures.length) {
  console.error(failures.map((failure) => `- ${failure}`).join('\n'));
  console.error(
    `\n${failures.length} problem(s) in ${examples.length} documented samples do not type-check ` +
      'against index.d.ts.'
  );
  process.exit(1);
}
