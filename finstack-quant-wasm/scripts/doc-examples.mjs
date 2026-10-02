// Collect every code sample a consumer can copy: each fenced `ts`/`js` block in
// an `index.d.ts` doc comment (the `@example` blocks) and in the Markdown files
// that document the package.
//
// `check-typescript-examples.mjs` type-checks the samples and
// `tests/facade/doc_examples.test.mjs` executes them. A sample must therefore
// be a complete program. One convenience: a block with no `import` statement
// continues from the standard setup, so both gates put `PRELUDE` in front of
// it. A fence tagged `no-run` (```` ```js no-run ````) is skipped by both
// gates; use it only for a sample that shows what not to write.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const PACKAGE = new URL('../', import.meta.url);

/** The files whose samples are gated, relative to the package root. */
export const SOURCES = [
  'index.d.ts',
  'README.md',
  '../.agents/rules/wasm/javascript-usage-standards.md',
];

const LANGUAGES = new Map([
  ['ts', 'ts'],
  ['typescript', 'ts'],
  ['js', 'js'],
  ['javascript', 'js'],
]);

// Top-level facade namespaces (`core`, `analytics`, ...): the undotted keys of
// the checked-in export surface.
const NAMESPACES = Object.keys(
  JSON.parse(readFileSync(new URL('facade-surface.json', PACKAGE), 'utf8'))
)
  .filter((name) => !name.includes('.'))
  .sort();

/** Setup assumed by a sample that has no `import` statement of its own. */
export const PRELUDE = `import init, { ${NAMESPACES.join(', ')} } from "finstack-quant-wasm";\nawait init();\n`;

/**
 * Fenced `ts`/`js` blocks of one file.
 *
 * @param {string} text - File contents.
 * @param {boolean} inComments - Whether the fences sit in `/** ... *\/` comments
 *   (a declaration file) rather than in Markdown.
 * @returns {{ line: number, language: 'ts' | 'js', code: string }[]} Blocks
 *   with the 1-based line of the opening fence and the code as written.
 */
export function fencedBlocks(text, inComments) {
  const strip = inComments ? (line) => line.replace(/^\s*\* ?/, '') : (line) => line;
  const fence = inComments ? /^\s*\*\s?```\s*(.*)$/ : /^\s*```\s*(.*)$/;
  const blocks = [];
  let open = null;
  text.split('\n').forEach((raw, index) => {
    const match = raw.match(fence);
    if (!match) {
      if (open) open.lines.push(strip(raw));
      return;
    }
    if (open) {
      if (open.language && !open.skip)
        blocks.push({ line: open.line, language: open.language, code: open.lines.join('\n') });
      open = null;
      return;
    }
    const [tag = '', ...flags] = match[1].trim().split(/\s+/);
    open = {
      line: index + 1,
      language: LANGUAGES.get(tag.toLowerCase()),
      skip: flags.includes('no-run'),
      lines: [],
    };
  });
  if (open) throw new Error(`unterminated code fence opened at line ${open.line}`);
  return blocks;
}

/**
 * Every gated sample, as a complete program.
 *
 * @returns {{ source: string, line: number, language: 'ts' | 'js', name: string, code: string }[]}
 *   One entry per block: where it came from, a file-system-safe `name`, and
 *   the program text (with `PRELUDE` in front when the block has no import).
 */
export function collectExamples() {
  return SOURCES.flatMap((source) => {
    const url = new URL(source, PACKAGE);
    const label = source.replace(/^(\.\.\/)+/, '');
    return fencedBlocks(readFileSync(url, 'utf8'), source.endsWith('.d.ts')).map((block) => ({
      source: label,
      line: block.line,
      language: block.language,
      name: `${label.replace(/[^A-Za-z0-9]+/g, '_')}_L${block.line}`,
      code: /^\s*import\s/m.test(block.code) ? block.code : PRELUDE + block.code,
    }));
  });
}

export const PACKAGE_DIR = fileURLToPath(PACKAGE);
