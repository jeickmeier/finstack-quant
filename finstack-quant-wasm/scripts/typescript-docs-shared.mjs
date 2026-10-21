export const LEGACY_CATCH_ALL_THROW =
  '@throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.';

/**
 * Remove a leading `/**` and trailing `*\/` with their adjacent whitespace.
 *
 * @param {string} documentationText
 * @returns {string}
 */
export function stripCommentDelimiters(documentationText) {
  let body = documentationText;
  if (body.startsWith('/**')) body = body.slice(3).trimStart();
  if (body.endsWith('*/')) body = body.slice(0, -2).trimEnd();
  return body;
}

/**
 * Undo the raw-string spelling wasm-bindgen gives macro-forwarded doc lines.
 *
 * A `///` line forwarded through a `macro_rules!` macro reaches wasm-bindgen
 * as `#[doc = r" text"]` (or `r#" text"#` when the text holds a quote), and
 * the generated declaration keeps the literal: ` * r" text` and
 * ` * r#" text"#`. Rewrite those comment lines to ` * text`.
 *
 * @param {string} declarationText - Generated `.d.ts` text.
 * @returns {string}
 */
export function stripRawStringDocPrefixes(declarationText) {
  return declarationText
    .replace(/^(\s*\*) r#" ?(.*)"#$/gm, (_, star, text) => (text ? `${star} ${text}` : star))
    .replace(/^(\s*\*) r" ?(.*)$/gm, (_, star, text) => (text ? `${star} ${text}` : star));
}

export function documentationBlocks(documentationText) {
  if (!documentationText) return [];
  const lines = stripCommentDelimiters(documentationText)
    .split('\n')
    .map((line) => line.replace(/^\s*\* ?/, '').trimEnd());
  while (lines.length && !lines[0].trim()) lines.shift();
  while (lines.length && !lines.at(-1)?.trim()) lines.pop();

  const blocks = [];
  let current = { kind: 'prose', lines: [] };
  for (const line of lines) {
    const tag = line.trim().match(/^@(param|returns|throws|example)\b/);
    if (tag) {
      if (current.lines.length) blocks.push(current);
      current = { kind: tag[1], lines: [line] };
    } else {
      current.lines.push(line);
    }
  }
  if (current.lines.length) blocks.push(current);
  return blocks;
}

export function documentationFromBlocks(blocks) {
  if (!blocks.length) return null;
  return `/**\n${blocks
    .flatMap((block) => block.lines)
    .map((line) => ` *${line ? ` ${line}` : ''}`)
    .join('\n')}\n */`;
}

export function isLegacyPlaceholderExample(block) {
  if (block.kind !== 'example') return false;
  const value = block.lines.map((line) => line.trim()).join('\n');
  const common =
    value.startsWith('@example\n```typescript\n') &&
    value.includes('from "finstack-quant-wasm";\nawait init();\n');
  if (!common || !value.endsWith('\n```')) return false;
  return (
    /\nconst api: [A-Za-z_$][\w$]* = [^;]+;\nvoid api;\n```$/.test(value) ||
    /\nconst factory: [A-Za-z_$][\w$]* = [^;]+;\nvoid factory;\n```$/.test(value) ||
    /\n\/\/ Supply the documented arguments to ([A-Za-z_$][\w$]*)\(\.\.\.\) for your use case\.\nvoid \1;\n```$/.test(
      value
    )
  );
}

export function stripLegacyBoilerplate(documentationText) {
  if (!documentationText) return null;
  const blocks = documentationBlocks(documentationText).filter((block) => {
    const normalized = block.lines.join(' ').replace(/\s+/g, ' ').trim();
    return normalized !== LEGACY_CATCH_ALL_THROW && !isLegacyPlaceholderExample(block);
  });
  return documentationFromBlocks(blocks);
}
