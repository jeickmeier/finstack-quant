import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

// A worker method makes one native call, or a chain that only passes the
// previous handle along. This gate flags a worker service that decides from a
// native result: an `if`, `?:`, `??`, `for`/`while` or `.map`-style iteration
// whose condition or iterated value reads one.
//
// "Reads a native result" is a per-file taint over symbols, so a name reused in
// another scope is not tainted:
// - the roots are the service factory's `native` parameter and anything derived
//   from it (`const api = native.statements`);
// - a variable bound or assigned from an expression holding a tainted name is
//   tainted, destructuring included, and so is a parameter of a callback passed
//   to an iteration of a tainted value;
// - a local function that reads a tainted name returns a tainted value.
// A request argument is not tainted, even when it was once a native result.
//
// Exempt without a comment:
// - `tainted ?? null` and `tainted ?? undefined`, which only normalize an absent
//   value and decide nothing;
// - a chain where each step only passes the previous handle along, and a `.map`
//   over user-supplied points, because neither reads a native result in a
//   condition or an iterated value.
//
// A true composition takes `// finstack-compose: <reason>` on or directly above
// the line; a comment with no reason or no hit is an error. A decision moves to
// Rust.

const resolve =
  "move the decision to Rust; for pure composition add " +
  "`// finstack-compose: <reason>` on or directly above the line";

const { SyntaxKind: Kind } = ts;
const iterations = new Set(["map", "flatMap", "forEach", "filter"]);

/** The identifiers a binding pattern declares. */
function bindingIdentifiers(name) {
  if (ts.isIdentifier(name)) return [name];
  return name.elements.flatMap((element) =>
    ts.isOmittedExpression(element) ? [] : bindingIdentifiers(element.name),
  );
}

function iteratedValue(node) {
  if (
    !ts.isCallExpression(node) ||
    !ts.isPropertyAccessExpression(node.expression) ||
    !iterations.has(node.expression.name.text)
  )
    return null;
  return node.expression.expression;
}

/** A checker over one source file alone; imports stay unresolved and untainted. */
function checkerFor(text, file) {
  const options = {
    noLib: true,
    noResolve: true,
    target: ts.ScriptTarget.Latest,
  };
  const source = ts.createSourceFile(
    file,
    text,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TS,
  );
  const host = {
    ...ts.createCompilerHost(options),
    getSourceFile: (name) => (name === file ? source : undefined),
    fileExists: (name) => name === file,
    readFile: (name) => (name === file ? text : undefined),
  };
  const program = ts.createProgram({ rootNames: [file], options, host });
  return { source, checker: program.getTypeChecker() };
}

/** Every symbol in the file that holds a native value, to a fixed point. */
function taintedSymbols(source, checker) {
  const tainted = new Set();
  const symbolOf = (identifier) =>
    ts.isShorthandPropertyAssignment(identifier.parent)
      ? checker.getShorthandAssignmentValueSymbol(identifier.parent)
      : checker.getSymbolAtLocation(identifier);
  const reads = (node) => {
    if (ts.isIdentifier(node)) return tainted.has(symbolOf(node));
    return ts.forEachChild(node, reads) ?? false;
  };
  let grew = true;
  const add = (identifiers) => {
    for (const identifier of identifiers) {
      const symbol = symbolOf(identifier);
      if (symbol && !tainted.has(symbol)) {
        tainted.add(symbol);
        grew = true;
      }
    }
  };
  const visit = (node) => {
    if (
      ts.isParameter(node) &&
      ts.isIdentifier(node.name) &&
      node.name.text === "native"
    )
      add([node.name]);
    if (
      ts.isVariableDeclaration(node) &&
      node.initializer &&
      reads(node.initializer)
    )
      add(bindingIdentifiers(node.name));
    if (
      ts.isBinaryExpression(node) &&
      node.operatorToken.kind === Kind.EqualsToken &&
      ts.isIdentifier(node.left) &&
      reads(node.right)
    )
      add([node.left]);
    if (
      ts.isFunctionDeclaration(node) &&
      node.name &&
      node.body &&
      reads(node.body)
    )
      add([node.name]);
    const iterated = iteratedValue(node);
    if (iterated && reads(iterated)) {
      const callback = node.arguments[0];
      if (callback && ts.isFunctionLike(callback))
        add(callback.parameters.flatMap((p) => bindingIdentifiers(p.name)));
    }
    if (
      ts.isForOfStatement(node) &&
      reads(node.expression) &&
      ts.isVariableDeclarationList(node.initializer)
    )
      add(
        node.initializer.declarations.flatMap((d) =>
          bindingIdentifiers(d.name),
        ),
      );
    ts.forEachChild(node, visit);
  };
  while (grew) {
    grew = false;
    visit(source);
  }
  return reads;
}

function isAbsent(node) {
  return (
    node.kind === Kind.NullKeyword ||
    (ts.isIdentifier(node) && node.text === "undefined")
  );
}

/** The decision `node` makes from a native result, or null. */
function decision(node, reads) {
  if (ts.isIfStatement(node) && reads(node.expression)) return "if";
  if (ts.isConditionalExpression(node) && reads(node.condition)) return "?:";
  if (
    ts.isBinaryExpression(node) &&
    node.operatorToken.kind === Kind.QuestionQuestionToken &&
    reads(node.left) &&
    !isAbsent(node.right)
  )
    return "??";
  if (ts.isForOfStatement(node) && reads(node.expression)) return "for...of";
  if (ts.isForInStatement(node) && reads(node.expression)) return "for...in";
  if (ts.isForStatement(node) && node.condition && reads(node.condition))
    return "for";
  if (ts.isWhileStatement(node) && reads(node.expression)) return "while";
  if (ts.isDoStatement(node) && reads(node.expression)) return "do...while";
  const iterated = iteratedValue(node);
  if (iterated && reads(iterated)) return `.${node.expression.name.text}`;
  return null;
}

/**
 * Check one worker service source.
 * @param text - Source text.
 * @param file - Path shown in messages.
 * @returns `hits` is one `{ line, kind, text }` per flagged line, comment-covered
 * or not, `covered` the number of those a comment explains, and `errors` the
 * messages for uncovered hits and bad comments.
 */
export function scanSource(text, file) {
  const { source, checker } = checkerFor(text, file);
  const reads = taintedSymbols(source, checker);
  const hits = new Map();
  const visit = (node) => {
    const kind = decision(node, reads);
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
    const match = /\/\/\s*finstack-compose:(.*)$/.exec(content);
    if (!match) return;
    const line = index + 1;
    if (!match[1].trim()) {
      errors.push(
        `${file}:${line}: finstack-compose comment has no reason; write \`// finstack-compose: <reason>\``,
      );
      // A reasonless comment explains nothing, so its line stays uncovered.
      return;
    }
    const targets = [line, line + 1].filter((target) => hits.has(target));
    if (targets.length === 0)
      errors.push(
        `${file}:${line}: finstack-compose comment covers no decision on a native result; delete it`,
      );
    for (const target of targets) covered.add(target);
  });
  for (const hit of hits.values())
    if (!covered.has(hit.line))
      errors.push(
        `${file}:${hit.line}: ${hit.kind} reads a native result in \`${hit.text}\`; ${resolve}`,
      );
  return { hits: [...hits.values()], covered: covered.size, errors };
}

const services = "registry/shared/file/services";

/** Read the worker services under `root` and return the gate's error messages. */
export async function checkWorkerCompose(root) {
  const errors = [];
  for (const entry of await readdir(path.join(root, services), {
    recursive: true,
    withFileTypes: true,
  })) {
    if (!entry.isFile() || !entry.name.endsWith(".ts")) continue;
    const absolute = path.join(entry.parentPath, entry.name);
    const file = path.relative(root, absolute).split(path.sep).join("/");
    errors.push(...scanSource(await readFile(absolute, "utf8"), file).errors);
  }
  return errors;
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const errors = await checkWorkerCompose(
    fileURLToPath(new URL("../", import.meta.url)),
  );
  if (errors.length > 0) {
    console.error(errors.join("\n"));
    process.exit(1);
  }
  console.log("Checked the worker services for decisions on native results.");
}
