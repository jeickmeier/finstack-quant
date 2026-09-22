import { expect, it } from "vitest";
import { readFile } from "node:fs/promises";
import { themeOutput, generateTheme } from "../../scripts/gen-theme.mjs";
import {
  checkContrast,
  checkStockContrast,
  checkLiterals,
  tenantPalettes,
} from "../../scripts/check-tokens.mjs";
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
  expect(themeOutput(tokens).item.cssVars).toEqual({
    theme: tokens.theme,
    light: tokens.light,
    dark: tokens.dark,
  });
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
