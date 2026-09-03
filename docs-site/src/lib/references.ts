import { readFileSync } from 'node:fs';
import path from 'node:path';
import { cache } from 'react';

const referenceTitles = cache(() => {
  const markdown = readFileSync(path.join(process.cwd(), '..', 'docs', 'REFERENCES.md'), 'utf8');
  return new Map(Array.from(markdown.matchAll(/<a id="([^"]+)"><\/a>\s*\n#+\s+([^\n]+)/g), (match) => [match[1], match[2]]));
});

export function referenceLink(reference: string) {
  const anchor = reference.split('#').at(-1) ?? reference;
  return { href: `/references.html#${anchor}`, title: referenceTitles().get(anchor) ?? anchor.replaceAll('-', ' ') };
}
