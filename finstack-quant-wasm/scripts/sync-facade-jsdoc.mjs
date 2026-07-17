import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import ts from 'typescript';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const facadePath = join(root, 'index.d.ts');
const rawPath = join(root, 'pkg', 'finstack_quant_wasm.d.ts');
const write = process.argv.includes('--write');
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
      functions.set(statement.name.text, leadingJsdoc(rawText, statement)?.text ?? null);
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
  if (!documentationText || !('parameters' in node)) return documentationText;
  const names = new Map(
    node.parameters.map((parameter) => {
      const name = parameter.name.getText(facade);
      return [canonicalParameterName(name), name];
    })
  );
  const body = documentationText
    .replace(/^\/\*\*\s*|\s*\*\/$/g, '')
    .split('\n')
    .map((line) => line.replace(/^\s*\* ?/, '').trimEnd());
  const converted = [];
  let inArguments = false;
  for (const line of body) {
    const stripped = line.trim();
    if (stripped === '# Arguments') {
      inArguments = true;
      continue;
    }
    if (inArguments && stripped.startsWith('#')) inArguments = false;
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

function formatDocumentation(documentationText, node) {
  const indent = ts.isInterfaceDeclaration(node.parent) ? '  ' : '';
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
    if (description && section === 'throws')
      tags.push(`@throws Error - ${description.replace(/^throws\s*/i, '')}`);
    section = null;
    sectionText = [];
  };

  for (const line of lines) {
    const stripped = line.trim();
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
      const name = parameterMap.get(camelCase(parameter[1]));
      if (name) tags.push(`@param ${name} - ${parameter[2]}`);
      continue;
    }
    const argument = stripped.match(/^\*\s*`([A-Za-z_][A-Za-z0-9_]*)`\s*-\s*(.+)$/);
    if (argument) {
      const name = parameterMap.get(camelCase(argument[1]));
      if (name) tags.push(`@param ${name} - ${argument[2]}`);
      continue;
    }
    const jsdoc = stripped.match(/^@(returns|throws|example)\b\s*(.*)$/);
    if (jsdoc) {
      tags.push(`@${jsdoc[1]}${jsdoc[2] ? ` ${jsdoc[2]}` : ''}`);
      continue;
    }
    if (section) sectionText.push(stripped.replace(/^[-*]\s*/, ''));
  }
  flushSection();
  return tags;
}

function mergeRustTags(documentationText, node) {
  if (!documentationText || !('parameters' in node)) return documentationText;
  const parameterNames = node.parameters.map((parameter) => parameter.name.getText(facade));
  const candidate = tagsFromRustdoc(documentationText, parameterNames);
  if (!candidate?.length) return documentationText;

  const existing = new Set(
    [...documentationText.matchAll(/@(param|returns|throws)\b[^\n]*/g)].map((match) =>
      match[0].replace(/\s+/g, ' ').trim()
    )
  );
  const existingParameters = new Set(
    [...documentationText.matchAll(/@param\s+([A-Za-z_$][\w$]*)\b/g)].map((match) => match[1])
  );
  const hasReturns = [...existing].some((value) => value.startsWith('@returns'));
  const hasThrows = [...existing].some((value) => value.startsWith('@throws'));
  const additions = candidate.filter((tag) => {
    const normalized = tag.replace(/\s+/g, ' ').trim();
    const parameter = normalized.match(/^@param\s+([A-Za-z_$][\w$]*)\b/);
    if (parameter) return !existingParameters.has(parameter[1]);
    if (normalized.startsWith('@returns')) return !hasReturns;
    if (normalized.startsWith('@throws')) return !hasThrows;
    return !existing.has(normalized);
  });
  if (!additions.length) return documentationText;
  return documentationText.replace(/\*\/$/, `${additions.map((tag) => ` * ${tag}\n`).join('')} */`);
}

function score(documentation) {
  if (!documentation) return 0;
  return (
    documentation.length +
    200 * (documentation.match(/@param/g)?.length ?? 0) +
    100 * (documentation.match(/@returns/g)?.length ?? 0) +
    100 * (documentation.match(/@throws/g)?.length ?? 0)
  );
}

function mergeDocumentation(primary, secondary) {
  if (!primary) return secondary;
  if (!secondary) return primary;
  const preferred = score(primary) >= score(secondary) ? primary : secondary;
  const supplemental = preferred === primary ? secondary : primary;
  const existingParameters = new Set(
    [...preferred.matchAll(/@param\s+([A-Za-z_$][\w$]*)\b/g)].map((match) => match[1])
  );
  const tags = [];
  for (const tag of supplemental.matchAll(/@(param|returns|throws)\b[^\n]*/g)) {
    const value = tag[0].replace(/\s+/g, ' ').trim();
    const parameter = value.match(/^@param\s+([A-Za-z_$][\w$]*)\b/);
    if (parameter && !existingParameters.has(parameter[1])) {
      tags.push(value);
      existingParameters.add(parameter[1]);
    } else if (!parameter && !preferred.includes(value)) {
      tags.push(value);
    }
  }
  if (!tags.length) return preferred;
  return preferred.replace(/\*\/$/, `${tags.map((tag) => ` * ${tag}\n`).join('')} */`);
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
    return raw.functions.get(name) ?? null;
  }
  const source = raw.classes.get(interfaceName);
  return source?.members.get(`instance:${name}`) ?? null;
}

const raw = rawDocumentation();
const facade = sourceFile(facadePath, facadeText);
const replacements = [];
let synchronized = 0;

for (const statement of facade.statements) {
  if (!ts.isInterfaceDeclaration(statement)) continue;
  const interfaceName = statement.name.text;
  for (const node of [statement, ...statement.members]) {
    const rawCandidate = convertRustArguments(
      normalizeParameterTags(candidateDocumentation(node, interfaceName, facade, raw), node),
      node
    );
    const existing = leadingJsdoc(facadeText, node, facade);
    const candidate = mergeRustTags(mergeDocumentation(rawCandidate, existing?.text ?? null), node);
    const formattedCandidate = candidate && formatDocumentation(candidate, node);
    if (!formattedCandidate || formattedCandidate === existing?.text) continue;
    replacements.push({
      start: existing?.start ?? node.getStart(facade, false),
      end: existing?.end ?? node.getStart(facade, false),
      text: existing
        ? formattedCandidate
        : `${formattedCandidate}\n${ts.isInterfaceDeclaration(node.parent) ? '  ' : ''}`,
    });
    synchronized += 1;
  }
}

let updated = facadeText;
for (const replacement of replacements.sort((left, right) => right.start - left.start)) {
  updated = `${updated.slice(0, replacement.start)}${replacement.text}${updated.slice(replacement.end)}`;
}

if (write && updated !== facadeText) {
  writeFileSync(facadePath, updated);
}

console.log(
  `${write ? 'synchronized' : 'would synchronize'} ${synchronized} facade JSDoc block(s)`
);
