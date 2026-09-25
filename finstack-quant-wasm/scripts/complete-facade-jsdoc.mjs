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

function numericReturnDescription(name, existingSummary) {
  const method = (name || '').toLowerCase();
  if (method === 'df') return 'Discount factor for 1 unit paid at the requested time.';
  if (method === 'zero') return 'Continuously compounded zero rate as a decimal.';
  if (method === 'forward' || method === 'rate' || method === 'ratebetween')
    return 'Forward or zero rate as a decimal.';
  if (method === 'volga')
    return 'Volga: change in vega per 1.0 absolute move in implied volatility.';
  if (method.includes('vol') || method === 'impliedvol' || method === 'atmvol')
    return 'Implied volatility as a decimal, such as 0.20 for 20%.';
  if (method.includes('yearfraction') || method === 'toyearssimple')
    return 'Year fraction in years under the selected day-count convention.';
  if (method.includes('prob') || method.includes('cdf')) return 'Probability in `[0, 1]`.';
  if (method.includes('price') || method.includes('pv'))
    return 'Price in the same units as the supplied spot, forward, or notional.';
  if (method === 'delta') return 'Spot delta: change in value per unit spot.';
  if (method === 'gamma') return 'Spot gamma: change in delta per unit spot.';
  if (method === 'vega')
    return 'Vega: change in value per 1.0 absolute move in implied volatility.';
  if (method === 'theta') return 'Theta: change in value per year of calendar time.';
  if (method === 'rho' || method === 'foreignrho')
    return 'Interest-rate rho: change in value per 1.0 absolute move in the corresponding rate.';
  if (method === 'vanna') return 'Vanna: cross sensitivity of delta to implied volatility.';
  const summary = firstSummarySentence(existingSummary);
  if (summary) return `${summary}.`;
  return `Returns the ${humanize(name)} as a finite number.`;
}

function returnDescription(type, name, existingSummary) {
  if (!type) return 'Returns the result of this call.';
  const text = type.getText(sourceFile);
  const method = name || '';
  if (text === 'number') return numericReturnDescription(method, existingSummary);
  if (text === 'boolean') {
    const summary = firstSummarySentence(existingSummary);
    if (summary) return `${summary}.`;
    return 'Returns `true` or `false`.';
  }
  if (text === 'string') {
    if (method === 'toString') return 'Human-readable string form of this value.';
    if (method === 'toJson' || method.endsWith('Json')) return 'Canonical JSON string.';
    return 'JSON string or identifier produced by this call.';
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

const parameterDescriptions = new Map([
  [
    'moduleOrPath',
    'Optional module source: a URL, Response, WebAssembly.Module, or Promise accepted by wasm-bindgen initialization.',
  ],
  ['marketJson', 'Canonical market-context JSON supplying curves, quotes, and FX data.'],
  ['asOf', 'ISO-8601 valuation date used to select market inputs and date-dependent cashflows.'],
  ['model', "Optional pricing-model identifier; omit to use the instrument's default model."],
  [
    'envelope',
    'Calibration envelope containing the plan, market data, and optional prior market objects.',
  ],
  ['id', 'Identifier assigned to this object.'],
  ['baseDate', 'ISO-8601 base or valuation date that anchors the curve time axis.'],
  ['dayCount', 'Day-count convention used to convert dates into year fractions.'],
  [
    'projectionGrid',
    "Projection-grid specification that defines the curve's forward-rate intervals.",
  ],
  ['resetLag', 'Reset lag applied when projecting the index or forward rate.'],
  ['expiries', 'Option-expiry tenors that define the surface rows in ascending order.'],
  ['tenors', 'Underlying tenors that define the surface columns in ascending order.'],
  ['paramsFlat', 'Row-major flattened parameter array aligned with the expiry and tenor axes.'],
  ['forwards', 'Forward values aligned with the corresponding expiry and tenor grid points.'],
  ['interpolationMode', 'Interpolation policy used between the supplied surface pillars.'],
  ['base', 'Base currency of the FX quote or conversion.'],
  ['quote', 'Quote currency of the FX quote or conversion.'],
  ['rate', 'FX or interest rate as a decimal in the quote convention of the surrounding call.'],
  ['date', 'ISO-8601 date at which the requested value or market quote applies.'],
  ['policy', 'FX conversion timing policy applied to the requested cashflow or value.'],
  ['atmVols', 'At-the-money implied volatilities aligned with the supplied expiries.'],
  ['rr25d', '25-delta risk reversals aligned with the supplied expiries.'],
  ['bf25d', '25-delta butterflies aligned with the supplied expiries.'],
  ['rr10d', '10-delta risk reversals aligned with the supplied expiries.'],
  ['bf10d', '10-delta butterflies aligned with the supplied expiries.'],
  ['fromLevels', 'Earlier hierarchy-level snapshot used as the start of the period comparison.'],
  ['toLevels', 'Later hierarchy-level snapshot used as the end of the period comparison.'],
  [
    'metrics',
    "Optional metric identifiers to compute with this call; omit, null, or empty follows that callable's documented default.",
  ],
  [
    'metricPricingOverrides',
    'Optional JSON metric-pricing overrides merged into the instrument envelope before validation.',
  ],
  [
    'marketHistory',
    'Optional serialized market-history JSON required by historical risk metrics such as historical VaR.',
  ],
  ['spec', 'Specification object or JSON used to construct this value.'],
  ['json', 'Canonical JSON string for this value.'],
  ['jsonStr', 'Canonical JSON string to validate and re-serialize.'],
  ['params', 'Optional parameters; omit to use defaults.'],
  ['t', 'Time from the curve base date in years.'],
  ['spot', 'Current spot price or exchange rate in the same units as the strike.'],
  ['isCall', 'Whether to value a call (`true`) or put (`false`).'],
  ['computeIncremental', 'When true, also compute incremental VaR; omit to skip.'],
  ['midYearConvention', 'When true, apply mid-year DCF discounting.'],
  ['marketVols', 'Market implied volatilities used as calibration or pricing inputs.'],
  ['varValue', 'Loss-convention VaR in the same units as `positionValue`; must be non-positive.'],
  ['instrumentJson', 'Canonical instrument envelope JSON in the Finstack v1 schema.'],
  ['modelJson', 'Financial-model specification JSON.'],
  ['configJson', 'Configuration JSON for this call.'],
  ['resultsJson', 'Evaluated statement-result JSON.'],
  ['returnsJson', 'Numeric return-series JSON.'],
  ['scenarioJson', 'JSON-serialized scenario definition applied to the market or instrument.'],
  ['method', 'Method name selecting the calculation variant.'],
]);

function parameterDescription(parameter) {
  const name = parameter.name.getText(sourceFile);
  const type = parameter.type?.getText(sourceFile) ?? 'value';
  if (parameterDescriptions.has(name)) return parameterDescriptions.get(name);
  if (name.endsWith('Json')) return `Canonical JSON for ${humanize(name.slice(0, -4))}.`;
  if (name.endsWith('Date')) return `ISO-8601 ${humanize(name)} used by this calculation.`;
  if (name === 'matrix')
    return 'Square numeric matrix as nested `number[][]` or flat row-major entries.';
  if (type === 'number') return `${capitalize(humanize(name))} as a finite number.`;
  if (type === 'string') return `${capitalize(humanize(name))} as a string.`;
  if (type === 'boolean') return `Whether ${humanize(name)} is true.`;
  return `${capitalize(humanize(name))} used by this call.`;
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
      if (!parameters.has(name)) tags.push(`@param ${name} - ${parameterDescription(parameter)}`);
    }
    const skipReturns =
      ts.isConstructorDeclaration(node) || node.type?.kind === ts.SyntaxKind.VoidKeyword;
    if (!skipReturns && !hasTag(documentationText, 'returns')) {
      tags.push(
        `@returns ${returnDescription(
          node.type,
          nodeName(node),
          existingSummary && existingSummary.length >= 16 ? existingSummary : defaultSummary
        )}`
      );
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
