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
  const css = `${imports}
:root { ${declarations(tokens.theme)} ${declarations(tokens.density.compact)} }
:root, [data-theme="light"] { ${declarations(tokens.light)} }
[data-theme="dark"] { ${declarations(tokens.dark)} }
[data-density="compact"] { ${declarations(tokens.density.compact)} }
[data-density="comfortable"] { ${declarations(tokens.density.comfortable)} }
@theme inline { ${Object.keys(tokens.light)
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
@utility finstack-field { min-height: var(--field-height); }
@utility finstack-surface { background: var(--background); color: var(--foreground); font-family: var(--font-sans); font-size: var(--text-base); }
`;
  const item = {
    name: "finstack-theme",
    type: "registry:theme",
    docs: "Shared light/dark theme. Import styles/finstack/theme.css after Tailwind. Set data-theme and data-density on the app root. Tenant styles override the same properties after this stylesheet. IBM Plex Sans and Mono are supplied by pinned Fontsource packages under SIL OFL-1.1; no font binaries are copied into registry files.",
    dependencies: tokens.fonts.map((font) => `${font.package}@${font.version}`),
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
