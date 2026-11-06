/** Encode a string literal for generated JS/TS, including script-tag contexts. */
export function sourceString(value) {
  if (typeof value !== "string")
    throw new TypeError("Source string literals require a string");
  return JSON.stringify(value).replace(
    /[<>\u2028\u2029]/g,
    (character) =>
      `\\u${character.charCodeAt(0).toString(16).padStart(4, "0")}`,
  );
}
