import { readFile, readdir } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { converter, parse, wcagContrast } from "culori";
import ts from "typescript";
import postcss from "postcss";
import { byCodeUnit } from "./order.mjs";

export function checkContrast(palette) {
  for (const [key, value] of Object.entries(palette)) {
    const color = parse(value);
    if (!color || (color.alpha !== undefined && color.alpha !== 1))
      throw new Error(`Expected opaque colour: ${key}`);
  }
  const check = (a, b, minimum) => {
    if (
      !palette[a] ||
      !palette[b] ||
      wcagContrast(palette[a], palette[b]) < minimum
    )
      throw new Error(`Contrast below ${minimum}: ${a}/${b}`);
  };
  for (const bg of ["background", "card"]) {
    for (const fg of [
      "foreground",
      "muted-foreground",
      "positive",
      "negative",
      "warning",
      "error",
      "info",
      "par",
    ])
      check(fg, bg, 4.5);
    for (const mark of [
      "input",
      "ring",
      ...Array.from({ length: 6 }, (_, i) => `chart-${i + 1}`),
    ])
      check(mark, bg, 3);
  }
  for (const bg of ["primary", "accent", "card", "popover", "secondary"])
    check(`${bg}-foreground`, bg, 4.5);
  check("muted-foreground", "muted", 4.5);
  for (const fill of Object.keys(palette).filter((key) =>
    /^(sequential-|diverging-)/.test(key),
  )) {
    if (
      Math.max(
        wcagContrast(palette[fill], palette["cell-light"]),
        wcagContrast(palette[fill], palette["cell-dark"]),
      ) < 4.5
    )
      throw new Error(`Cell text contrast: ${fill}`);
  }
}

// Stock Tabs use foreground/60 in light mode and muted-foreground in dark mode.
export function checkStockContrast(palette, mode) {
  const rgb = converter("rgb");
  const foreground = rgb(
    palette[mode === "light" ? "foreground" : "muted-foreground"],
  );
  const opacity = mode === "light" ? 0.6 : 1;
  for (const key of ["muted", "background"]) {
    const background = rgb(palette[key]);
    const painted = {
      mode: "rgb",
      r: foreground.r * opacity + background.r * (1 - opacity),
      g: foreground.g * opacity + background.g * (1 - opacity),
      b: foreground.b * opacity + background.b * (1 - opacity),
    };
    if (wcagContrast(painted, background) < 4.5)
      throw new Error(`Stock Tabs contrast below 4.5: ${mode}/${key}`);
  }
}

export function checkLiterals(source, name) {
  const checkValue = (value) => {
    if (
      /(?:#[\da-f]{3,8}\b|\b(?:rgb|hsl|hwb|lab|lch|oklab|oklch|color)\s*\()/i.test(
        value,
      ) ||
      // culori also accepts unprefixed hex ("fade", "bdc"); hex needs "#" (matched above).
      value
        .split(/\s+/)
        .some((part) => !/^[\da-f]+$/i.test(part) && parse(part)?.mode) ||
      /(?:bg|text|border|ring|fill|stroke)-(?:red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose|slate|gray|zinc|neutral|stone|white|black)(?:-\d+)?(?:\b|$)/.test(
        value,
      )
    )
      throw new Error(`Colour literal outside theme: ${name}`);
  };
  if (name.endsWith(".css")) {
    postcss.parse(source).walkDecls((decl) => {
      if (
        decl.prop.includes("font") &&
        !/^(var\(|inherit|initial|unset)/.test(decl.value) &&
        /family|sans|mono|^font$/.test(decl.prop)
      )
        throw new Error(`Font literal outside theme: ${name}`);
      checkValue(decl.value);
    });
  } else {
    const ast = ts.createSourceFile(name, source, ts.ScriptTarget.Latest, true);
    const visit = (node) => {
      if (ts.isStringLiteralLike(node)) checkValue(node.text);
      if (
        ts.isPropertyAssignment(node) &&
        ["fontFamily", "font"].includes(
          node.name.getText(ast).replaceAll(/["']/g, ""),
        ) &&
        ts.isStringLiteralLike(node.initializer) &&
        !node.initializer.text.startsWith("var(")
      )
        throw new Error(`Font literal outside theme: ${name}`);
      ts.forEachChild(node, visit);
    };
    visit(ast);
  }
}
/** Keys a brand file may set, each with the same shape as tokens.json. */
const brandSections = new Set(["theme", "light", "dark", "density", "fonts"]);
const fontFields = ["package", "version", "family", "faces", "license"];
const isPlainObject = (value) =>
  value !== null && typeof value === "object" && !Array.isArray(value);

function brandFont(font) {
  if (!isPlainObject(font))
    throw new Error("Each brand font must be an object");
  for (const key of Object.keys(font))
    if (!fontFields.includes(key))
      throw new Error(`Unknown brand font field: ${key}`);
  for (const field of fontFields) {
    if (!Object.hasOwn(font, field))
      throw new Error(`Brand font is missing ${field}`);
    const value = font[field];
    if (field === "faces") {
      if (
        !Array.isArray(value) ||
        value.length === 0 ||
        value.some((face) => typeof face !== "string" || face.trim() === "")
      )
        throw new Error(
          "Brand font faces must be a non-empty list of stylesheet names",
        );
    } else if (typeof value !== "string" || value.trim() === "")
      throw new Error(`Brand font ${field} must be a non-empty string`);
  }
  return structuredClone(font);
}

/**
 * Merge a brand file over the default tokens. Only existing keys may be set;
 * `fonts` replaces the Fontsource list (an empty list means the host supplies
 * its fonts) and a non-empty list must still supply every family the theme
 * fonts name; a palette key set in one mode must be set in the other so a
 * brand cannot ship a half-themed dark mode.
 */
export function mergeBrand(tokens, brand) {
  if (!isPlainObject(brand))
    throw new Error("A brand file must hold a JSON object");
  if (Object.hasOwn(brand, "name") && typeof brand.name !== "string")
    throw new Error("Brand name must be a string");
  const merged = structuredClone(tokens);
  for (const section of Object.keys(brand)) {
    if (section === "name") continue;
    if (!brandSections.has(section))
      throw new Error(`Unknown brand section: ${section}`);
    const values = brand[section];
    if (section === "fonts") {
      if (!Array.isArray(values))
        throw new Error("Brand fonts must be an array of Fontsource entries");
      merged.fonts = values.map(brandFont);
      continue;
    }
    if (!isPlainObject(values))
      throw new Error(`Brand ${section} must be an object`);
    const scopes =
      section === "density"
        ? Object.keys(values).map((mode) => [
            `density.${mode}`,
            Object.hasOwn(merged.density, mode) ? merged.density[mode] : null,
            values[mode],
          ])
        : [[section, merged[section], values]];
    for (const [label, target, entries] of scopes) {
      if (!isPlainObject(target))
        throw new Error(`Unknown brand section: ${label}`);
      if (!isPlainObject(entries))
        throw new Error(`Brand ${label} must be an object`);
      for (const key of Object.keys(entries)) {
        if (!Object.hasOwn(target, key))
          throw new Error(`Unknown ${label} token: ${key}`);
        const value = entries[key];
        if (typeof value !== "string" || value.trim() === "")
          throw new Error(`Brand ${label}.${key} must be a non-empty string`);
        target[key] = value;
      }
    }
  }
  const light = Object.keys(brand.light ?? {}).sort(byCodeUnit);
  const dark = Object.keys(brand.dark ?? {}).sort(byCodeUnit);
  if (light.join() !== dark.join())
    throw new Error(
      "Brand palette keys must be set in both light and dark modes",
    );
  if (merged.fonts.length) {
    const supplied = new Set(merged.fonts.map((font) => font.family));
    for (const { family } of tokens.fonts)
      if (!supplied.has(family))
        for (const key of ["font-sans", "font-mono"])
          if (merged.theme[key].includes(family))
            throw new Error(
              `Brand fonts no longer supply "${family}", which theme.${key} names; add its package or set theme.${key}`,
            );
  }
  return merged;
}

/** Key parity plus the contrast rules every shipped palette must meet. */
export function validateTokens(tokens) {
  if (
    Object.keys(tokens.light).sort(byCodeUnit).join() !==
    Object.keys(tokens.dark).sort(byCodeUnit).join()
  )
    throw new Error("Light/dark token keys differ");
  for (const mode of ["light", "dark"]) {
    checkContrast(tokens[mode]);
    checkStockContrast(tokens[mode], mode);
  }
}
export function tenantPalettes(tokens, css) {
  const palettes = { light: { ...tokens.light }, dark: { ...tokens.dark } };
  postcss.parse(css).walkRules((rule) => {
    for (const mode of ["light", "dark"])
      if (
        rule.selector.includes(`[data-theme="${mode}"]`) ||
        (mode === "light" && rule.selector.includes(":root"))
      )
        rule.walkDecls((decl) => {
          if (
            decl.prop.startsWith("--") &&
            decl.prop.slice(2) in palettes[mode]
          )
            palettes[mode][decl.prop.slice(2)] = decl.value;
        });
  });
  return palettes;
}
export async function checkTokens(root) {
  const tokens = JSON.parse(
    await readFile(
      path.join(root, "registry/theme/finstack-theme/tokens.json"),
      "utf8",
    ),
  );
  validateTokens(tokens);
  validateTokens(
    mergeBrand(
      tokens,
      JSON.parse(
        await readFile(
          path.join(root, "registry/theme/finstack-theme/brand.example.json"),
          "utf8",
        ),
      ),
    ),
  );
  const tenant = tenantPalettes(
    tokens,
    await readFile(path.join(root, "tests/shared/fixtures/tenant.css"), "utf8"),
  );
  for (const [mode, palette] of Object.entries(tenant)) {
    checkContrast(palette);
    checkStockContrast(palette, mode);
  }
  for (const directory of ["src", "registry"]) {
    const files = await readdir(path.join(root, directory), {
      recursive: true,
      withFileTypes: true,
    });
    for (const file of files) {
      const absolute = path.join(file.parentPath, file.name);
      const relative = path.relative(root, absolute);
      if (
        !file.isFile() ||
        /^(registry\/theme\/|src\/generated\/)/.test(relative) ||
        !/\.(css|[cm]?[jt]sx?)$/.test(file.name)
      )
        continue;
      checkLiterals(await readFile(absolute, "utf8"), relative);
    }
  }
}
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  await checkTokens(fileURLToPath(new URL("../", import.meta.url)));
  console.log(
    "Checked theme, brand example, tenant contrast and source token usage.",
  );
}
