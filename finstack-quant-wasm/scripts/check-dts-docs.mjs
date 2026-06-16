import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const dts = readFileSync(join(root, 'index.d.ts'), 'utf8');
const readme = readFileSync(join(root, 'README.md'), 'utf8');

const ownedClasses = [
  'Performance',
  'CreditFactorModel',
  'CreditCalibrator',
  'LevelsAtDate',
  'PeriodDecomposition',
  'FactorCovarianceForecast',
  'Portfolio',
];

const failures = [];

for (const className of ownedClasses) {
  const classPattern = new RegExp(
    `export\\s+declare\\s+class\\s+${className}\\s*\\{[\\s\\S]*?Release the underlying wasm heap allocation\\.[\\s\\S]*?free\\(\\):\\s*void;[\\s\\S]*?\\}`,
    'm'
  );
  if (!classPattern.test(dts)) {
    failures.push(`${className} must document free() in index.d.ts`);
  }

  if (!readme.includes(className)) {
    failures.push(`${className} must be listed in README disposal docs`);
  }
}

if (!dts.includes('WASM ownership: classes with a `free(): void` method own wasm heap memory.')) {
  failures.push('index.d.ts must include the package-level WASM ownership note');
}

if (!readme.includes('## WASM Object Disposal')) {
  failures.push('README.md must include the WASM Object Disposal section');
}

if (failures.length > 0) {
  console.error(failures.map((failure) => `- ${failure}`).join('\n'));
  process.exit(1);
}
