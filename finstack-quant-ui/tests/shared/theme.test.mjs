import { expect, it } from "vitest";
import { mkdtemp, readdir, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import prettier from "prettier";
import postcss from "postcss";
import { parse } from "culori";
import {
  themeOutput,
  generateBrand,
  generateTheme,
} from "../../scripts/gen-theme.mjs";
import {
  checkContrast,
  checkStockContrast,
  checkLiterals,
  mergeBrand,
  tenantPalettes,
  validateTokens,
} from "../../scripts/check-tokens.mjs";
const brandExample = new URL(
  "../../registry/theme/finstack-theme/brand.example.json",
  import.meta.url,
);
const tokens = JSON.parse(
  await readFile(
    new URL("../../registry/theme/finstack-theme/tokens.json", import.meta.url),
    "utf8",
  ),
);
it("generates matching modes deterministically from one source", async () => {
  expect(Object.keys(tokens.light).sort()).toEqual(
    Object.keys(tokens.dark).sort(),
  );
  expect(themeOutput(tokens)).toEqual(themeOutput(structuredClone(tokens)));
  await expect(generateTheme(true)).resolves.toHaveLength(2);
  // The CLI would append cssVars to the application stylesheet after the theme
  // import, overriding a brand stylesheet; theme.css owns every token instead.
  expect(themeOutput(tokens).item).not.toHaveProperty("cssVars");
});
it("keeps contrast in the default and single-stylesheet tenant themes", async () => {
  const css = await readFile(
    new URL("./fixtures/tenant.css", import.meta.url),
    "utf8",
  );
  for (const palette of [
    tokens.light,
    tokens.dark,
    ...Object.values(tenantPalettes(tokens, css)),
  ])
    expect(() => checkContrast(palette)).not.toThrow();
  expect(() =>
    checkContrast({ ...tokens.light, foreground: tokens.light.background }),
  ).toThrow(/Contrast/);
  expect(() =>
    checkContrast({
      ...tokens.light,
      input: tokens.light.background,
    }),
  ).toThrow(/Contrast/);
  expect(() =>
    checkContrast({ ...tokens.light, border: tokens.light.background }),
  ).not.toThrow();
  expect(() =>
    checkContrast({ ...tokens.dark, "chart-1": tokens.dark.card }),
  ).toThrow(/Contrast/);
  expect(() =>
    checkContrast({ ...tokens.light, foreground: "rgba(0,0,0,0.1)" }),
  ).toThrow(/opaque/);
});
it("keeps stock inactive Tabs contrast after opacity compositing", async () => {
  const css = await readFile(
    new URL("./fixtures/tenant.css", import.meta.url),
    "utf8",
  );
  for (const palettes of [tokens, tenantPalettes(tokens, css)])
    for (const mode of ["light", "dark"])
      expect(() => checkStockContrast(palettes[mode], mode)).not.toThrow();
  const previous = { ...tokens.light, foreground: "#202b29" };
  expect(() => checkContrast(previous)).not.toThrow();
  expect(() => checkStockContrast(previous, "light")).toThrow(
    /Stock Tabs contrast/,
  );
});

it.each([
  [".a { border: 1px solid red; }", "test.css"],
  [".a { --font-sans: Georgia, serif; }", "test.css"],
  ['const className = "bg-red-500 text-white";', "test.tsx"],
  [".a { color: #abc; }", "test.css"],
  [".a { color: rgb(1 2 3); }", "test.css"],
  [".a { color: red; }", "test.css"],
  [".a { font-family: Arial; }", "test.css"],
  ['const style = { fontFamily: "Arial" };', "test.tsx"],
  ['const style = { color: "oklch(0.5 0.2 90)" };', "test.tsx"],
])("rejects colour/font literals outside the theme %#", (source, name) =>
  expect(() => checkLiterals(source, name)).toThrow(/literal/),
);
it("allows token references and computed geometry", () => {
  expect(() =>
    checkLiterals(
      ".a { color: var(--foreground); font-family: var(--font-sans); width: 100%; }",
      "test.css",
    ),
  ).not.toThrow();
  expect(() =>
    checkLiterals(
      'const style = { color: "var(--primary)", width: width / 2 };',
      "test.tsx",
    ),
  ).not.toThrow();
  expect(() =>
    checkLiterals('const label = "Fade to target";', "test.tsx"),
  ).not.toThrow();
});
it("pins licensed font packages and ships no font binaries", async () => {
  const { item } = themeOutput(tokens);
  expect(item.files.every((file) => file.path.endsWith(".css"))).toBe(true);
  for (const font of tokens.fonts) {
    const base = new URL(
      `../../node_modules/${font.package}/`,
      import.meta.url,
    );
    const metadata = JSON.parse(
      await readFile(new URL("package.json", base), "utf8"),
    );
    expect(metadata.version).toBe(font.version);
    expect(await readFile(new URL("LICENSE", base), "utf8")).toContain(
      "SIL Open Font License, Version 1.1",
    );
    for (const face of font.faces)
      expect(await readFile(new URL(face, base), "utf8")).toContain(
        `font-family: '${font.family}'`,
      );
  }
});
it("derives every colour literal in the generated stylesheet from a token", () => {
  const { css } = themeOutput(tokens);
  const values = new Set(
    [...Object.values(tokens.light), ...Object.values(tokens.dark)].map((v) =>
      v.toLowerCase(),
    ),
  );
  const literals = [];
  postcss.parse(css).walkDecls((decl) => {
    for (const match of decl.value.matchAll(
      /#[0-9a-f]{3,8}\b|\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\s*\([^)]*\)/gi,
    ))
      literals.push(match[0].toLowerCase());
    for (const word of decl.value.split(/[\s,()/]+/))
      if (
        /^[a-z]+$/i.test(word) &&
        !["transparent", "currentcolor"].includes(word.toLowerCase()) &&
        parse(word)?.mode
      )
        literals.push(word.toLowerCase());
  });
  expect(literals.length).toBeGreaterThan(0);
  expect(literals.filter((literal) => !values.has(literal))).toEqual([]);
});
it("declares the colour scheme and the native chart overlay hooks from tokens", () => {
  const { css } = themeOutput(tokens);
  expect(css).toContain(':root, [data-theme="light"] { color-scheme: light;');
  expect(css).toContain(
    ':root.dark, [data-theme="dark"] { color-scheme: dark;',
  );
  for (const hook of [
    "--ts-chart-tooltip-background: var(--popover)",
    "--ts-chart-tooltip-color: var(--popover-foreground)",
    "--ts-chart-tooltip-font: 500 var(--text-xs)/1.3 var(--font-sans)",
    "--ts-chart-tooltip-shadow: var(--elevation)",
    "--ts-chart-crosshair-label-halo: var(--background)",
  ])
    expect(css).toContain(hook);
});
it("gives every theme token outside the Tailwind namespaces a consumer", async () => {
  const generator = await readFile(
    new URL("../../scripts/gen-theme.mjs", import.meta.url),
    "utf8",
  );
  // Font, text, spacing and radius keys are consumed by the Tailwind utilities
  // of every consumer; the rest must be referenced by the template or a source.
  const tailwind = /^(font-(?:sans|mono)|text-|spacing$|radius$)/;
  const sources = [];
  for (const directory of ["registry", "src"]) {
    const base = new URL(`../../${directory}/`, import.meta.url);
    for (const entry of await readdir(base, {
      recursive: true,
      withFileTypes: true,
    }))
      if (
        entry.isFile() &&
        /\.(tsx?|mjs|css)$/.test(entry.name) &&
        !entry.parentPath.includes("generated")
      )
        sources.push(
          await readFile(path.join(entry.parentPath, entry.name), "utf8"),
        );
  }
  const text = sources.join("\n");
  expect(
    Object.keys(tokens.theme).filter(
      (key) =>
        !tailwind.test(key) &&
        !generator.includes(`var(--${key})`) &&
        !text.includes(`var(--${key})`),
    ),
  ).toEqual([]);
});
it("merges a brand over the defaults and rejects unknown or half-themed keys", async () => {
  const brand = JSON.parse(await readFile(brandExample, "utf8"));
  const merged = mergeBrand(tokens, brand);
  expect(merged.light.primary).toBe(brand.light.primary);
  expect(merged.dark.primary).toBe(brand.dark.primary);
  expect(merged.theme.radius).toBe(brand.theme.radius);
  expect(merged.light.background).toBe(tokens.light.background);
  expect(merged.fonts).toEqual(tokens.fonts);
  expect(() => validateTokens(merged)).not.toThrow();
  expect(mergeBrand(tokens, { name: "label" })).toEqual(tokens);
  expect(() => mergeBrand(tokens, { logo: "wordmark.svg" })).toThrow(
    /Unknown brand section/,
  );
  expect(() => mergeBrand(tokens, { $schema: "x" })).toThrow(
    /Unknown brand section/,
  );
  expect(() =>
    mergeBrand(tokens, { light: { brand: "#000" }, dark: { brand: "#fff" } }),
  ).toThrow(/Unknown light token/);
  expect(() =>
    mergeBrand(tokens, {
      light: { constructor: "#000" },
      dark: { constructor: "#fff" },
    }),
  ).toThrow(/Unknown light token/);
  expect(() =>
    mergeBrand(tokens, JSON.parse('{"light":{"__proto__":"#000"}}')),
  ).toThrow(/Unknown light token/);
  expect(() => mergeBrand(tokens, { light: { primary: "#5146a5" } })).toThrow(
    /both light and dark/,
  );
  expect(() => mergeBrand(tokens, { light: "#000" })).toThrow(
    /must be an object/,
  );
  expect(() => mergeBrand(tokens, { density: { cosy: {} } })).toThrow(
    /Unknown brand section: density.cosy/,
  );
  expect(() => mergeBrand(tokens, { fonts: [{ package: "x" }] })).toThrow(
    /missing version/,
  );
  expect(() =>
    mergeBrand(tokens, {
      fonts: [{ ...tokens.fonts[0], faces: "latin-400.css" }],
    }),
  ).toThrow(/faces/);
  expect(() =>
    mergeBrand(tokens, { fonts: [{ ...tokens.fonts[0], weight: 400 }] }),
  ).toThrow(/Unknown brand font field/);
  expect(() => mergeBrand(tokens, { fonts: [tokens.fonts[0]] })).toThrow(
    /no longer supply "IBM Plex Mono", which theme.font-mono names/,
  );
  expect(() => mergeBrand(tokens, { fonts: [] })).not.toThrow();
  expect(() =>
    validateTokens(
      mergeBrand(tokens, {
        light: { primary: tokens.light.background },
        dark: { primary: tokens.dark.background },
      }),
    ),
  ).toThrow(/Contrast/);
});
it("renders a brand through the whole template, print block and font imports included", () => {
  const brand = {
    theme: {
      "font-sans": '"Brand Sans", sans-serif',
      "font-mono": '"Brand Mono", monospace',
    },
    light: { primary: "#5146a5", ring: "#5146a5" },
    dark: { primary: "#b7acf2", ring: "#b7acf2" },
    fonts: [
      {
        package: "@fontsource/brand-sans",
        version: "1.0.0",
        family: "Brand Sans",
        faces: ["latin-400.css"],
        license: "OFL-1.1",
      },
      {
        package: "@fontsource/brand-mono",
        version: "1.0.0",
        family: "Brand Mono",
        faces: ["latin-400.css"],
        license: "OFL-1.1",
      },
    ],
  };
  const { css, item } = themeOutput(mergeBrand(tokens, brand));
  expect(css).toContain('@import "@fontsource/brand-sans/latin-400.css"');
  expect(css).toContain('@import "@fontsource/brand-mono/latin-400.css"');
  expect(css).not.toContain("ibm-plex");
  // The light value lands in the root block and both print selectors; the dark value once.
  expect(css.match(/--primary: #5146a5;/g)).toHaveLength(3);
  expect(css.match(/--primary: #b7acf2;/g)).toHaveLength(1);
  expect(css).toContain('--font-sans: "Brand Sans", sans-serif;');
  expect(item.dependencies).toContain("@fontsource/brand-sans@1.0.0");
  expect(item).not.toHaveProperty("cssVars");
});
it("writes a validated brand stylesheet through the generator entry point", async () => {
  const dir = await mkdtemp(path.join(tmpdir(), "finstack-brand-"));
  try {
    const out = path.join(dir, "theme.css");
    const { name, tokens: merged } = await generateBrand(
      fileURLToPath(brandExample),
      out,
    );
    expect(name).toBe("example-brand");
    const css = await readFile(out, "utf8");
    expect(css).toBe(
      await prettier.format(themeOutput(merged).css, { parser: "css" }),
    );
    expect(css).toContain("--primary: #5146a5;");
    expect(css).toContain("--radius: 0.5rem;");
  } finally {
    await rm(dir, { recursive: true });
  }
});
