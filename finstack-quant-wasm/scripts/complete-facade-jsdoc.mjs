import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';
import ts from 'typescript';
import { stripLegacyBoilerplate } from './typescript-docs-shared.mjs';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const option = process.argv.find((value) => value.startsWith('--declaration='));
const declarationPath = option
  ? resolve(process.cwd(), option.slice('--declaration='.length))
  : join(root, 'index.d.ts');
const write = process.argv.includes('--write');
const check = process.argv.includes('--check');
if (write && check) {
  console.error('--write and --check are mutually exclusive');
  process.exit(2);
}
const sourceText = readFileSync(declarationPath, 'utf8');
const sourceFile = ts.createSourceFile(
  declarationPath,
  sourceText,
  ts.ScriptTarget.Latest,
  true,
  ts.ScriptKind.TS
);

function leadingJsdoc(text, node, file) {
  const start = node.getStart(file, false);
  const prefix = text.slice(0, start);
  const commentStart = prefix.lastIndexOf('/**');
  if (commentStart < 0) return null;
  const candidate = prefix.slice(commentStart);
  const commentEnd = candidate.indexOf('*/');
  if (commentEnd < 0 || candidate.slice(commentEnd + 2).trim()) return null;
  return {
    text: candidate.slice(0, commentEnd + 2),
    start: commentStart,
    end: commentStart + commentEnd + 2,
  };
}

function isExported(node) {
  return node.modifiers?.some((modifier) => modifier.kind === ts.SyntaxKind.ExportKeyword) ?? false;
}

function nodeName(node) {
  if (ts.isConstructorDeclaration(node) || ts.isConstructSignatureDeclaration(node))
    return 'constructor';
  if ('name' in node && node.name) return node.name.getText(sourceFile);
  return 'value';
}

function humanize(name) {
  return name
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .replace(/_/g, ' ')
    .replace(/\bJson\b/g, 'JSON')
    .replace(/\bApi\b/g, 'API')
    .replace(/\bFx\b/g, 'FX')
    .replace(/\bPv\b/g, 'PV')
    .replace(/\bPnl\b/g, 'P&L')
    .toLowerCase();
}

function capitalize(value) {
  return `${value.slice(0, 1).toUpperCase()}${value.slice(1)}`;
}

function summary(documentationText) {
  if (!documentationText) return null;
  const lines = documentationText
    .replace(/^\/\*\*|\*\/$/g, '')
    .split('\n')
    .map((line) => line.replace(/^\s*\* ?/, '').trim());
  return lines.find((line) => line && !line.startsWith('@')) ?? null;
}

function hasTag(documentationText, tag) {
  return documentationText?.includes(`@${tag}`) ?? false;
}

function documentedParameters(documentationText) {
  return new Set(
    [...(documentationText ?? '').matchAll(/@param(?:\s+\{[^}]+\})?\s+([A-Za-z_$][\w$]*)\b/g)].map(
      (match) => match[1]
    )
  );
}

function insertSummary(documentationText, value) {
  if (!documentationText) return `/**\n * ${value}\n */`;
  const lines = documentationText.split('\n');
  for (let index = 0; index < lines.length; index += 1) {
    const content = lines[index].replace(/^\s*\* ?/, '').trim();
    if (!content || content.startsWith('@') || content === '/**' || content === '*/') continue;
    lines[index] = lines[index].replace(content, value);
    return lines.join('\n');
  }
  lines.splice(1, 0, ` * ${value}`);
  return lines.join('\n');
}

function appendTags(documentationText, tags) {
  if (!tags.length) return documentationText;
  if (!documentationText.includes('\n')) {
    const summaryText = documentationText.replace(/^\/\*\*\s*|\s*\*\/$/g, '').trim();
    return `/**\n * ${summaryText}\n${tags.map((tag) => ` * ${tag}\n`).join('')} */`;
  }
  return documentationText.replace(/\*\/$/, `${tags.map((tag) => ` * ${tag}\n`).join('')} */`);
}

function documentationIndent(node) {
  return ts.isInterfaceDeclaration(node.parent) || ts.isClassDeclaration(node.parent) ? '  ' : '';
}

function isPrivateMember(node) {
  return (
    node.modifiers?.some((modifier) => modifier.kind === ts.SyntaxKind.PrivateKeyword) ?? false
  );
}

function formatDocumentation(documentationText, node) {
  const indent = documentationIndent(node);
  const body = documentationText
    .replace(/^\/\*\*\s*|\s*\*\/$/g, '')
    .split('\n')
    .map((line) => line.replace(/^\s*\* ?/, '').trimEnd());
  while (body.length && !body[0].trim()) body.shift();
  while (body.length && !body.at(-1)?.trim()) body.pop();
  return [
    '/**',
    ...body.map((line) => `${indent} *${line ? ` ${line}` : ''}`),
    `${indent} */`,
  ].join('\n');
}

function firstSummarySentence(existingSummary) {
  if (!existingSummary) return null;
  const first = existingSummary
    .split('\n')
    .map((line) => line.trim())
    .find((line) => line && !line.startsWith('@'));
  if (!first) return null;
  return first.replace(/\.$/, '');
}

// A number-returning member with no `@returns` repeats its own summary. The
// completer never derives units or conventions from a method name: those are
// written in the Rustdoc and reach this file through `sync-facade-jsdoc`.
function summaryReturnDescription(existingSummary) {
  const summary = firstSummarySentence(existingSummary);
  return summary ? `${summary}.` : null;
}

function returnDescription(type, name, existingSummary) {
  if (!type) return 'Returns the result of this call.';
  const text = type.getText(sourceFile);
  const method = name || '';
  if (text === 'number') return summaryReturnDescription(existingSummary);
  if (text === 'boolean') {
    const summary = firstSummarySentence(existingSummary);
    if (summary) return `${summary}.`;
    return 'Returns `true` or `false`.';
  }
  if (text === 'string') {
    if (method === 'toString') return 'Human-readable string form of this value.';
    return summaryReturnDescription(existingSummary);
  }
  if (text === 'bigint') return 'Integer count produced by this call.';
  if (/^Promise<(.+)>$/.test(text))
    return `Returns a Promise that resolves to \`${text.slice(8, -1)}\`.`;
  if (/^(Float|Uint|Int)\d+Array$/.test(text))
    return `Returns a \`${text}\` of results aligned with the input order.`;
  if (text.endsWith('[]')) return `Returns a \`${text}\` aligned with the input order.`;
  if (/^Record</.test(text) || text.startsWith('{'))
    return `Returns a structured \`${text}\` object.`;
  if (text === 'T') return 'A typed instrument handle.';
  if (/^[A-Za-z_$][\w$]*(?:<.*>)?$/.test(text)) {
    if (
      /(?:Json|Result|Envelope|Stats)$/.test(text) ||
      text === 'DatedSeries' ||
      text === 'LevelsAtDate' ||
      text === 'PeriodDecomposition' ||
      text === 'LookbackReturns'
    ) {
      return `Returns a structured \`${text}\` object.`;
    }
    return `Returns a \`${text}\` handle.`;
  }
  const summary = firstSummarySentence(existingSummary);
  if (summary) return `${summary}.`;
  return `Returns a \`${text}\` result.`;
}

// Parameters whose meaning is fixed by the facade itself, not by the callable.
// Every other undocumented parameter is left without an `@param`, which
// `check-typescript-docs.mjs` reports: its description belongs in the Rustdoc.
const parameterDescriptions = new Map([
  [
    'moduleOrPath',
    'Optional module source: a URL, Response, WebAssembly.Module, or Promise accepted by wasm-bindgen initialization.',
  ],
]);

function parameterDescription(parameter) {
  return parameterDescriptions.get(parameter.name.getText(sourceFile)) ?? null;
}

function classNameFor(interfaceName) {
  return interfaceName.replace(/Constructor$/, '').replace(/Namespace$/, '');
}

function nodeSummary(node, interfaceName) {
  const name = nodeName(node);
  const className = classNameFor(interfaceName ?? 'value');
  if (ts.isPropertySignature(node)) {
    const description = parameterDescriptions.get(name);
    if (description) return `${capitalize(description)}`;
    if (name === 'prototype')
      return `JavaScript prototype of the \`${className}\` class; construct instances with \`new\` or the named factories.`;
    return `${capitalize(humanize(name))} of this \`${className}\`.`;
  }
  if (name === 'constructor') return `Construct a \`${className}\` handle.`;
  if (name === 'toJson') return `Serialize this \`${className}\` value to canonical JSON.`;
  if (name === 'fromJson') return `Parse a \`${className}\` value from canonical JSON.`;
  if (name === 'toString')
    return `Return the human-readable representation of this \`${className}\` value.`;
  if (name === 'price') return `Price this \`${className}\` against the supplied market.`;
  if (name === 'greeks') return `Named first-order greeks produced by the selected model.`;
  if (name.startsWith('with'))
    return `Return a copy of this \`${className}\` with ${humanize(name.slice(4))} configured.`;
  if (interfaceName?.endsWith('Constructor'))
    return `Return a \`${className}\` handle configured for ${humanize(name)}.`;
  return `${capitalize(humanize(name))} for this \`${className}\`.`;
}

function interfaceSummary(name) {
  if (name === 'WasmOwned')
    return 'Lifecycle contract for a WebAssembly-backed value that owns a wasm heap allocation.';
  if (name.endsWith('Namespace'))
    return `Namespaced TypeScript entry points for ${humanize(classNameFor(name))} calculations and types.`;
  if (name.endsWith('Constructor'))
    return `Constructors and factories for \`${classNameFor(name)}\` handles.`;
  if (name.endsWith('Options')) return `Named options for constructing a \`${name.slice(0, -7)}\`.`;
  if (/(?:Json|Result|Envelope)$/.test(name))
    return `Structured result object returned as \`${name}\`.`;
  return `WebAssembly handle for a \`${name}\`.`;
}

function typeSummary(name) {
  return `Accepted ${humanize(name)} values.`;
}

function completeDocumentation(node, interfaceName) {
  let documentationText = stripLegacyBoilerplate(
    leadingJsdoc(sourceText, node, sourceFile)?.text ?? null
  );
  const existingSummary = summary(documentationText);
  const defaultSummary = ts.isInterfaceDeclaration(node)
    ? interfaceSummary(node.name.text)
    : ts.isClassDeclaration(node)
      ? `WebAssembly handle for a \`${node.name.text}\`.`
      : ts.isTypeAliasDeclaration(node)
        ? typeSummary(node.name.text)
        : nodeSummary(node, interfaceName);
  if (!existingSummary || existingSummary.length < 16) {
    documentationText = insertSummary(documentationText, defaultSummary);
  }

  const tags = [];
  if ('parameters' in node) {
    const parameters = documentedParameters(documentationText);
    for (const parameter of node.parameters ?? []) {
      const name = parameter.name.getText(sourceFile);
      const description = parameters.has(name) ? null : parameterDescription(parameter);
      if (description) tags.push(`@param ${name} - ${description}`);
    }
    const skipReturns =
      ts.isConstructorDeclaration(node) || node.type?.kind === ts.SyntaxKind.VoidKeyword;
    if (!skipReturns && !hasTag(documentationText, 'returns')) {
      const description = returnDescription(
        node.type,
        nodeName(node),
        existingSummary && existingSummary.length >= 16 ? existingSummary : defaultSummary
      );
      if (description) tags.push(`@returns ${description}`);
    }
  }
  return appendTags(documentationText, tags);
}

const replacements = [];
for (const statement of sourceFile.statements) {
  if (ts.isInterfaceDeclaration(statement) && isExported(statement)) {
    for (const node of [statement, ...statement.members]) {
      if (
        node !== statement &&
        !ts.isMethodSignature(node) &&
        !ts.isConstructSignatureDeclaration(node) &&
        !ts.isCallSignatureDeclaration(node) &&
        !ts.isPropertySignature(node)
      )
        continue;
      const documentation = formatDocumentation(
        completeDocumentation(node, statement.name.text),
        node
      );
      const existing = leadingJsdoc(sourceText, node, sourceFile);
      if (documentation !== existing?.text) {
        replacements.push({
          start: existing?.start ?? node.getStart(sourceFile, false),
          end: existing?.end ?? node.getStart(sourceFile, false),
          text: existing ? documentation : `${documentation}\n${documentationIndent(node)}`,
        });
      }
    }
  } else if (ts.isClassDeclaration(statement) && isExported(statement)) {
    for (const node of [statement, ...statement.members]) {
      if (isPrivateMember(node)) continue;
      if (
        node !== statement &&
        !ts.isConstructorDeclaration(node) &&
        !ts.isMethodDeclaration(node) &&
        !ts.isPropertyDeclaration(node) &&
        !ts.isGetAccessorDeclaration(node) &&
        !ts.isSetAccessorDeclaration(node)
      )
        continue;
      const documentation = formatDocumentation(
        completeDocumentation(node, statement.name.text),
        node
      );
      const existing = leadingJsdoc(sourceText, node, sourceFile);
      if (documentation !== existing?.text) {
        replacements.push({
          start: existing?.start ?? node.getStart(sourceFile, false),
          end: existing?.end ?? node.getStart(sourceFile, false),
          text: existing ? documentation : `${documentation}\n${documentationIndent(node)}`,
        });
      }
    }
  } else if (ts.isTypeAliasDeclaration(statement) && isExported(statement)) {
    const documentation = formatDocumentation(completeDocumentation(statement, null), statement);
    const existing = leadingJsdoc(sourceText, statement, sourceFile);
    if (documentation !== existing?.text) {
      replacements.push({
        start: existing?.start ?? statement.getStart(sourceFile, false),
        end: existing?.end ?? statement.getStart(sourceFile, false),
        text: existing ? documentation : `${documentation}\n`,
      });
    }
  } else if (ts.isFunctionDeclaration(statement) && isExported(statement)) {
    const documentation = formatDocumentation(completeDocumentation(statement, null), statement);
    const existing = leadingJsdoc(sourceText, statement, sourceFile);
    if (documentation !== existing?.text) {
      replacements.push({
        start: existing?.start ?? statement.getStart(sourceFile, false),
        end: existing?.end ?? statement.getStart(sourceFile, false),
        text: existing ? documentation : `${documentation}\n`,
      });
    }
  } else if (ts.isVariableStatement(statement) && isExported(statement)) {
    const existing = leadingJsdoc(sourceText, statement, sourceFile);
    const declarationNames = statement.declarationList.declarations
      .map((declaration) => declaration.name.getText(sourceFile))
      .join(', ');
    const documentation = formatDocumentation(
      insertSummary(
        existing?.text ?? null,
        `Namespaced TypeScript entry point${statement.declarationList.declarations.length > 1 ? 's' : ''} for ${humanize(declarationNames)} APIs.`
      ),
      statement
    );
    if (documentation !== existing?.text) {
      replacements.push({
        start: existing?.start ?? statement.getStart(sourceFile, false),
        end: existing?.end ?? statement.getStart(sourceFile, false),
        text: existing ? documentation : `${documentation}\n`,
      });
    }
  }
}

let updated = sourceText;
for (const replacement of replacements.sort((left, right) => right.start - left.start)) {
  updated = `${updated.slice(0, replacement.start)}${replacement.text}${updated.slice(replacement.end)}`;
}
if (write && updated !== sourceText) writeFileSync(declarationPath, updated);
if (check && updated !== sourceText) {
  console.error(`${declarationPath}: facade JSDoc is incomplete (${replacements.length} block(s))`);
  process.exit(1);
}
console.log(
  `${write ? 'completed' : check ? 'verified' : 'would complete'} ${replacements.length} facade JSDoc block(s)`
);
