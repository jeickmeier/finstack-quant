/**
 * Locale-independent UTF-16 code-unit order, identical to the default
 * `Array.prototype.sort` on strings. Generated artifacts must not depend on
 * the host locale, so do not replace this with `localeCompare`.
 *
 * @param {string} a
 * @param {string} b
 * @returns {number}
 */
export function byCodeUnit(a, b) {
  if (a < b) return -1;
  if (a > b) return 1;
  return 0;
}
