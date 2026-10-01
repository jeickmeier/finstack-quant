import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';
import ts from 'typescript';
import {
  documentationBlocks,
  isLegacyPlaceholderExample,
  LEGACY_CATCH_ALL_THROW,
} from './typescript-docs-shared.mjs';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const option = process.argv.find((value) => value.startsWith('--declaration='));
const declarationPath = option
  ? resolve(process.cwd(), option.slice('--declaration='.length))
  : join(root, 'index.d.ts');
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

function returnIsVoid(node) {
  if (ts.isConstructorDeclaration(node)) {
    return true;
  }
  return node.type?.kind === ts.SyntaxKind.VoidKeyword;
}

function hasPrivateModifier(node) {
  return (
    node.modifiers?.some((modifier) => modifier.kind === ts.SyntaxKind.PrivateKeyword) ?? false
  );
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

  const blocks = documentationBlocks(documentationText);
  if (
    blocks.some(
      (block) => block.lines.join(' ').replace(/\s+/g, ' ').trim() === LEGACY_CATCH_ALL_THROW
    )
  ) {
    error(node, `${label}: contains fabricated catch-all @throws boilerplate`);
  }
  if (blocks.some(isLegacyPlaceholderExample)) {
    error(node, `${label}: contains a non-executable placeholder @example`);
  }

  const joined = (documentationText ?? '').replace(/\s+/g, ' ');
  const genericPhrases = [
    ['units described above', 'generic numeric @returns boilerplate'],
    ['exposed by this', 'generic field summary boilerplate'],
    ['this callable', 'generic parameter boilerplate'],
    ['declared TypeScript shape', 'generic @returns boilerplate'],
    ['units and constraints stated above', 'generic numeric @param boilerplate'],
    ['in the documented order', 'generic collection-order @returns boilerplate'],
    ['TypeScript type that constrains', 'generic type-alias summary boilerplate'],
    ['or WebAssembly handle', 'generic handle @returns boilerplate'],
    ['requested string representation or JSON payload', 'generic JSON @returns boilerplate'],
    ['TypeScript view of the', 'generic interface summary boilerplate'],
    ['consumed by this API', 'generic JSON @param boilerplate'],
    ['consumed by this operation', 'generic @param boilerplate'],
    ['consumed by this calculation', 'generic @param boilerplate'],
    ['documented condition is satisfied', 'generic boolean @returns boilerplate'],
    ['documented condition holds', 'generic boolean @returns boilerplate'],
    ['defaults follow the callable', 'generic optional-flag @param boilerplate'],
    ['follow the type and convention', 'generic @param boilerplate'],
    ['Create a new `', 'generic constructor-summary boilerplate'],
    ['Create the object from its inputs', 'generic constructor-summary boilerplate'],
    ['Structured specification that defines', 'generic spec @param boilerplate'],
    ['JSON-serialized representation accepted by this API', 'generic JSON @param boilerplate'],
    ['Whether to enable', 'generic boolean @param boilerplate'],
    ['on the documented day-count basis', 'generic time @param boilerplate'],
    ['in the documented quote convention', 'generic spot @param boilerplate'],
    ['satisfy this calculation', 'generic market JSON @param boilerplate'],
    ['accepted by this operation', 'generic JSON @param boilerplate'],
    ['units required by this function', 'generic numeric @param boilerplate'],
    ['string consumed by this', 'generic string @param boilerplate'],
    ['input consumed by this', 'generic @param boilerplate'],
    ['Construction and factory entry points', 'generic constructor-interface boilerplate'],
    ['WebAssembly values.', 'generic constructor-interface boilerplate'],
    [
      'Pricing options that select calculation behavior',
      'generic metricPricingOverrides @param boilerplate',
    ],
    [
      'Chronological market snapshots used to project or backtest',
      'generic marketHistory @param boilerplate',
    ],
  ];
  for (const [phrase, message] of genericPhrases) {
    if (joined.includes(phrase)) {
      error(node, `${label}: contains ${message}`);
    }
  }
  // Shapes the completer used to derive from a name or a type alone. They say
  // nothing the signature does not, and the unit templates were wrong (vega and
  // rho are per vol point and per 1bp; theta is per day).
  const placeholderPatterns = [
    [/@param \S+ - [A-Z][\w ]* used by this call\./, 'placeholder @param'],
    [/@param \S+ - [A-Z][\w ]* as a (?:finite number|string)\./, 'placeholder @param'],
    [/@param \S+ - Whether [\w ]+ is true\./, 'placeholder @param'],
    [/@returns JSON string or identifier produced by this call\./, 'placeholder @returns'],
    [/@returns Returns the result of this call\./, 'placeholder @returns'],
    [/@returns Returns the [\w ]+ as a finite number\./, 'placeholder @returns'],
    [/per 1\.0 absolute move in/, 'template Greek unit (state the Rust unit in the Rustdoc)'],
    [/per year of calendar time/, 'template Greek unit (state the Rust unit in the Rustdoc)'],
  ];
  for (const [pattern, message] of placeholderPatterns) {
    if (pattern.test(joined)) {
      error(node, `${label}: contains ${message}`);
    }
  }
  const internalName = joined.match(/\[`Js[A-Z]\w*/);
  if (internalName) {
    error(
      node,
      `${label}: names the internal Rust wrapper ${internalName[0]}\`]; use the public JS name`
    );
  } else {
    const rustLink = joined.match(/\[`[^`\]]+`\]/);
    if (rustLink) {
      error(node, `${label}: contains the rustdoc link ${rustLink[0]}; write the public JS name`);
    }
  }
  const seenParameters = new Set();
  for (const [, name] of joined.matchAll(/@param(?:\s+\{[^}]+\})?\s+([A-Za-z_$][\w$]*)\b/g)) {
    if (seenParameters.has(name)) {
      error(node, `${label}: documents @param \`${name}\` more than once`);
    }
    seenParameters.add(name);
  }

  if (/Perform \S+ for this `/.test(joined)) {
    error(node, `${label}: contains generic method-summary boilerplate`);
  }
  if (/Compute \S+ for this `/.test(joined)) {
    error(node, `${label}: contains generic method-summary boilerplate`);
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

function checkClass(node) {
  const name = declarationName(node);
  checkDocumentedNode(node, `class ${name}`);

  for (const member of node.members) {
    if (hasPrivateModifier(member)) {
      continue;
    }
    if (ts.isConstructorDeclaration(member)) {
      checkDocumentedNode(member, `${name}.constructor`, { callable: true });
    } else if (ts.isMethodDeclaration(member)) {
      checkDocumentedNode(member, `${name}.${declarationName(member)}`, { callable: true });
    } else if (
      ts.isPropertyDeclaration(member) ||
      ts.isGetAccessorDeclaration(member) ||
      ts.isSetAccessorDeclaration(member)
    ) {
      checkDocumentedNode(member, `${name}.${declarationName(member)}`);
    }
  }
}

for (const statement of sourceFile.statements) {
  if (ts.isInterfaceDeclaration(statement) && isExported(statement)) {
    checkInterface(statement);
  } else if (ts.isClassDeclaration(statement)) {
    // `declare class` handles are published through the namespace interfaces.
    checkClass(statement);
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
