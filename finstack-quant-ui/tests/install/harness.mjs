import { readFile } from "node:fs/promises";
import path from "node:path";
import ts from "typescript";

const printer = ts.createPrinter();
const names = (node) => {
  const found = new Set();
  function visit(child) {
    if (ts.isIdentifier(child)) found.add(child.text);
    ts.forEachChild(child, visit);
  }
  visit(node);
  return found;
};
const bindings = (node) =>
  ts.isIdentifier(node)
    ? [node.text]
    : node.elements.flatMap((part) =>
        ts.isBindingElement(part) ? bindings(part.name) : [],
      );

// Project the selected gallery case and only declarations/imports it references.
// Consumer type/build checks then prove those imports are in the item's own closure.
function project(source, functionName, item) {
  const file = ts.createSourceFile(
    "harness.tsx",
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TSX,
  );
  const fn = file.statements.find(
    (node) =>
      ts.isFunctionDeclaration(node) && node.name?.text === functionName,
  );
  const branch = fn.body.statements.find(ts.isSwitchStatement);
  let selected = false,
    body;
  for (const clause of branch.caseBlock.clauses) {
    if (ts.isCaseClause(clause) && clause.expression.text === item)
      selected = true;
    if (selected && clause.statements.length) {
      body = clause.statements;
      break;
    }
  }
  if (!body) throw Error(`No gallery harness for ${item}`);
  const declarations = [];
  function collect(nodes, local) {
    for (const node of nodes) {
      if (ts.isVariableStatement(node))
        for (const declaration of node.declarationList.declarations)
          declarations.push({
            node: declaration,
            bound: bindings(declaration.name),
            local,
            text: `const ${declaration.getText(file)};`,
          });
      if (!local && ts.isFunctionDeclaration(node) && node !== fn)
        declarations.push({
          node,
          bound: [node.name.text],
          local,
          text: node.getText(file),
        });
    }
  }
  collect(file.statements, false);
  collect(fn.body.statements, true);
  const used = new Set(body.flatMap((node) => [...names(node)])),
    keep = new Set();
  for (let changed = true; changed;) {
    changed = false;
    for (const declaration of declarations)
      if (
        !keep.has(declaration) &&
        declaration.bound.some((name) => used.has(name))
      ) {
        keep.add(declaration);
        changed = true;
        for (const name of names(declaration.node)) used.add(name);
      }
  }
  const imports = [];
  for (const node of file.statements.filter(ts.isImportDeclaration)) {
    const clause = node.importClause;
    if (!clause) continue;
    const defaultName =
      clause.name && used.has(clause.name.text) ? clause.name : undefined;
    const bound = clause.namedBindings;
    const named =
      bound &&
      (ts.isNamedImports(bound)
        ? ts.factory.updateNamedImports(
            bound,
            bound.elements.filter((part) => used.has(part.name.text)),
          )
        : used.has(bound.name.text)
          ? bound
          : undefined);
    if (
      !defaultName &&
      (!named || (ts.isNamedImports(named) && !named.elements.length))
    )
      continue;
    imports.push(
      printer.printNode(
        ts.EmitHint.Unspecified,
        ts.factory.updateImportDeclaration(
          node,
          node.modifiers,
          ts.factory.updateImportClause(
            clause,
            clause.isTypeOnly,
            defaultName,
            named,
          ),
          node.moduleSpecifier,
          node.attributes,
        ),
        file,
      ),
    );
  }
  return `"use client";\n${imports.join("\n")}\nconst name=${JSON.stringify(item)}, density="compact" as const, width=880, publication=false, variant="default" as string;\n${[
    ...keep,
  ]
    .filter((d) => !d.local)
    .map((d) => d.text)
    .join("\n")}\nexport function InstalledItem(){\n${[...keep]
    .filter((d) => d.local)
    .map((d) => d.text)
    .join("\n")}\n${body.map((node) => node.getText(file)).join("\n")}\n}\n`;
}

const nativeForms = new Map([
  ["schema-form", ["SchemaForm", "schema-form/schema-form"]],
  ["instrument-form", ["InstrumentForm", "instrument-form/instrument-form"]],
  [
    "market-context-form",
    ["MarketContextForm", "market-context-form/market-context-form"],
  ],
  [
    "calibration-form",
    ["CalibrationForm", "calibration-form/calibration-form"],
  ],
]);
export async function visualHarness(repo, item) {
  if (nativeForms.has(item.name)) {
    const [component, target] = nativeForms.get(item.name);
    const properties =
      item.name === "schema-form"
        ? "module={bond} defaultValues={bond.codec.parse(data.bond.request.instrumentJson) as Record<string,unknown>}"
        : item.name === "instrument-form"
          ? "type={type} onTypeChange={setType} defaultJson={data.bond.request.instrumentJson}"
          : `defaultJson={JSON.stringify(data.${item.name === "market-context-form" ? "market.supplemental" : "calibration[0]!.input"})}`;
    const source = `"use client";
import {useState} from "react";
import {${component}} from "@/components/finstack/components/${target}";
${item.name === "schema-form" ? 'import schema from "./bond.schema.json"; import {createWireCodec} from "@/lib/finstack/codec.mjs"; const codec=createWireCodec(schema); const bond={schema,codec,metadata:[],example:{}};' : ""}
import data from "./data.json";
const validate=async(json:string)=>json;
export function InstalledItem(){const [type,setType]=useState("bond"),[count,setCount]=useState(0);return <><p>Supplied valid fixture; standalone structural form with a caller validation callback.</p><${component} ${properties} validate={validate} onSubmit={()=>setCount(n=>n+1)}/><output aria-label="Submission count">{count}</output></>}`;
    return item.name === "schema-form"
      ? {
          source,
          extra: {
            "bond.schema.json": await readFile(
              path.join(
                repo,
                "finstack-quant-ui/src/generated/schemas/bond.json",
              ),
              "utf8",
            ),
          },
        }
      : source;
  }
  const primitive = ["registry:ui", "registry:theme"].includes(item.type);
  const source = await readFile(
    path.join(
      repo,
      "docs-site/src/components/registry-gallery",
      primitive ? "primitives.tsx" : "components.tsx",
    ),
    "utf8",
  );
  let result = project(
    source,
    primitive ? "PrimitiveDemo" : "ComponentDemo",
    item.name,
  );
  if (item.name === "finstack-chart") {
    // The supplied figure definition is fixture data, not another installed component.
    const definition = await readFile(
      path.join(
        repo,
        "finstack-quant-ui/registry/components/figure-example/figure-example.tsx",
      ),
      "utf8",
    );
    const file = ts.createSourceFile(
      "figure.tsx",
      definition,
      ts.ScriptTarget.Latest,
      true,
      ts.ScriptKind.TSX,
    );
    const fixture = file.statements
      .filter(
        (node) =>
          (ts.isImportDeclaration(node) &&
            node.moduleSpecifier.text.startsWith("@tanstack/")) ||
          (ts.isVariableStatement(node) &&
            node.declarationList.declarations.some(
              (declaration) =>
                declaration.name.getText(file) === "figureExample",
            )),
      )
      .map((node) => node.getText(file))
      .join("\n");
    return {
      source: await readFile(
        path.join(repo, "finstack-quant-ui/tests/install/fixture/chart.tsx"),
        "utf8",
      ),
      extra: { "figure-data.ts": fixture },
    };
  }
  const wrappers = new Map([
    ["curve-chart", "CurveChart"],
    ["calibration-fit-chart", "CalibrationFitChart"],
    ["scenario-heatmap", "ScenarioHeatmap"],
    ["vol-surface-chart", "VolSurfaceChart"],
    ["fx-surface-chart", "FxSurfaceChart"],
    ["vol-cube-explorer", "VolCubeExplorer"],
  ]);
  if (wrappers.has(item.name)) {
    const owner = wrappers.get(item.name);
    result = result.replace(
      '"use client";',
      '\"use client\";\nimport {captureFigure,captureFigures,recordActivation,TooltipAction} from "./wrapper-probe";',
    );
    const props = `${item.name === "scenario-heatmap" ? "figureRef={captureFigure}" : "figureRefs={captureFigures}"} title="Independent wrapper figure" subtitle="Supplied presentation through public wrapper props" onSelect={recordActivation} renderTooltipBody={({defaultBody,pinned,dismiss})=><>{defaultBody}{pinned&&<TooltipAction dismiss={dismiss}/>}</>}`;
    result = result.replace(`<${owner}`, `<${owner} ${props}`);
    return {
      source: result,
      extra: {
        "wrapper-probe.tsx": await readFile(
          path.join(
            repo,
            "finstack-quant-ui/tests/install/fixture/wrapper-probe.tsx",
          ),
          "utf8",
        ),
      },
    };
  }
  return result;
}
