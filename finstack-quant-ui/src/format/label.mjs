/**
 * Words that stay upper case: market, model and convention acronyms, the
 * acronyms in Rust metric ids (`ytw`, `irr`, `was` for weighted average
 * spread) and the `01` sensitivity names. `var` is left out on purpose because
 * schema keys use it for variance (`realized_var_method`, `var_config`), and
 * `cs` because they use it for capital structure (`cs_ref`).
 */
const acronyms = new Set([
  "abf",
  "abs",
  "asw",
  "atm",
  "bf",
  "cdr",
  "cds",
  "ce",
  "clo",
  "cmbs",
  "cmo",
  "cms",
  "cpr",
  "cs01",
  "df",
  "dpi",
  "dscr",
  "dv01",
  "fra",
  "fx",
  "fx01",
  "gp",
  "hvar",
  "hw",
  "hw1f",
  "imm",
  "ir",
  "irr",
  "irs",
  "isda",
  "libor",
  "lp",
  "mbs",
  "mc",
  "moic",
  "nav",
  "nav01",
  "ndf",
  "npv",
  "oas",
  "ois",
  "pde",
  "pv",
  "pv01",
  "rr",
  "sabr",
  "sofr",
  "tarn",
  "tba",
  "trs",
  "tvpi",
  "vol",
  "wal",
  "wam",
  "warf",
  "was",
  "xccy",
  "xirr",
  "yf",
  "yoy",
  "ytm",
  "ytw",
]);

/**
 * Sentence-case display label for a Rust wire, metric or registry name.
 * Typography only: it splits on `-` and `_`, capitalises the first word and
 * upper-cases recognised acronyms. It adds no financial names: schema field,
 * metric and registry names render through it, and components add no curated
 * label maps for those names (README "Validation boundary").
 * @param {string} name - Kebab- or snake-case name such as `bucketed_dv01`.
 * @returns {string} Space-separated label such as `Bucketed DV01`.
 */
export function labelFor(name) {
  return name
    .split(/[-_]/)
    .map((word, index) =>
      acronyms.has(word)
        ? word.toUpperCase()
        : index === 0
          ? word.replace(/^./, (letter) => letter.toUpperCase())
          : word,
    )
    .join(" ");
}
