import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import ts from 'typescript';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const declarationPath = join(root, 'index.d.ts');
const sourceText = readFileSync(declarationPath, 'utf8');
const sourceFile = ts.createSourceFile(
  declarationPath,
  sourceText,
  ts.ScriptTarget.Latest,
  true,
  ts.ScriptKind.TS
);
const maxErrors = Number.parseInt(
  process.argv.find((value) => value.startsWith('--max-errors='))?.split('=')[1] ?? '200',
  10
);
const failures = [];

function declarationName(node) {
  if ('name' in node && node.name) {
    return node.name.getText(sourceFile);
  }
  return '<anonymous>';
}

function lineOf(node) {
  return sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile)).line + 1;
}

function documentation(node) {
  const ranges = ts.getLeadingCommentRanges(sourceText, node.getFullStart()) ?? [];
  const range = [...ranges].reverse().find(({ pos }) => sourceText.startsWith('/**', pos));
  return range ? sourceText.slice(range.pos, range.end) : null;
}

function summary(documentationText) {
  if (!documentationText) {
    return null;
  }
  const lines = documentationText
    .replace(/^\/\*\*|\*\/$/g, '')
    .split('\n')
    .map((line) => line.replace(/^\s*\* ?/, '').trim());
  return lines.find((line) => line && !line.startsWith('@')) ?? null;
}

function parameterTags(documentationText) {
  if (!documentationText) {
    return new Set();
  }
  return new Set(
    [...documentationText.matchAll(/@param(?:\s+\{[^}]+\})?\s+([A-Za-z_$][\w$]*)\b/g)].map(
      (match) => match[1]
    )
  );
}

function hasTag(documentationText, tag) {
  return documentationText?.includes(`@${tag}`) ?? false;
}

function hasNonThrowingBehavior(documentationText) {
  return /does not throw|never throws|non-throwing/i.test(documentationText ?? '');
}

function returnIsVoid(node) {
  return node.type?.kind === ts.SyntaxKind.VoidKeyword;
}

function error(node, message) {
  failures.push(`${declarationPath}:${lineOf(node)}: ${message}`);
}

function checkDocumentedNode(node, label, options = {}) {
  const documentationText = documentation(node);
  const nodeSummary = summary(documentationText);
  if (!nodeSummary || nodeSummary.length < 16) {
    error(node, `${label}: missing substantive JSDoc summary`);
  }

  if (options.example && !hasTag(documentationText, 'example')) {
    error(node, `${label}: missing @example`);
  }

  if (!options.callable) {
    return;
  }

  const documentedParameters = parameterTags(documentationText);
  for (const parameter of node.parameters ?? []) {
    const name = parameter.name.getText(sourceFile);
    if (!documentedParameters.has(name)) {
      error(node, `${label}: missing @param for \`${name}\``);
    }
  }

  if (!returnIsVoid(node) && !hasTag(documentationText, 'returns')) {
    error(node, `${label}: missing @returns`);
  }

  if (
    (node.parameters?.length ?? 0) > 0 &&
    !hasTag(documentationText, 'throws') &&
    !hasNonThrowingBehavior(documentationText)
  ) {
    error(node, `${label}: document thrown errors or state that the operation does not throw`);
  }
}

function isExported(node) {
  return node.modifiers?.some((modifier) => modifier.kind === ts.SyntaxKind.ExportKeyword) ?? false;
}

function checkInterface(node) {
  const name = declarationName(node);
  const requiresExample = name.endsWith('Namespace') || name.endsWith('Constructor');
  checkDocumentedNode(node, `interface ${name}`, { example: requiresExample });

  for (const member of node.members) {
    if (
      ts.isMethodSignature(member) ||
      ts.isConstructSignatureDeclaration(member) ||
      ts.isCallSignatureDeclaration(member)
    ) {
      checkDocumentedNode(member, `${name}.${declarationName(member)}`, { callable: true });
    } else if (ts.isPropertySignature(member)) {
      checkDocumentedNode(member, `${name}.${declarationName(member)}`);
    }
  }
}

for (const statement of sourceFile.statements) {
  if (ts.isInterfaceDeclaration(statement) && isExported(statement)) {
    checkInterface(statement);
  } else if (ts.isTypeAliasDeclaration(statement) && isExported(statement)) {
    checkDocumentedNode(statement, `type ${declarationName(statement)}`);
  } else if (ts.isFunctionDeclaration(statement) && isExported(statement)) {
    checkDocumentedNode(statement, `function ${declarationName(statement)}`, {
      callable: true,
      example: true,
    });
  } else if (ts.isVariableStatement(statement) && isExported(statement)) {
    for (const declaration of statement.declarationList.declarations) {
      checkDocumentedNode(statement, `constant ${declarationName(declaration)}`);
    }
  }
}

if (failures.length > 0) {
  if (process.argv.includes('--summary')) {
    const summary = new Map();
    for (const failure of failures) {
      const category = failure.slice(failure.lastIndexOf(': ') + 2);
      summary.set(category, (summary.get(category) ?? 0) + 1);
    }
    for (const [category, count] of [...summary.entries()].sort(
      (left, right) => right[1] - left[1]
    )) {
      console.error(`${count}\t${category}`);
    }
  }
  for (const failure of failures.slice(0, maxErrors)) {
    console.error(failure);
  }
  if (failures.length > maxErrors) {
    console.error(`... ${failures.length - maxErrors} additional documentation errors omitted`);
  }
  console.error(`TypeScript facade documentation: ${failures.length} error(s)`);
  process.exit(1);
}

console.log('TypeScript facade documentation: clean');
