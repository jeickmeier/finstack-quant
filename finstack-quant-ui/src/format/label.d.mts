/**
 * Sentence-case display label for a Rust wire, metric or registry name.
 * Typography only: it splits on `-` and `_`, capitalises the first word and
 * upper-cases recognised acronyms (`bucketed_dv01` reads `Bucketed DV01`). It
 * adds no financial names: schema field, metric and registry names render
 * through it, and components add no curated label maps for those names.
 * @param name - Kebab- or snake-case name.
 * @returns The space-separated label.
 */
export declare function labelFor(name: string): string;
