import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import prettier from "prettier";
const root = fileURLToPath(new URL("../", import.meta.url));
export function themeOutput(tokens) {
  const declarations = (values) =>
    Object.entries(values)
      .map(([key, value]) => `--${key}: ${value};`)
      .join("\n");
  const imports = tokens.fonts
    .flatMap((font) =>
      font.faces.map((face) => `@import "${font.package}/${face}";`),
    )
    .join("\n");
  const css = `@import "shadcn/tailwind.css";
@import "tw-animate-css";
${imports}
@custom-variant dark (&:where(.dark, .dark *, [data-theme="dark"], [data-theme="dark"] *));
:root { ${declarations(tokens.theme)} ${declarations(tokens.density.compact)} }
:root, [data-theme="light"] { ${declarations(tokens.light)} }
:root.dark, [data-theme="dark"] { ${declarations(tokens.dark)} }
[data-density="compact"] { ${declarations(tokens.density.compact)} }
[data-density="comfortable"] { ${declarations(tokens.density.comfortable)} }
@theme inline { --radius-sm:calc(var(--radius) - 4px); --radius-md:calc(var(--radius) - 2px); --radius-lg:var(--radius); --radius-xl:calc(var(--radius) + 4px); ${Object.keys(
    tokens.light,
  )
    .map((key) => `--color-${key}: var(--${key});`)
    .join("\n")} }
@theme { ${Object.keys(tokens.theme)
    .filter((key) =>
      /^(font-(?:sans|mono)|text-|spacing$|radius-|breakpoint-)/.test(key),
    )
    .map((key) => `--${key}: ${tokens.theme[key]};`)
    .join("\n")} }
@utility finstack-numeric { font-variant-numeric: var(--font-numeric-variant); }
@utility finstack-row { min-height: var(--row-height); }
@utility finstack-surface { background: var(--background); color: var(--foreground); font-family: var(--font-sans); font-size: var(--text-base); }
.finstack-term-sheet { container-type:inline-size; }
.finstack-editable-term { display:grid; grid-template-columns:minmax(0,1fr) auto; align-items:center; gap:var(--spacing); }
.finstack-fieldset { padding:0; margin:0 0 calc(var(--spacing) * 3); border:0; }
.finstack-fieldset > legend { float:left; margin-bottom:var(--spacing); }
.finstack-fieldset > legend + * { clear:both; }
.finstack-fieldset > details { padding:var(--spacing) 0; font-size:var(--text-xs); }
.finstack-term-fields { min-width:0; }
.finstack-term-sheet .finstack-field-frame { display:grid; grid-template-columns:minmax(100px,var(--term-width)) minmax(0,1fr); align-items:center; gap:0 calc(var(--spacing) * 2); padding-block:var(--spacing); }
.finstack-term-sheet .finstack-field-frame > * { margin:0; }
.finstack-term-sheet .finstack-field-frame > [role=alert] { grid-column:2; padding:var(--spacing) 0; }
.finstack-term-sheet .finstack-field-label { align-self:center; }
.finstack-term-sheet .finstack-fieldset .finstack-fieldset { padding-top:calc(var(--spacing) * 2); }
.finstack-term-sheet .finstack-optional-term > span { color:var(--muted-foreground); }
.finstack-schema-root > legend { display:none; }
.finstack-schema-root > details { margin-bottom:calc(var(--spacing) * 2); }
.finstack-instrument-header { display:flex; flex-wrap:wrap; align-items:center; gap:calc(var(--spacing) * 3); border-bottom:var(--border-width) solid var(--border); padding-bottom:calc(var(--spacing) * 2); }
.finstack-instrument-header .finstack-instrument-selector { flex:1; min-width:220px; }
.finstack-pricing-params { display:flex; flex-direction:column; gap:calc(var(--spacing) * 3); }
.finstack-pricing-context { display:flex; flex-wrap:wrap; align-items:center; gap:calc(var(--spacing) * 3); }
.finstack-pricing-context .finstack-field-frame { display:flex; align-items:center; gap:calc(var(--spacing) * 2); }
.finstack-pricing-context .finstack-field-frame > * { margin:0; }
.finstack-pricing-context__date .finstack-field-control { width:180px; }
.finstack-pricing-context .finstack-field-label { white-space:nowrap; }
.finstack-pricing-metrics { display:grid; gap:calc(var(--spacing) * 2); }
.finstack-pricing-advanced { display:grid; gap:calc(var(--spacing) * 4); }
@container (min-width:960px) {
  .finstack-schema-root > .finstack-term-fields { columns:2; column-gap:calc(var(--spacing) * 8); }
  .finstack-schema-root > .finstack-term-fields > * { break-inside:avoid; }
}
@container (max-width:420px) {
  .finstack-term-sheet .finstack-field-frame { grid-template-columns:112px minmax(0,1fr); gap:var(--spacing); }
}
.finstack-workbench-container { container-type: inline-size; min-width: 0; }
.finstack-workbench { display:flex; flex-direction:column; min-width:0; background:var(--background); }
.finstack-workbench__bar { display:flex; flex-wrap:wrap; align-items:center; gap:8px; padding:8px 12px; border-bottom:1px solid var(--border); }
.finstack-workbench__brand { font-family:var(--font-mono); font-size:14px; font-weight:500; color:var(--primary); }
.finstack-workbench__tabs { max-width:100%; overflow-x:auto; }
.finstack-workbench__context { margin-left:auto; }
.finstack-workbench__inputs,.finstack-workbench__panel { min-width:0; min-height:0; }
.finstack-workbench__panel { padding:12px 16px; }
.finstack-workbench__results { display:grid; grid-template-columns:minmax(0,1fr); min-height:0; border-top:1px solid var(--border); }
.finstack-workbench__summary { padding:12px 16px; min-width:0; min-height:0; }
.finstack-workbench__detail { min-width:0; min-height:0; border-top:1px solid var(--border); }
.finstack-workbench__detail > [role=tabpanel] { padding:12px 16px; }
.finstack-workbench__status { display:flex; flex-wrap:wrap; gap:12px; padding:5px 12px; border-top:1px solid var(--border); font-size:11px; color:var(--muted-foreground); }
.finstack-workbench__state { display:inline-flex; gap:6px; align-items:center; font-size:11px; white-space:nowrap; }
.finstack-workbench__state::before { content:''; width:5px; height:5px; border-radius:50%; background:var(--primary); }
.finstack-market { container-type:inline-size; min-width:0; }
.finstack-market__layout { display:grid; grid-template-columns:minmax(0,1fr); gap:16px; }
.finstack-market__rail { min-width:0; font-size:12.5px; }
.finstack-market__category { padding:6px 0; border-bottom:1px solid var(--border); }
.finstack-market__selected { min-width:0; }
.finstack-market__selected > header { display:flex; flex-wrap:wrap; justify-content:space-between; align-items:center; gap:8px; margin-bottom:12px; }
.finstack-market__selected > header h2 { font-family:var(--font-mono); font-size:12.5px; }
 .finstack-market__category-picker { display:none; }
@container (max-width:719px) { .finstack-market__category-picker { display:block; } .finstack-market__rail nav>ul { display:block; max-height:180px; overflow:auto; } .finstack-market__category { min-width:0; max-width:none; } .finstack-market__category:not([data-active-category=true]) { display:none; } .finstack-market__rail nav[data-searching=true] .finstack-market__category { display:block; } }
@container (min-width:720px) { .finstack-market__layout { grid-template-columns:200px minmax(0,1fr); } .finstack-market__rail { max-height:440px; overflow:auto; border-right:1px solid var(--border); padding-right:12px; } }
@container (min-width:960px) {
[data-registry-focus] .finstack-workbench { height:calc(100dvh - 2px); min-height:620px; }
.finstack-workbench { height:min(820px,calc(100dvh - 56px)); min-height:620px; display:grid; grid-template-rows:auto minmax(0,52fr) minmax(0,48fr) auto; }
.finstack-workbench__bar { gap:6px; }
.finstack-workbench__inputs { overflow:hidden; }
.finstack-workbench__panel { height:100%; overflow:auto; }
.finstack-workbench__results { grid-template-columns:minmax(0,38fr) minmax(0,62fr); overflow:hidden; }
.finstack-workbench__summary { overflow:auto; }
.finstack-workbench__detail { display:flex; flex-direction:column; border-top:0; border-left:1px solid var(--border); overflow:hidden; }
.finstack-workbench__detail > [role=tabpanel] { flex:1; min-height:0; overflow:auto; }
}
/* Settle native SVG geometry at a physical width before the print dialog opens. */
[data-printing] .finstack-market__layout, [data-printing] .finstack-stored-curve__layout { display:block; }
[data-printing] .finstack-market__rail { display:none; }
[data-printing] .finstack-market__selected { width:180mm; max-width:none; }
[data-printing] [data-finstack-figure] { width:180mm; max-width:100%; }
@media print { .finstack-workbench { display:block; height:auto; min-height:0; } .finstack-workbench__inputs,.finstack-workbench__panel,.finstack-workbench__results,.finstack-workbench__summary,.finstack-workbench__detail { display:block; height:auto; overflow:visible; } .finstack-workbench__bar,.finstack-workbench__status { display:none; } }

@media print {
  @page { size: auto; margin: 12mm; }
  .finstack-surface:has([data-finstack-print]) { ${declarations(tokens.light)} width: auto !important; max-width: none !important; padding: 0 !important; }
  [data-finstack-print] { ${declarations(tokens.light)} font-size: 9pt; color: var(--foreground); background: var(--background); }
  [data-finstack-print] div, [data-finstack-print] section { max-height: none !important; overflow: visible !important; }
  [data-finstack-print] button, [data-finstack-print] [role="tablist"] { display: none !important; }
  [data-finstack-print] pre, [data-finstack-print] [data-json-print-source] { max-height: none; overflow: visible; white-space: pre-wrap; overflow-wrap: anywhere; font-size: 8pt; line-height: 1.4; }
  [data-finstack-print] table { width: 100%; font-size: 9pt; border-collapse: collapse; }
  [data-finstack-print] thead { position: static !important; display: table-header-group; }
  [data-finstack-print] tfoot { position: static !important; display: table-footer-group; }
  [data-finstack-print] tr { break-inside: avoid; }
  [data-finstack-print] th, [data-finstack-print] td { white-space: normal; overflow-wrap: anywhere; }
  [data-finstack-print] .finstack-market__layout, [data-finstack-print] .finstack-stored-curve__layout { display:block; }
  [data-finstack-print] .finstack-workbench__panel { padding-inline:0; }
  [data-finstack-print] .finstack-market__selected { width:180mm; max-width:100%; }
  [data-finstack-print] [data-finstack-figure] { width:180mm; max-width:100%; break-inside: avoid; }
  [data-finstack-print] .ts-chart-host, [data-finstack-print] .ts-chart-surface { height: auto !important; }
  [data-finstack-print] svg { max-width: 100%; height: auto; break-inside: avoid; }
  [data-finstack-print] caption, [data-finstack-print] h2, [data-finstack-print] h3, [data-finstack-print] summary { break-after: avoid; }
}
`;
  const item = {
    name: "finstack-theme",
    type: "registry:theme",
    docs: "Shared light/dark theme. Import styles/finstack/theme.css after Tailwind. Set data-theme and data-density on the app root. Tenant styles override the same properties after this stylesheet. IBM Plex Sans and Mono are supplied by pinned Fontsource packages under SIL OFL-1.1; no font binaries are copied into registry files.",
    dependencies: [
      "shadcn@4.21.0",
      "tw-animate-css@1.4.0",
      ...tokens.fonts.map((font) => `${font.package}@${font.version}`),
    ],
    files: [
      {
        path: "finstack-theme/theme.css",
        type: "registry:file",
        target: "styles/finstack/theme.css",
      },
    ],
    cssVars: { theme: tokens.theme, light: tokens.light, dark: tokens.dark },
  };
  return { css, item };
}
export async function generateTheme(check = false) {
  const tokens = JSON.parse(
    await readFile(
      path.join(root, "registry/theme/finstack-theme/tokens.json"),
      "utf8",
    ),
  );
  const { css, item } = themeOutput(tokens);
  const outputs = [
    [
      "registry/theme/finstack-theme/theme.css",
      await prettier.format(css, { parser: "css" }),
    ],
    [
      "registry/theme/registry.json",
      await prettier.format(
        JSON.stringify({
          $schema: "https://ui.shadcn.com/schema/registry.json",
          items: [item],
        }),
        { parser: "json" },
      ),
    ],
  ];
  for (const [file, content] of outputs) {
    if (check) {
      if ((await readFile(path.join(root, file), "utf8")) !== content)
        throw new Error(`Theme drift: ${file}`);
    } else await writeFile(path.join(root, file), content);
  }
  return outputs;
}
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  await generateTheme(process.argv.includes("--check"));
  console.log("Checked theme generation.");
}
