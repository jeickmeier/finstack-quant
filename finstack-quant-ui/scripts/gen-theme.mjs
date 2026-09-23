import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import prettier from "prettier";
import { isDeepStrictEqual } from "node:util";
import { withoutMetadata } from "./registry-metadata.mjs";
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
.finstack-surface summary, .finstack-workbench summary { cursor:pointer; font-size:var(--text-xs); font-weight:500; letter-spacing:0.02em; color:var(--muted-foreground); padding-block:var(--spacing); }
.finstack-surface summary:hover, .finstack-workbench summary:hover { color:var(--foreground); }
.finstack-surface summary:focus-visible, .finstack-workbench summary:focus-visible { outline:2px solid var(--ring); outline-offset:2px; border-radius:var(--radius-sm); }
.finstack-surface [data-slot=button][aria-pressed=true] { background:var(--muted); color:var(--foreground); box-shadow:inset 0 0 0 var(--border-width) var(--border); }
.finstack-na { color:var(--muted-foreground); }
.finstack-surface table caption, .finstack-workbench table caption { caption-side:top; text-align:left; margin:0 0 var(--spacing); font-size:var(--text-xs); color:var(--muted-foreground); }
.finstack-term-sheet { container-type:inline-size; }
.finstack-quote-ticket {
  border: var(--border-width) solid var(--border);
  border-left: 3px solid var(--primary);
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--card) 88%, var(--accent));
  padding: calc(var(--spacing) * 2) calc(var(--spacing) * 3);
}
.finstack-quote-ticket__header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: calc(var(--spacing) * 2);
  padding-bottom: calc(var(--spacing) * 2);
  border-bottom: var(--table-rule-width) solid var(--hairline);
}
.finstack-quote-ticket__eyebrow,
.finstack-quote-ticket__category {
  font-size: var(--text-xs);
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}
.finstack-quote-ticket__eyebrow {
  color: var(--primary);
}
.finstack-quote-ticket__category {
  color: var(--muted-foreground);
}
.finstack-quote-ticket__body {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(205px, 44%);
  align-items: center;
  gap: calc(var(--spacing) * 4);
  padding-top: calc(var(--spacing) * 2);
}
.finstack-quote-ticket__description label {
  display: block;
  color: var(--foreground);
  font-size: var(--text-lg);
  font-weight: 600;
  line-height: 1.2;
}
.finstack-quote-ticket__label-row { display:flex; flex-wrap:wrap; align-items:baseline; gap:var(--spacing); }
.finstack-quote-ticket__tranche-id { color:var(--muted-foreground); font-family:var(--font-mono); font-size:var(--text-xs); }
.finstack-quote-ticket__description p {
  margin-top: var(--spacing);
  color: var(--muted-foreground);
  font-size: var(--text-xs);
  line-height: 1.35;
}
.finstack-quote-ticket__description .finstack-quote-ticket__replacement {
  color: var(--warning);
}
.finstack-quote-ticket__editor {
  min-width: 0;
}
.finstack-quote-ticket__entry {
  display: flex;
  align-items: center;
  gap: var(--spacing);
}
.finstack-quote-ticket__input-wrap {
  display: flex;
  align-items: center;
  flex: 1;
  min-width: 0;
  background: var(--card);
  border: var(--border-width) solid var(--border);
  border-radius: var(--radius-sm);
  padding-right: calc(var(--spacing) * 2);
}
.finstack-quote-ticket__input-wrap:focus-within {
  outline: 2px solid var(--ring);
  outline-offset: 1px;
}
.finstack-quote-ticket__body[data-invalid] .finstack-quote-ticket__input-wrap {
  border-color: var(--error);
}
.finstack-quote-ticket .finstack-quote-ticket__input {
  min-width: 0;
  height: 34px;
  border: 0;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
  padding: 0 calc(var(--spacing) * 2);
  color: var(--foreground);
  font-family: var(--font-mono);
  font-size: var(--text-lg);
  font-weight: 500;
}
.finstack-quote-ticket .finstack-quote-ticket__input:focus-visible {
  outline: none;
  box-shadow: none;
}
.finstack-quote-ticket__unit {
  flex: 0 0 auto;
  color: var(--muted-foreground);
  font-size: var(--text-xs);
  white-space: nowrap;
}
.finstack-quote-ticket__source {
  display: inline-flex;
  align-items: center;
  gap: calc(var(--spacing) * 1.5);
  margin-top: var(--spacing);
  color: var(--muted-foreground);
  font-size: var(--text-xs);
}
.finstack-quote-ticket__footer {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  justify-content: space-between;
  gap: 0 calc(var(--spacing) * 2);
}
.finstack-quote-ticket .finstack-quote-ticket__more {
  height: auto;
  min-height: 22px;
  padding: 0;
  font-size: var(--text-xs);
  white-space: nowrap;
}
.finstack-quote-ticket__source::before {
  content: "";
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--muted-foreground);
}
.finstack-quote-ticket__source[data-active]::before {
  background: var(--primary);
}
.finstack-quote-ticket__error {
  margin-top: var(--spacing);
  color: var(--error);
  font-size: var(--text-xs);
}
@container (max-width:520px) {
  .finstack-quote-ticket__body {
    grid-template-columns: minmax(0, 1fr);
    gap: calc(var(--spacing) * 2);
  }
}
@container (min-width:520px) {
  .finstack-workbench[data-density=compact] .finstack-quote-ticket {
    display:grid;
    grid-template-columns:minmax(0,1fr) minmax(205px,44%);
    column-gap:calc(var(--spacing) * 3);
    align-items:center;
    padding:var(--spacing) calc(var(--spacing) * 2.5);
  }
  .finstack-workbench[data-density=compact] .finstack-quote-ticket__header {
    grid-column:1;
    grid-row:1;
    align-self:end;
    border:0;
    padding:0;
  }
  .finstack-workbench[data-density=compact] .finstack-quote-ticket__eyebrow,
  .finstack-workbench[data-density=compact] .finstack-quote-ticket__category {
    font-size:10px;
    line-height:1;
  }
  .finstack-workbench[data-density=compact] .finstack-quote-ticket__body { display:contents; }
  .finstack-workbench[data-density=compact] .finstack-quote-ticket__description {
    grid-column:1;
    grid-row:2;
    align-self:start;
  }
  .finstack-workbench[data-density=compact] .finstack-quote-ticket__description label { font-size:var(--text-base); line-height:1.15; }
  .finstack-workbench[data-density=compact] .finstack-quote-ticket__description p { margin-top:1px; font-size:10px; line-height:1.15; }
  .finstack-workbench[data-density=compact] .finstack-quote-ticket__editor {
    grid-column:2;
    grid-row:1 / span 2;
    align-self:center;
  }
  .finstack-workbench[data-density=compact] .finstack-quote-ticket__input { height:28px; font-size:var(--text-base); }
  .finstack-workbench[data-density=compact] .finstack-quote-ticket__source { margin-top:0; font-size:10px; }
  .finstack-workbench[data-density=compact] .finstack-quote-ticket .finstack-quote-ticket__more { min-height:16px; font-size:10px; }
  .finstack-workbench[data-density=compact] .finstack-quote-ticket__footer { line-height:1.1; }
}
.finstack-editable-term { display:grid; grid-template-columns:minmax(0,1fr) auto; align-items:center; gap:var(--spacing); }
.finstack-fieldset { padding:0; margin:0 0 calc(var(--spacing) * 3); border:0; }
.finstack-fieldset > legend { float:left; margin-bottom:var(--spacing); font-size:var(--text-sm); font-weight:500; letter-spacing:0.02em; }
.finstack-fieldset > legend + * { clear:both; }
.finstack-fieldset > details { padding:var(--spacing) 0; font-size:var(--text-xs); }
.finstack-term-disclosure { margin-top:calc(var(--spacing) * 2); border-top:var(--table-rule-width) solid var(--hairline); padding-top:var(--spacing); }
.finstack-fieldset > details.finstack-term-disclosure { font-size:var(--text-sm); padding-top:var(--spacing); }
.finstack-term-disclosure > summary { cursor:pointer; width:fit-content; color:var(--muted-foreground); font-size:var(--text-sm); }
.finstack-schema-root > .finstack-term-disclosure { column-span:all; }
.finstack-term-fields { min-width:0; }
.finstack-term-sheet .finstack-field-frame { display:grid; grid-template-columns:minmax(100px,var(--term-width)) minmax(0,1fr); align-items:center; gap:0 calc(var(--spacing) * 2); min-height:var(--row-height); padding-block:calc(var(--spacing) / 2); }
.finstack-term-sheet .finstack-field-frame + .finstack-field-frame, .finstack-term-sheet .finstack-field-frame + .finstack-optional-term, .finstack-term-sheet .finstack-optional-term + .finstack-field-frame { border-top:var(--table-rule-width) solid var(--hairline); }
.finstack-term-sheet .finstack-field-label [data-slot=field-label] { color:var(--muted-foreground); font-weight:400; font-size:var(--text-sm); letter-spacing:0.01em; }
.finstack-term-sheet .finstack-field-frame[data-dirty] .finstack-field-label::after { content:''; width:6px; height:6px; border-radius:50%; background:var(--warning); flex:0 0 6px; }
.finstack-term-sheet .finstack-optional-term { min-height:var(--row-height); border-bottom:0; }
.finstack-term-sheet .finstack-optional-term > span { font-size:var(--text-sm); min-width:calc(var(--term-width) - var(--spacing) * 2); }
.finstack-field-label [data-slot=button], .finstack-field-label [data-slot=popover-trigger] { width:14px; height:14px; min-width:0; padding:0; border:var(--border-width) solid var(--border); border-radius:50%; color:var(--muted-foreground); font-size:9px; font-weight:600; line-height:1; opacity:0.7; }
.finstack-field-label :is([data-slot=button],[data-slot=popover-trigger]):hover, .finstack-field-label :is([data-slot=button],[data-slot=popover-trigger]):focus-visible { opacity:1; border-color:var(--muted-foreground); }
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
  .finstack-schema-root > .finstack-term-fields { columns:2; column-gap:calc(var(--spacing) * 8); column-fill:balance; }
  .finstack-schema-root > .finstack-term-fields > * { min-width:0; }
  .finstack-schema-root .finstack-field-frame, .finstack-schema-root .finstack-optional-term, .finstack-schema-root .finstack-fieldset > legend, .finstack-schema-root .finstack-array-row { break-inside:avoid; }
  .finstack-schema-root .finstack-fieldset > legend { break-after:avoid; }
}
@container (max-width:420px) {
  .finstack-term-sheet .finstack-field-frame { grid-template-columns:112px minmax(0,1fr); gap:var(--spacing); }
}
.finstack-money--prominent { color:var(--foreground); font-family:var(--font-sans); line-height:1.2; overflow-wrap:anywhere; }
.finstack-money__currency { font-family:var(--font-mono); font-size:var(--text-sm); font-weight:500; color:var(--muted-foreground); }
.finstack-money__amount { font-size:var(--text-2xl); font-weight:500; letter-spacing:-0.025em; }
.finstack-valuation { display:grid; gap:calc(var(--spacing) * 2); min-width:0; padding-block:var(--spacing) calc(var(--spacing) * 3); border-bottom:var(--border-width) solid var(--border); }
.finstack-valuation[data-compact] { gap:var(--spacing); }
.finstack-valuation__context { display:flex; flex-wrap:wrap; align-items:baseline; column-gap:calc(var(--spacing) * 3); row-gap:var(--spacing); color:var(--muted-foreground); font-size:var(--text-xs); }
.finstack-valuation__context > div { min-width:0; overflow-wrap:anywhere; }
.finstack-valuation__context dt,.finstack-valuation__context dd { display:inline; }
.finstack-measures { display:grid; gap:calc(var(--spacing) * 2); min-width:0; }
.finstack-measures [data-slot="table-row"] { border-color:var(--hairline); }
.finstack-measure-key { display:block; min-width:8ch; white-space:normal; overflow-wrap:anywhere; }
.finstack-workbench-container { container-type: inline-size; min-width: 0; }
.finstack-workbench { display:flex; flex-direction:column; min-width:0; background:var(--background); }
.finstack-workbench__bar { display:flex; flex-wrap:wrap; align-items:center; gap:calc(var(--spacing) * 2); padding:calc(var(--spacing) * 2) calc(var(--spacing) * 3); border-bottom:var(--border-width) solid var(--border); }
@container (min-width:720px) {
  .finstack-workbench[data-density=compact] .finstack-instrument-header > div:first-child { display:grid; grid-template-columns:190px auto; align-items:end; column-gap:calc(var(--spacing) * 2); }
  .finstack-workbench[data-density=compact] .finstack-instrument-header > div:first-child > :is(label,[role=group]) { grid-column:1; }
  .finstack-workbench[data-density=compact] .finstack-instrument-header > div:first-child > nav { grid-column:2; grid-row:2; display:flex; align-items:center; gap:var(--spacing); max-width:300px; overflow-x:auto; white-space:nowrap; }
  .finstack-workbench[data-density=compact] .finstack-instrument-header > div:first-child > nav [data-slot=button] { height:var(--control-height); }
}
.finstack-workbench__id { font-family:var(--font-mono); font-size:var(--text-sm); color:var(--muted-foreground); white-space:nowrap; }
.finstack-workbench__bar .finstack-pricing-context__date .finstack-field-control { width:180px; }
.finstack-kbd { display:inline-flex; align-items:center; justify-content:center; min-width:14px; height:14px; padding:0 3px; margin-right:calc(var(--spacing) * 0.5); border:var(--border-width) solid var(--border); border-radius:var(--radius-sm); font-family:var(--font-mono); font-size:10px; line-height:1; color:var(--muted-foreground); background:var(--background); }
[data-active] > .finstack-kbd, [aria-selected=true] > .finstack-kbd { color:var(--primary); border-color:var(--primary); }
.finstack-workbench__stale { display:inline-flex; align-items:center; gap:calc(var(--spacing) * 1.5); font-size:var(--text-xs); color:var(--warning); }
.finstack-workbench__stale::before { content:''; width:5px; height:5px; border-radius:50%; background:var(--warning); }
.finstack-workbench[data-stale] .finstack-workbench__summary-body, .finstack-workbench[data-stale] .finstack-workbench__detail > [role=tabpanel] { opacity:0.55; transition:opacity 120ms; }
.finstack-workbench__brand { font-family:var(--font-mono); font-size:var(--text-base); font-weight:500; color:var(--primary); }
.finstack-workbench__tabs { max-width:100%; overflow-x:auto; }
.finstack-workbench__context { margin-left:auto; }
.finstack-workbench__inputs,.finstack-workbench__panel { min-width:0; min-height:0; }
.finstack-workbench__panel { padding:calc(var(--spacing) * 2) calc(var(--spacing) * 4); }
.finstack-workbench__results { display:grid; grid-template-columns:minmax(0,1fr); min-height:0; border-top:var(--border-width) solid var(--border); }
.finstack-workbench__summary { display:flex; flex-direction:column; min-width:0; min-height:0; }
.finstack-workbench__results-header { display:flex; flex-wrap:wrap; align-items:center; justify-content:space-between; gap:calc(var(--spacing) * 2); min-height:calc(var(--spacing) * 10); padding:var(--spacing) calc(var(--spacing) * 4); border-bottom:var(--border-width) solid var(--border); background:var(--card); flex-shrink:0; }
.finstack-title { font-size:var(--text-sm); font-weight:500; letter-spacing:0.02em; }
.finstack-workbench__summary-body { min-width:0; min-height:0; padding:calc(var(--spacing) * 2) calc(var(--spacing) * 4); }
.finstack-workbench[data-density=compact] .finstack-workbench__summary-body { padding-top:calc(var(--spacing) * 2); }
.finstack-workbench[data-density=compact] .finstack-valuation[data-compact] { grid-template-columns:minmax(0,1fr); grid-template-rows:auto auto auto auto; row-gap:var(--spacing); padding-block:0 calc(var(--spacing) * 2); }
.finstack-workbench[data-density=compact] .finstack-valuation[data-compact] > h2 { grid-column:1; grid-row:1; }
.finstack-workbench[data-density=compact] .finstack-valuation__value { grid-column:1; grid-row:2; align-self:start; }
.finstack-workbench[data-density=compact] .finstack-valuation__context { grid-column:1; grid-row:3; row-gap:var(--spacing); }
.finstack-workbench[data-density=compact] .finstack-valuation__context > div { gap:0; }
.finstack-workbench[data-density=compact] .finstack-valuation[data-compact] > :last-child { grid-column:1; grid-row:4; }
.finstack-workbench__key-results { display:grid; grid-template-columns:repeat(auto-fit,minmax(min(100%,140px),1fr)); gap:calc(var(--spacing) * 1.5); margin:0; }
.finstack-workbench__key-results > div { min-width:0; padding:calc(var(--spacing) * 1.5) calc(var(--spacing) * 2); background:var(--card); border:var(--border-width) solid var(--hairline); border-top:2px solid var(--primary); border-radius:var(--radius-sm); }
.finstack-workbench__key-results dt { color:var(--muted-foreground); font-size:var(--text-xs); line-height:1.3; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
.finstack-workbench__key-results dd { margin-top:var(--spacing); font-size:var(--text-base); font-weight:500; line-height:1.2; overflow-wrap:anywhere; }
.finstack-workbench__key-results dd > span { display:flex; flex-wrap:wrap; align-items:baseline; column-gap:var(--spacing); font-family:var(--font-mono); font-size:inherit; font-weight:inherit; }
.finstack-workbench__key-results dd > span > :not(.finstack-na) { white-space:nowrap; }
.finstack-workbench__key-results dd .finstack-na { margin-left:0; font-family:var(--font-sans); font-size:var(--text-xs); font-weight:400; }
.finstack-workbench[data-density=compact] .finstack-measures { gap:calc(var(--spacing) * 1.5); }
.finstack-workbench[data-density=compact] .finstack-measures__toolbar { flex-wrap:nowrap; gap:var(--spacing); padding-block:var(--spacing); overflow-x:auto; }
.finstack-workbench[data-density=compact] .finstack-measures__toolbar > span, .finstack-workbench[data-density=compact] .finstack-measures__toolbar > [role=group] { flex-shrink:0; }
.finstack-workbench__detail { min-width:0; min-height:0; border-top:var(--border-width) solid var(--border); }
.finstack-workbench__detail > [role=tabpanel] { padding:calc(var(--spacing) * 2) calc(var(--spacing) * 4); }
.finstack-workbench__status { display:flex; flex-wrap:wrap; gap:calc(var(--spacing) * 3); padding:calc(var(--spacing) * 1.25) calc(var(--spacing) * 3); border-top:var(--border-width) solid var(--border); background:var(--card); font-size:var(--text-xs); color:var(--muted-foreground); }
.finstack-workbench__status .font-mono { color:var(--foreground); }
.finstack-workbench__state { display:inline-flex; gap:calc(var(--spacing) * 1.5); align-items:center; height:var(--control-height); padding:0 calc(var(--spacing) * 2); border:var(--border-width) solid var(--border); border-radius:var(--radius-md); font-size:var(--text-xs); white-space:nowrap; }
.finstack-workbench__state::before { content:''; width:5px; height:5px; border-radius:50%; background:var(--muted-foreground); }
.finstack-workbench__state[data-state=priced]::before { background:var(--positive); }
.finstack-workbench__state[data-state=pending]::before { background:var(--warning); animation:finstack-pulse 0.8s ease-in-out infinite; }
.finstack-workbench__state[data-state=failed] { color:var(--error); border-color:var(--error); }
.finstack-workbench__state[data-state=failed]::before { background:var(--error); }
@keyframes finstack-pulse { 0%, 100% { opacity:1; } 50% { opacity:0.25; } }
@media (prefers-reduced-motion: reduce) { .finstack-workbench__state::before { animation:none !important; } }
.finstack-market { container-type:inline-size; min-width:0; }
.finstack-market__layout { display:grid; grid-template-columns:minmax(0,1fr); gap:16px; }
.finstack-market__rail { min-width:0; font-size:var(--text-sm); }
.finstack-market__list-heading { display:flex; justify-content:space-between; gap:8px; margin-bottom:4px; color:var(--muted-foreground); font-size:var(--text-xs); }
.finstack-market__references { max-height:260px; overflow:auto; }
.finstack-market__reference { display:grid; width:100%; height:auto; justify-content:stretch; gap:2px; padding:7px 8px; text-align:left; white-space:normal; }
.finstack-market__reference-name { font-family:var(--font-mono); font-weight:600; }
.finstack-market__reference-detail { color:var(--muted-foreground); font-size:var(--text-xs); }
.finstack-market__rail [data-slot=button] { border-left:var(--rail-width) solid transparent; border-radius:0 var(--radius-sm) var(--radius-sm) 0; }
.finstack-market__rail [data-slot=button][aria-pressed=true] { border-left-color:var(--primary); background:var(--accent); color:var(--foreground); box-shadow:none; }
.finstack-market__category { padding:calc(var(--spacing) * 1.5) 0; border-bottom:var(--table-rule-width) solid var(--hairline); }
.finstack-market__all-fields { margin-top:12px; padding-top:8px; border-top:var(--table-rule-width) solid var(--hairline); font-size:var(--text-xs); }
.finstack-market__all-fields > summary { cursor:pointer; }
.finstack-market__all-fields nav { margin-top:8px; }
.finstack-market__selected { min-width:0; }
.finstack-market__selected > header { display:flex; flex-wrap:wrap; justify-content:space-between; align-items:center; gap:8px; margin-bottom:12px; }
.finstack-market__selected > header h2 { font-family:var(--font-mono); font-size:var(--text-sm); }
.finstack-market__conventions { display:flex; flex-wrap:wrap; gap:8px 18px; margin-bottom:6px; font-size:var(--text-xs); }
.finstack-market__conventions div { display:flex; gap:5px; }
.finstack-market__conventions dt, .finstack-market__provenance { color:var(--muted-foreground); }
.finstack-market__conventions dd { font-family:var(--font-mono); }
.finstack-market__provenance { margin-bottom:10px; font-size:var(--text-xs); }
@container (max-width:719px) { .finstack-market__references { max-height:160px; } }
@container (min-width:720px) { .finstack-market__layout { grid-template-columns:280px minmax(0,1fr); } .finstack-market__rail { max-height:380px; overflow:auto; border-right:1px solid var(--border); padding-right:12px; } }
@container (max-width:959px) { .finstack-workbench__bar { position:sticky; top:0; z-index:10; background:var(--background); } }
@container (max-width:1180px) { .finstack-workbench__bar .finstack-pricing-context .finstack-field-label { position:absolute; width:1px; height:1px; overflow:hidden; clip:rect(0 0 0 0); white-space:nowrap; } .finstack-workbench__bar .finstack-pricing-context { gap:calc(var(--spacing) * 1.5); } }
@container (min-width:960px) {
[data-registry-focus] .finstack-workbench { height:calc(100dvh - 2px); min-height:620px; }
.finstack-workbench { height:min(820px,calc(100dvh - 56px)); min-height:620px; display:grid; grid-template-rows:auto minmax(0,44fr) minmax(0,56fr) auto; }
.finstack-workbench[data-density=compact] { grid-template-rows:auto minmax(0,47fr) minmax(0,53fr) auto; }
.finstack-workbench__bar { gap:calc(var(--spacing) * 1.5); }
.finstack-workbench__inputs { overflow:hidden; }
.finstack-workbench__panel { height:100%; overflow:auto; }
.finstack-workbench__results { grid-template-columns:minmax(0,38fr) minmax(0,62fr); overflow:hidden; }
.finstack-workbench__summary-body { overflow:auto; }
.finstack-workbench__detail { display:flex; flex-direction:column; border-top:0; border-left:1px solid var(--border); overflow:hidden; }
.finstack-workbench__detail > [role=tabpanel] { flex:1; min-height:0; overflow:auto; }
.finstack-workbench[data-density=compact] .finstack-valuation__value .finstack-money__amount { white-space:nowrap; }
}
@media (max-height:820px) {
  @container (min-width:960px) {
    .finstack-workbench { grid-template-rows:auto minmax(0,50fr) minmax(0,50fr) auto; }
    .finstack-workbench[data-density=compact] { grid-template-rows:auto minmax(0,55fr) minmax(0,45fr) auto; }
  }
}
/* Settle native SVG geometry at a physical width before the print dialog opens. */
[data-printing] .finstack-market__layout, [data-printing] .finstack-stored-curve__layout { display:block; }
[data-printing] .finstack-market__rail { display:none; }
[data-printing] .finstack-market__selected { width:180mm; max-width:none; }
[data-printing] [data-finstack-figure] { width:180mm; max-width:100%; }
@media print { .finstack-workbench { display:block; height:auto; min-height:0; } .finstack-workbench__inputs,.finstack-workbench__panel,.finstack-workbench__results,.finstack-workbench__summary,.finstack-workbench__summary-body,.finstack-workbench__detail { display:block; height:auto; overflow:visible; } .finstack-workbench__bar,.finstack-workbench__status { display:none; } }

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
      const actual = await readFile(path.join(root, file), "utf8");
      if (
        file.endsWith(".json")
          ? !isDeepStrictEqual(
              withoutMetadata(JSON.parse(actual)),
              JSON.parse(content),
            )
          : actual !== content
      )
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
