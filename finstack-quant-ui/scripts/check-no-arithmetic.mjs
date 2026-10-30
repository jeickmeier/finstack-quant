import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

// The UI draws what Rust computed. This gate flags arithmetic in `registry/**`
// and `src/**`: binary and compound `* / - + % **`, prefix minus on a
// non-literal, `Math.*` calls, `.reduce` and Big operations.
//
// Exempt without a comment: string concatenation (an operand is visibly a
// string) and a binary whose direct operand is `.length` or `.size`; a nested
// expression such as `row * list.length + column` is still flagged on its
// outer `+`. Everything else needs one of two explanations:
// - a path glob with a reason in `no-arithmetic-allow.json`, for chart, form
//   and codec mechanics; a glob that excludes no flagged line is an error;
// - `// finstack-arith: <reason>` on or directly above the line (`{/* ... */}`
//   among JSX children); it covers every hit on the line, and a comment with
//   no reason or no hit is an error.
// Financial logic gets neither: it moves to Rust.

const allowFile = "scripts/no-arithmetic-allow.json";
const resolve =
  `move financial logic to Rust; for layout or mechanics add \`// finstack-arith: <reason>\` ` +
  `on or directly above the line, or a glob with a reason in ${allowFile}`;

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
// `sub` and `mul` are big.js aliases of `minus` and `times`.
const bigOperations = new Set([
  "plus",
  "minus",
  "times",
  "div",
  "abs",
  "pow",
  "sub",
  "mul",
]);

const unwrap = (node) =>
  ts.isParenthesizedExpression(node) ? unwrap(node.expression) : node;

function isStringLike(expression) {
  const node = unwrap(expression);
  if (ts.isStringLiteralLike(node) || ts.isTemplateExpression(node))
    return true;
  if (ts.isConditionalExpression(node))
    return isStringLike(node.whenTrue) || isStringLike(node.whenFalse);
  return (
    ts.isBinaryExpression(node) &&
    node.operatorToken.kind === Kind.PlusToken &&
    (isStringLike(node.left) || isStringLike(node.right))
  );
}

function isCount(expression) {
  const node = unwrap(expression);
  return (
    ts.isPropertyAccessExpression(node) &&
    (node.name.text === "length" || node.name.text === "size")
  );
}

function arithmeticKind(node) {
  if (ts.isBinaryExpression(node)) {
    const { left, operatorToken, right } = node;
    if (
      !arithmeticOperators.has(operatorToken.kind) ||
      isStringLike(left) ||
      isStringLike(right) ||
      isCount(left) ||
      isCount(right)
    )
      return null;
    return `operator ${operatorToken.getText()}`;
  }
  if (ts.isPrefixUnaryExpression(node))
    return node.operator === Kind.MinusToken &&
      !ts.isNumericLiteral(node.operand) &&
      !ts.isBigIntLiteral(node.operand)
      ? "prefix minus"
      : null;
  if (ts.isNewExpression(node))
    return node.expression.getText() === "Big" ? "new Big" : null;
  if (!ts.isCallExpression(node)) return null;
  const callee = node.expression;
  if (ts.isIdentifier(callee)) return callee.text === "Big" ? "Big()" : null;
  if (!ts.isPropertyAccessExpression(callee)) return null;
  const method = callee.name.text;
  if (callee.expression.getText() === "Math") return `Math.${method}`;
  if (method === "reduce" || method === "reduceRight") return `.${method}`;
  return bigOperations.has(method) ? `Big .${method}` : null;
}

function scriptKind(file) {
  if (file.endsWith(".tsx")) return ts.ScriptKind.TSX;
  return /\.[cm]?ts$/.test(file) ? ts.ScriptKind.TS : ts.ScriptKind.JSX;
}

/**
 * Check one source file.
 * @param text - Source text.
 * @param file - Path shown in messages; its extension selects the parser.
 * @returns `hits` is one `{ line, kind, text }` per flagged line, comment-covered
 * or not; `errors` is the messages for uncovered hits and bad comments.
 */
export function scanSource(text, file) {
  const source = ts.createSourceFile(
    file,
    text,
    ts.ScriptTarget.Latest,
    true,
    scriptKind(file),
  );
  const hits = new Map();
  const visit = (node) => {
    const kind = arithmeticKind(node);
    if (kind) {
      const line =
        source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1;
      if (!hits.has(line))
        hits.set(line, {
          line,
          kind,
          text: node.getText(source).replace(/\s+/g, " ").slice(0, 80),
        });
    }
    ts.forEachChild(node, visit);
  };
  visit(source);

  const errors = [];
  const covered = new Set();
  text.split("\n").forEach((content, index) => {
    const match = /(?:\/\/|\/\*)\s*finstack-arith:(.*)$/.exec(content);
    if (!match) return;
    const line = index + 1;
    if (!match[1].replace("*/", "").replace("}", "").trim()) {
      errors.push(
        `${file}:${line}: finstack-arith comment has no reason; write \`// finstack-arith: <reason>\``,
      );
      // A reasonless comment explains nothing, so its line stays uncovered.
      return;
    }
    const targets = [line, line + 1].filter((target) => hits.has(target));
    if (targets.length === 0)
      errors.push(
        `${file}:${line}: finstack-arith comment covers no flagged arithmetic; delete it`,
      );
    for (const target of targets) covered.add(target);
  });
  for (const hit of hits.values())
    if (!covered.has(hit.line))
      errors.push(
        `${file}:${hit.line}: ${hit.kind} in \`${hit.text}\`; ${resolve}`,
      );
  return { hits: [...hits.values()], errors };
}

/** Compile a path glob: `**` spans directories, `*` stays inside one. */
export function globToRegExp(glob) {
  const pattern = glob
    .replace(/[.+^${}()|[\]\\]/g, "\\$&")
    .replace(/\*\*\/?|\*/g, (star) => (star === "*" ? "[^/]*" : ".*"));
  return new RegExp(`^${pattern}$`);
}

/**
 * Check a set of files against the allow-list.
 * @param files - `{ file, text }` with paths relative to the package root.
 * @param allow - Path glob to the reason it is excluded.
 * @returns Every error message; empty when the gate passes.
 */
export function checkSources(files, allow) {
  const globs = Object.entries(allow).map(([glob, reason]) => ({
    glob,
    reason,
    pattern: globToRegExp(glob),
    used: false,
  }));
  const errors = [];
  for (const { file, text } of files) {
    const { hits, errors: found } = scanSource(text, file);
    const excluding = globs.filter(({ pattern }) => pattern.test(file));
    if (excluding.length === 0) errors.push(...found);
    else if (hits.length > 0) for (const glob of excluding) glob.used = true;
  }
  for (const { glob, reason, used } of globs) {
    if (typeof reason !== "string" || !reason.trim())
      errors.push(`${allowFile}: "${glob}" has no reason; give one`);
    if (!used)
      errors.push(
        `${allowFile}: "${glob}" excludes no flagged arithmetic; delete it`,
      );
  }
  return errors;
}

const scanned = /\.[cm]?[jt]sx?$/;
const excluded =
  /^src\/generated\/|(^|\/)tests?\/|\.test\.|\.d\.[cm]?ts$|\/examples\/|(^|\/)components\/ui\//;

/** Read the scanned tree under `root` and return the gate's error messages. */
export async function checkNoArithmetic(root) {
  const files = [];
  for (const directory of ["registry", "src"])
    for (const entry of await readdir(path.join(root, directory), {
      recursive: true,
      withFileTypes: true,
    })) {
      const absolute = path.join(entry.parentPath, entry.name);
      const file = path.relative(root, absolute).split(path.sep).join("/");
      if (entry.isFile() && scanned.test(entry.name) && !excluded.test(file))
        files.push({ file, text: await readFile(absolute, "utf8") });
    }
  return checkSources(
    files,
    JSON.parse(await readFile(path.join(root, allowFile), "utf8")),
  );
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const errors = await checkNoArithmetic(
    fileURLToPath(new URL("../", import.meta.url)),
  );
  if (errors.length > 0) {
    console.error(errors.join("\n"));
    process.exit(1);
  }
  console.log("Checked registry and src for arithmetic outside Rust.");
}
