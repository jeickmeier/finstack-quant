# Select highlight readability

Highlighted Select and Combobox options now use the normal text color against
the existing soft accent background. Light text changes from green `#145b50` to
near-black `#0b1311`; dark text changes from green `#bae4d2` to `#e6ede8`.
Only the shared accent-foreground theme tokens and their generated outputs change.
The stock shadcn components remain byte-identical.

Focused review: PASS. The generated screen/print CSS and registry CSS variables
agree with the token source. No state, keyboard behavior, financial logic or
custom component override was added.

Validation:

- 14 existing theme tests passed; theme generation, formatting, contrast and all
  150 registry import closures/stock source integrity checks passed.
- Registry-only production export passed in 27.28 seconds.
- Served browser inspection verified selected and keyboard-focused Select items
  in light mode and the selected item in dark mode. Computed text/background
  colors match the tokens: contrast 16.10:1 light and 9.76:1 dark.
- The highlighted searchable Combobox option also resolves to the corrected
  light text/background pair.
- The user's existing workbench form values were preserved; verification used
  a temporary tab. Existing tabs need a refresh to load the rebuilt stylesheet.

No full-library suite or screenshot-baseline update ran for this token-only fix.
