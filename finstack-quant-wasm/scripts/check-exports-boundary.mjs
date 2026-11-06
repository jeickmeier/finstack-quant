import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';

// The facade re-exports; only Rust computes. Run by `mise run wasm-lint`, this
// gate over `exports/**` fails on arithmetic (binary and compound operators
// except string concatenation, prefix minus on a non-literal, `Math.*`,
// `.reduce`), and on a function whose statement or object property has no
// leading comment saying what wasm-bindgen cannot express. A function nested
// inside a commented statement shares its comment.

const { SyntaxKind: Kind } = ts;
const arithmeticOperators = new Set([
  Kind.AsteriskToken,
  Kind.SlashToken,
  Kind.MinusToken,
  Kind.PlusToken,
  Kind.PercentToken,
  Kind.AsteriskAsteriskToken,
  Kind.AsteriskEqualsToken,
  Kind.SlashEqualsToken,
  Kind.MinusEqualsToken,
  Kind.PlusEqualsToken,
  Kind.PercentEqualsToken,
  Kind.AsteriskAsteriskEqualsToken,
]);
const reason = /wasm-bindgen[\s\S]*?\b(?:cannot|can't)\b/;

const isStringLike = (node) =>
  ts.isStringLiteralLike(node) ||
  ts.isTemplateExpression(node) ||
  (ts.isBinaryExpression(node) &&
    node.operatorToken.kind === Kind.PlusToken &&
    (isStringLike(node.left) || isStringLike(node.right)));

function arithmeticKind(node) {
  if (ts.isBinaryExpression(node))
    return arithmeticOperators.has(node.operatorToken.kind) &&
      !isStringLike(node.left) &&
      !isStringLike(node.right)
      ? `operator ${node.operatorToken.getText()}`
      : null;
  if (ts.isPrefixUnaryExpression(node))
    return node.operator === Kind.MinusToken && !ts.isNumericLiteral(node.operand)
      ? 'prefix minus'
      : null;
  if (!ts.isCallExpression(node) || !ts.isPropertyAccessExpression(node.expression)) return null;
  const { expression: owner, name } = node.expression;
  if (owner.getText() === 'Math') return `Math.${name.text}`;
  return name.text === 'reduce' || name.text === 'reduceRight' ? `.${name.text}` : null;
}

const isFunction = (node) =>
  ts.isFunctionDeclaration(node) ||
  ts.isFunctionExpression(node) ||
  ts.isArrowFunction(node) ||
  ts.isMethodDeclaration(node);

function anchorOf(node) {
  let anchor = node;
  while (!ts.isStatement(anchor) && !ts.isPropertyAssignment(anchor) && anchor.parent)
    anchor = anchor.parent;
  return anchor;
}

const leadingComment = (text, node) =>
  (ts.getLeadingCommentRanges(text, node.getFullStart()) ?? [])
    .map(({ pos, end }) => text.slice(pos, end))
    .join('\n');

/**
 * Check one facade module.
 * @param text - JavaScript source text.
 * @param file - Path shown in messages.
 * @returns One `file:line: problem; fix` message per violation; empty when clean.
 */
export function checkExportsSource(text, file) {
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  const lineOf = (node) => source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1;
  const errors = [];
  const visit = (node, inFunction) => {
    const kind = arithmeticKind(node);
    if (kind)
      errors.push(
        `${file}:${lineOf(node)}: ${kind} in \`${node.getText(source)}\`; the facade only re-exports, so move the computation to Rust`
      );
    const startsFunction = isFunction(node) && !inFunction;
    if (startsFunction && !reason.test(leadingComment(text, anchorOf(node))))
      errors.push(
        `${file}:${lineOf(node)}: function with no comment saying what wasm-bindgen cannot express; add one directly above the statement or property, or bind it in Rust and re-export it`
      );
    ts.forEachChild(node, (child) => visit(child, inFunction || startsFunction));
  };
  visit(source, false);
  return errors;
}

/** Read every module under `exportsRoot` and return the gate's messages. */
export async function checkExports(exportsRoot, base = exportsRoot) {
  const entries = await readdir(exportsRoot, { recursive: true, withFileTypes: true });
  const files = entries
    .filter((entry) => entry.isFile() && /\.[cm]?js$/.test(entry.name))
    .map((entry) => path.join(entry.parentPath, entry.name))
    .sort();
  const errors = [];
  for (const file of files)
    errors.push(...checkExportsSource(await readFile(file, 'utf8'), path.relative(base, file)));
  return errors;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const packageRoot = fileURLToPath(new URL('../', import.meta.url));
  const errors = await checkExports(path.join(packageRoot, 'exports'), packageRoot);
  if (errors.length > 0) {
    console.error(errors.join('\n'));
    process.exit(1);
  }
  console.log('Checked exports/ for arithmetic and unexplained functions.');
}
