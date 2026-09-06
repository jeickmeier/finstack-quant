import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';
import ts from 'typescript';
import {
  documentationBlocks,
  documentationFromBlocks,
  LEGACY_CATCH_ALL_THROW,
} from './typescript-docs-shared.mjs';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const optionPath = (name, fallback) => {
  const option = process.argv.find((value) => value.startsWith(`--${name}=`));
  return option ? resolve(process.cwd(), option.slice(name.length + 3)) : fallback;
};
const facadePath = optionPath('facade', join(root, 'index.d.ts'));
const rawPath = optionPath('raw', join(root, 'pkg', 'finstack_quant_wasm.d.ts'));
const write = process.argv.includes('--write');
const check = process.argv.includes('--check');
if (write && check) {
  console.error('--write and --check are mutually exclusive');
  process.exit(2);
}
const facadeText = readFileSync(facadePath, 'utf8');
const rawText = readFileSync(rawPath, 'utf8');

function sourceFile(path, text) {
  return ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
}

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

function memberName(node, file) {
  if (ts.isConstructorDeclaration(node) || ts.isConstructSignatureDeclaration(node)) {
    return 'constructor';
  }
  return node.name?.getText(file) ?? null;
}

function rawDocumentation() {
  const file = sourceFile(rawPath, rawText);
  const classes = new Map();
  const functions = new Map();
  for (const statement of file.statements) {
    if (ts.isClassDeclaration(statement) && statement.name) {
      const members = new Map();
      for (const member of statement.members) {
        const name = memberName(member, file);
        if (!name) continue;
        const scope = member.modifiers?.some(
          (modifier) => modifier.kind === ts.SyntaxKind.StaticKeyword
        )
          ? 'static'
          : 'instance';
        members.set(`${scope}:${name}`, leadingJsdoc(rawText, member, file)?.text ?? null);
      }
      classes.set(statement.name.text, {
        documentation: leadingJsdoc(rawText, statement, file)?.text ?? null,
        members,
      });
    } else if (ts.isFunctionDeclaration(statement) && statement.name) {
      functions.set(statement.name.text, leadingJsdoc(rawText, statement, file)?.text ?? null);
    }
  }
  return { classes, functions };
}

function camelCase(name) {
  return name.replace(/_([a-zA-Z])/g, (_, letter) => letter.toUpperCase());
}

function canonicalParameterName(name) {
  return name.replace(/_/g, '').toLowerCase();
}

function normalizeParameterTags(documentationText, node) {
  if (!documentationText || !('parameters' in node)) return documentationText;
  const names = new Map(
    node.parameters.map((parameter) => {
      const name = parameter.name.getText(facade);
      return [canonicalParameterName(name), name];
    })
  );
  return documentationText.replace(/@param\s+([A-Za-z_$][\w$]*)\b/g, (tag, name) => {
    const normalized = names.get(canonicalParameterName(name));
    return normalized ? `@param ${normalized}` : tag;
  });
}

function convertRustArguments(documentationText, node) {
  if (!documentationText) return documentationText;
  const names = new Map(
    ('parameters' in node ? node.parameters : []).map((parameter) => {
      const name = parameter.name.getText(facade);
      return [canonicalParameterName(name), name];
    })
  );
  const body = documentationText
    .replace(/^\/\*\*\s*|\s*\*\/$/g, '')
    .split('\n')
    .map((line) => line.replace(/^\s*\* ?/, '').trimEnd());
  const converted = [];
  let skippingSection = false;
  let inArguments = false;
  for (const line of body) {
    const stripped = line.trim();
    if (stripped === '# Arguments') {
      inArguments = true;
      skippingSection = false;
      continue;
    }
    if (stripped === '# Returns' || stripped === '# Errors') {
      inArguments = false;
      skippingSection = true;
      continue;
    }
    if ((inArguments || skippingSection) && stripped.startsWith('#')) {
      inArguments = false;
      skippingSection = false;
    }
    if (skippingSection) continue;
    if (inArguments) {
      const argument = stripped.match(/^\*\s*`([A-Za-z_][A-Za-z0-9_]*)`\s*-\s*(.+)$/);
      if (argument) {
        const name = names.get(canonicalParameterName(argument[1]));
        if (name) converted.push(`@param ${name} - ${argument[2]}`);
        continue;
      }
      if (stripped && converted.at(-1)?.startsWith('@param')) {
        converted[converted.length - 1] += ` ${stripped}`;
      }
      continue;
    }
    converted.push(line);
  }
  return `/**\n${converted.map((line) => ` *${line ? ` ${line}` : ''}`).join('\n')}\n */`;
}

function documentationIndent(node) {
  return ts.isInterfaceDeclaration(node.parent) || ts.isClassDeclaration(node.parent) ? '  ' : '';
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

function tagsFromRustdoc(documentationText, parameterNames) {
  const lines = documentationText.split('\n');
  const parameterMap = new Map(parameterNames.map((name) => [camelCase(name), name]));
  const tags = [];
  let section = null;
  let sectionText = [];
  const flushSection = () => {
    const description = sectionText.join(' ').replace(/\s+/g, ' ').trim();
    if (description && section === 'returns') tags.push(`@returns ${description}`);
    if (description && section === 'throws') tags.push(`@throws Error - ${description}`);
    section = null;
    sectionText = [];
  };

  for (const line of lines) {
    const stripped =
      line.trim() === '*/'
        ? ''
        : line
            .replace(/^\s*\/\*\*\s?/, '')
            .replace(/^\s*\*\s?/, '')
            .trim();
    if (stripped === '# Returns') {
      flushSection();
      section = 'returns';
      continue;
    }
    if (stripped === '# Errors') {
      flushSection();
      section = 'throws';
      continue;
    }
    if (stripped.startsWith('#')) {
      flushSection();
      continue;
    }
    const parameter = stripped.match(/^@param\s+([A-Za-z_][A-Za-z0-9_]*)\s*-\s*(.+)$/);
    if (parameter) {
      flushSection();
      const name = parameterMap.get(camelCase(parameter[1]));
      if (name) tags.push(`@param ${name} - ${parameter[2]}`);
      continue;
    }
    const argument = stripped.match(/^\*\s*`([A-Za-z_][A-Za-z0-9_]*)`\s*-\s*(.+)$/);
    if (argument) {
      flushSection();
      const name = parameterMap.get(camelCase(argument[1]));
      if (name) tags.push(`@param ${name} - ${argument[2]}`);
      continue;
    }
    const jsdoc = stripped.match(/^@(returns|throws|example)\b\s*(.*)$/);
    if (jsdoc) {
      flushSection();
      tags.push(`@${jsdoc[1]}${jsdoc[2] ? ` ${jsdoc[2]}` : ''}`);
      continue;
    }
    if (section) {
      sectionText.push(stripped.replace(/^[-*]\s*/, ''));
      continue;
    }
    if (stripped && tags.length) {
      const last = tags[tags.length - 1];
      if (last.startsWith('@param') || last.startsWith('@returns') || last.startsWith('@throws')) {
        tags[tags.length - 1] = `${last} ${stripped}`;
      }
    }
  }
  flushSection();
  return tags;
}

function stripRustdocHeadings(documentationText) {
  if (!documentationText) return documentationText;
  if (!/# (?:Arguments|Returns|Errors)\b/.test(documentationText)) return documentationText;
  const body = documentationText
    .replace(/^\/\*\*\s*|\s*\*\/$/g, '')
    .split('\n')
    .map((line) => line.replace(/^\s*\* ?/, '').trimEnd());
  const kept = [];
  let skipping = false;
  for (const line of body) {
    const stripped = line.trim();
    if (stripped === '# Arguments' || stripped === '# Returns' || stripped === '# Errors') {
      skipping = true;
      continue;
    }
    if (skipping && stripped.startsWith('#')) skipping = false;
    if (skipping) continue;
    kept.push(line);
  }
  return `/**\n${kept.map((line) => ` *${line ? ` ${line}` : ''}`).join('\n')}\n */`;
}

function synchronizeTags(rawDocumentationText, facadeDocumentationText, node) {
  if (!rawDocumentationText) {
    if (!facadeDocumentationText) return null;
    const blocks = documentationBlocks(facadeDocumentationText).filter(
      (block) =>
        !(
          block.kind === 'throws' &&
          block.lines.join(' ').replace(/\s+/g, ' ').trim() === LEGACY_CATCH_ALL_THROW
        )
    );
    return documentationFromBlocks(blocks);
  }

  const parameterNames =
    'parameters' in node ? node.parameters.map((parameter) => parameter.name.getText(facade)) : [];
  const rawTags = tagsFromRustdoc(rawDocumentationText, parameterNames);
  const authoritative = new Map(
    ['param', 'returns', 'throws'].map((kind) => [
      kind,
      rawTags.filter((tag) => tag.startsWith(`@${kind}`)),
    ])
  );
  let blocks = documentationBlocks(
    stripRustdocHeadings(facadeDocumentationText ?? rawDocumentationText)
  ).filter(
    (block) =>
      !(
        block.kind === 'throws' &&
        block.lines.join(' ').replace(/\s+/g, ' ').trim() === LEGACY_CATCH_ALL_THROW
      )
  );
  for (const [kind, tags] of authoritative) {
    if (!tags.length) continue;
    blocks = blocks.filter((block) => block.kind !== kind);
    blocks.push(...tags.map((tag) => ({ kind, lines: [tag] })));
  }
  return documentationFromBlocks(blocks);
}

function rawClassName(interfaceName) {
  return interfaceName.endsWith('Constructor')
    ? interfaceName.slice(0, -'Constructor'.length)
    : interfaceName;
}

function candidateDocumentation(node, interfaceName, file, raw) {
  if (ts.isInterfaceDeclaration(node)) {
    return raw.classes.get(rawClassName(interfaceName))?.documentation ?? null;
  }
  if (
    !ts.isMethodSignature(node) &&
    !ts.isConstructSignatureDeclaration(node) &&
    !ts.isPropertySignature(node)
  ) {
    return null;
  }

  const name = memberName(node, file);
  if (!name) return null;
  if (interfaceName.endsWith('Constructor')) {
    const source = raw.classes.get(rawClassName(interfaceName));
    const scope = name === 'constructor' ? 'instance' : 'static';
    return source?.members.get(`${scope}:${name}`) ?? null;
  }
  if (interfaceName.endsWith('Namespace')) {
    return ts.isMethodSignature(node) ? (raw.functions.get(name) ?? null) : null;
  }
  const source = raw.classes.get(interfaceName);
  return source?.members.get(`instance:${name}`) ?? null;
}

const raw = rawDocumentation();
const facade = sourceFile(facadePath, facadeText);
const replacements = [];
let synchronized = 0;

function synchronizeNode(node, rawDocumentationText) {
  const normalized = normalizeParameterTags(rawDocumentationText, node);
  const stripped = convertRustArguments(normalized, node);
  const existing = leadingJsdoc(facadeText, node, facade);
  const candidate = synchronizeTags(normalized, existing?.text ?? stripped, node);
  const formattedCandidate = candidate && formatDocumentation(candidate, node);
  if (!formattedCandidate || formattedCandidate === existing?.text) return;
  replacements.push({
    start: existing?.start ?? node.getStart(facade, false),
    end: existing?.end ?? node.getStart(facade, false),
    text: existing ? formattedCandidate : `${formattedCandidate}\n${documentationIndent(node)}`,
  });
  synchronized += 1;
}

function isPrivateMember(node) {
  return (
    node.modifiers?.some((modifier) => modifier.kind === ts.SyntaxKind.PrivateKeyword) ?? false
  );
}

function classMemberDocumentation(member, source) {
  const name = memberName(member, facade);
  if (!name || !source) return null;
  const scope = member.modifiers?.some((modifier) => modifier.kind === ts.SyntaxKind.StaticKeyword)
    ? 'static'
    : 'instance';
  return source.members.get(`${scope}:${name}`) ?? null;
}

for (const statement of facade.statements) {
  if (ts.isInterfaceDeclaration(statement)) {
    const interfaceName = statement.name.text;
    for (const node of [statement, ...statement.members]) {
      synchronizeNode(node, candidateDocumentation(node, interfaceName, facade, raw));
    }
  } else if (ts.isClassDeclaration(statement) && statement.name) {
    const source = raw.classes.get(statement.name.text);
    synchronizeNode(statement, source?.documentation ?? null);
    for (const member of statement.members) {
      if (isPrivateMember(member)) continue;
      synchronizeNode(member, classMemberDocumentation(member, source));
    }
  } else if (ts.isFunctionDeclaration(statement) && statement.name) {
    synchronizeNode(statement, raw.functions.get(statement.name.text) ?? null);
  }
}

let updated = facadeText;
for (const replacement of replacements.sort((left, right) => right.start - left.start)) {
  updated = `${updated.slice(0, replacement.start)}${replacement.text}${updated.slice(replacement.end)}`;
}

if (write && updated !== facadeText) {
  writeFileSync(facadePath, updated);
}

if (check && updated !== facadeText) {
  console.error(`${facadePath}: facade JSDoc is not synchronized (${synchronized} block(s))`);
  process.exit(1);
}

console.log(
  `${write ? 'synchronized' : check ? 'verified' : 'would synchronize'} ${synchronized} facade JSDoc block(s)`
);
