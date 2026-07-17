import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import ts from 'typescript';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const declarationPath = join(root, 'index.d.ts');
const write = process.argv.includes('--write');
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

function returnDescription(type) {
  if (!type) return 'Returns the result produced by this operation.';
  const text = type.getText(sourceFile);
  if (text === 'number') return 'Returns the computed numeric result in the units described above.';
  if (text === 'boolean') return 'Returns `true` when the documented condition is satisfied.';
  if (text === 'string') return 'Returns the requested string representation or JSON payload.';
  if (text === 'bigint') return 'Returns the requested integer count.';
  if (/^Promise<(.+)>$/.test(text))
    return `Returns a Promise that resolves to \`${text.slice(8, -1)}\`.`;
  if (/^(Float|Uint|Int)\d+Array$/.test(text))
    return `Returns numeric results as a \`${text}\` in the documented order.`;
  if (text.endsWith('[]'))
    return `Returns the resulting \`${text}\` collection in the documented order.`;
  if (/^[A-Za-z_$][\w$]*(?:<.*>)?$/.test(text))
    return `Returns the resulting \`${text}\` value or WebAssembly handle.`;
  return 'Returns the result using the declared TypeScript shape.';
}

const parameterDescriptions = new Map([
  [
    'moduleOrPath',
    'Optional module source: a URL, Response, WebAssembly.Module, or Promise accepted by wasm-bindgen initialization.',
  ],
  [
    'marketJson',
    "JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.",
  ],
  ['asOf', 'ISO-8601 valuation date used to select market inputs and date-dependent cashflows.'],
  [
    'model',
    'Pricing-model key selecting the valuation model implemented by the underlying Rust API.',
  ],
  [
    'envelope',
    'Calibration envelope containing the plan, market data, and optional prior market objects.',
  ],
  ['id', 'Stable identifier used to name or select the requested object.'],
  ['baseDate', 'ISO-8601 base or valuation date that anchors the curve time axis.'],
  ['dayCount', 'Day-count convention used to convert dates into year fractions.'],
  [
    'projectionGrid',
    "Projection-grid specification that defines the curve's forward-rate intervals.",
  ],
  ['resetLag', 'Reset lag applied when projecting the index or forward rate.'],
  ['expiries', 'Option-expiry tenors that define the surface rows in ascending order.'],
  ['tenors', 'Underlying tenors that define the surface columns in ascending order.'],
  [
    'paramsFlat',
    'Row-major flattened parameter array aligned with the documented expiry and tenor axes.',
  ],
  ['forwards', 'Forward values aligned with the corresponding expiry and tenor grid points.'],
  ['interpolationMode', 'Interpolation policy used between the supplied surface pillars.'],
  ['base', 'Base currency of the FX quote or conversion.'],
  ['quote', 'Quote currency of the FX quote or conversion.'],
  ['rate', 'FX or interest rate expressed in the convention stated by this API.'],
  ['date', 'ISO-8601 date at which the requested value or market quote applies.'],
  ['policy', 'FX conversion timing policy applied to the requested cashflow or value.'],
  ['atmVols', 'At-the-money implied volatilities aligned with the supplied expiries.'],
  ['rr25d', '25-delta risk reversals aligned with the supplied expiries.'],
  ['bf25d', '25-delta butterflies aligned with the supplied expiries.'],
  ['rr10d', '10-delta risk reversals aligned with the supplied expiries.'],
  ['bf10d', '10-delta butterflies aligned with the supplied expiries.'],
  ['fromLevels', 'Earlier hierarchy-level snapshot used as the start of the period comparison.'],
  ['toLevels', 'Later hierarchy-level snapshot used as the end of the period comparison.'],
  ['metrics', 'Metric keys or values included in the requested calculation.'],
  ['pricingOptions', 'Pricing options that select calculation behavior and output detail.'],
  ['marketHistory', 'Chronological market snapshots used to project or backtest the result.'],
  ['spec', 'Structured specification that defines the requested object or calculation.'],
  ['json', 'JSON-serialized representation accepted by this API.'],
  ['marketVols', 'Market implied volatilities used as calibration or pricing inputs.'],
  ['varValue', 'Value-at-Risk level or estimate consumed by this calculation.'],
  ['instrumentJson', 'JSON-serialized financial instrument in the canonical Finstack schema.'],
  ['scenarioJson', 'JSON-serialized scenario definition applied to the market or instrument.'],
  ['method', 'Method name or configuration selecting the documented calculation variant.'],
]);

function parameterDescription(parameter) {
  const name = parameter.name.getText(sourceFile);
  const type = parameter.type?.getText(sourceFile) ?? 'value';
  if (parameterDescriptions.has(name)) return parameterDescriptions.get(name);
  if (name.endsWith('Json'))
    return `JSON-serialized ${humanize(name.slice(0, -4))} input accepted by this operation.`;
  if (name.endsWith('Date')) return `ISO-8601 ${humanize(name)} used by this calculation.`;
  if (type === 'number')
    return `${capitalize(humanize(name))} numeric input; use the units and constraints stated above.`;
  if (type === 'string') return `${capitalize(humanize(name))} string consumed by this operation.`;
  if (type === 'boolean') return `Whether to enable ${humanize(name)} behavior.`;
  return `${capitalize(humanize(name))} input consumed by this operation.`;
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
    return `${capitalize(humanize(name))} exposed by this \`${className}\` value.`;
  }
  if (name === 'constructor') return `Create a new \`${className}\` WebAssembly value.`;
  if (name === 'toJson') return `Serialize this \`${className}\` value to canonical JSON.`;
  if (name === 'fromJson') return `Parse a \`${className}\` value from canonical JSON.`;
  if (name === 'toString')
    return `Return the human-readable representation of this \`${className}\` value.`;
  if (name.startsWith('with'))
    return `Return a copy of this \`${className}\` with ${humanize(name.slice(4))} configured.`;
  if (interfaceName?.endsWith('Constructor'))
    return `Create a \`${className}\` value using the ${humanize(name)} convention.`;
  return `Perform ${humanize(name)} for this \`${className}\` value.`;
}

function interfaceSummary(name) {
  if (name === 'WasmOwned')
    return 'Lifecycle contract for a WebAssembly-backed value that owns a wasm heap allocation.';
  if (name.endsWith('Namespace'))
    return `Namespaced TypeScript entry points for ${humanize(classNameFor(name))} calculations and types.`;
  if (name.endsWith('Constructor'))
    return `Construction and factory entry points for \`${classNameFor(name)}\` WebAssembly values.`;
  return `TypeScript view of the \`${name}\` WebAssembly value.`;
}

function typeSummary(name) {
  return `TypeScript type that constrains the accepted ${humanize(name)} values.`;
}

function namespacePaths() {
  const paths = new Map();
  const members = new Map();
  for (const statement of sourceFile.statements) {
    if (ts.isVariableStatement(statement) && isExported(statement)) {
      for (const declaration of statement.declarationList.declarations) {
        const typeName = declaration.type?.getText(sourceFile);
        if (typeName) paths.set(typeName, declaration.name.getText(sourceFile));
      }
    }
    if (ts.isInterfaceDeclaration(statement)) members.set(statement.name.text, statement.members);
  }
  for (let pass = 0; pass < members.size; pass += 1) {
    for (const [name, properties] of members) {
      const parentPath = paths.get(name);
      if (!parentPath) continue;
      for (const property of properties) {
        if (!ts.isPropertySignature(property) || !property.type || !property.name) continue;
        paths.set(
          property.type.getText(sourceFile),
          `${parentPath}.${property.name.getText(sourceFile)}`
        );
      }
    }
  }
  return paths;
}

const paths = namespacePaths();

function exampleForInterface(name) {
  const path = paths.get(name);
  const rootExport = path?.split('.')[0];
  const target = path ?? name;
  const declaration = name.endsWith('Constructor') ? 'factory' : 'api';
  const targetType = name;
  const importLine = rootExport
    ? `import init, { ${rootExport} } from "finstack-quant-wasm";`
    : 'import init from "finstack-quant-wasm";';
  return `@example\n * \`\`\`typescript\n * ${importLine}\n * await init();\n * const ${declaration}: ${targetType} = ${target};\n * void ${declaration};\n * \`\`\``;
}

function exampleForFunction(name) {
  if (name === 'init') {
    return '@example\n * ```typescript\n * import init from "finstack-quant-wasm";\n * const wasm = await init();\n * void wasm;\n * ```';
  }
  return `@example\n * \`\`\`typescript\n * import init, { ${name} } from "finstack-quant-wasm";\n * await init();\n * // Supply the documented arguments to ${name}(...) for your use case.\n * void ${name};\n * \`\`\``;
}

function completeDocumentation(node, interfaceName) {
  let documentationText = leadingJsdoc(sourceText, node, sourceFile)?.text ?? null;
  const existingSummary = summary(documentationText);
  const defaultSummary = ts.isInterfaceDeclaration(node)
    ? interfaceSummary(node.name.text)
    : ts.isTypeAliasDeclaration(node)
      ? typeSummary(node.name.text)
      : nodeSummary(node, interfaceName);
  if (!existingSummary || existingSummary.length < 16) {
    documentationText = insertSummary(documentationText, defaultSummary);
  }

  const tags = [];
  const needsExample =
    ts.isInterfaceDeclaration(node) &&
    (node.name.text.endsWith('Namespace') || node.name.text.endsWith('Constructor'));
  if (needsExample && !hasTag(documentationText, 'example'))
    tags.push(exampleForInterface(node.name.text));
  if (ts.isFunctionDeclaration(node) && !hasTag(documentationText, 'example')) {
    tags.push(exampleForFunction(nodeName(node)));
  }

  if ('parameters' in node) {
    const parameters = documentedParameters(documentationText);
    for (const parameter of node.parameters ?? []) {
      const name = parameter.name.getText(sourceFile);
      if (!parameters.has(name)) tags.push(`@param ${name} - ${parameterDescription(parameter)}`);
    }
    if (node.type?.kind !== ts.SyntaxKind.VoidKeyword && !hasTag(documentationText, 'returns')) {
      tags.push(`@returns ${returnDescription(node.type)}`);
    }
    if (
      (node.parameters?.length ?? 0) > 0 &&
      !hasTag(documentationText, 'throws') &&
      !/does not throw|never throws|non-throwing/i.test(documentationText)
    ) {
      tags.push(
        '@throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.'
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
          text: existing
            ? documentation
            : `${documentation}\n${ts.isInterfaceDeclaration(node.parent) ? '  ' : ''}`,
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
console.log(
  `${write ? 'completed' : 'would complete'} ${replacements.length} facade JSDoc block(s)`
);
