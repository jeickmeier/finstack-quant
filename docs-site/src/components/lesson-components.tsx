import type { ReactNode } from 'react';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { cache } from 'react';
import Link from 'next/link';
import { ArrowUpRight, Check, ChevronDown, FlaskConical, Terminal } from 'lucide-react';
import { labSource } from '@/lib/source';

type SectionProps = { children: ReactNode; title?: string };

export function DeskContext({ children, title = 'The desk question', question }: SectionProps & { question?: string }) {
  return <aside className="desk-context" id="desk-context"><p className="section-eyebrow">{title}</p>{question && <p className="desk-question">{question}</p>}<div>{children}</div></aside>;
}

export function Standard({ children, title = 'The market standard' }: SectionProps) {
  return <section className="lesson-section" id="market-standard"><h2>{title}</h2>{children}</section>;
}

export function BuildIt({ children, title = 'Build it' }: SectionProps) {
  return <section className="lesson-section" id="build-it"><h2>{title}</h2>{children}</section>;
}

export function LabCard({ title = 'Open the companion lab', href, description, children, notebook }: {
  title?: string; href?: string; description?: string; children?: ReactNode; notebook?: string;
}) {
  const url = href ?? (notebook ? `/labs/${notebook.replace(/\.ipynb$/, '')}` : '/labs');
  if (url.startsWith('/labs/') && !labSource.getPage(url.replace(/^\/labs\//, '').split('/'))) {
    return <div className="lab-card lab-pending"><FlaskConical size={20} aria-hidden="true" /><span><span className="section-eyebrow">Companion notebook · awaiting validation</span><strong>{title}</strong>{description && <span>{description}</span>}{children}</span></div>;
  }
  return <Link href={url} className="lab-card"><FlaskConical size={20} aria-hidden="true" /><span><span className="section-eyebrow">Companion notebook</span><strong>{title}</strong>{description && <span>{description}</span>}{children}</span><ArrowUpRight size={20} aria-hidden="true" /></Link>;
}

export function Exercise({ title, question, solution, children, id }: {
  title?: string; question?: ReactNode; solution?: ReactNode; children?: ReactNode; id?: string;
}) {
  return <section className="exercise" id={id}>
    <p className="section-eyebrow">Required exercise</p>
    {title && <h3>{title}</h3>}{question && <div>{question}</div>}
    <details><summary><span>Reveal the complete solution</span><ChevronDown size={16} aria-hidden="true" /></summary><div className="exercise-solution">{solution ?? children}</div></details>
  </section>;
}

export function CheckYourself({ children, title = 'Check your understanding' }: SectionProps) {
  return <section className="check-yourself" id="check-yourself"><div className="check-heading"><Check size={18} aria-hidden="true" /><h2>{title}</h2></div>{children}</section>;
}

export function AvailabilityBadge({ children, status, label }: { children?: ReactNode; status?: string; label?: string }) {
  return <span className="availability-badge">{children ?? label ?? status ?? 'Python'}</span>;
}

type SnippetArtifact = { status: string; blocks: { id: string; role: string; stdout: string; assets: string[]; duration_seconds: number }[] };
const readArtifact = cache((lesson: string): SnippetArtifact | null => {
  if (!/^(?:\d\.\d|[CV]\d|capstone)$/.test(lesson)) throw new Error(`Invalid lesson id: ${lesson}`);
  try {
    return JSON.parse(readFileSync(path.join(process.cwd(), '.build', 'snippets', `${lesson}.json`), 'utf8')) as SnippetArtifact;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return null;
    throw error;
  }
});

export function ExecutedOutput({ lesson, block }: { lesson: string; block: string }) {
  const artifact = readArtifact(lesson);
  const result = artifact?.status === 'passed' ? artifact.blocks.find((item) => item.id === block) : undefined;
  return <figure className="executed-output">
    <figcaption><span><Terminal size={14} aria-hidden="true" />Captured output</span><code>{block}</code></figcaption>
    {result ? <>
      {result.stdout ? <pre><code>{result.stdout}</code></pre> : <p className="output-note">Assertions passed. This block produced no printed output.</p>}
      {result.assets.map((asset) => {
        if (asset.includes('..') || /^[a-z]+:/i.test(asset)) throw new Error(`Unsafe snippet asset: ${asset}`);
        const url = asset.startsWith('/') ? asset : `/${asset}`;
        if (/\.html$/i.test(asset)) return <div className="output-report" key={asset}><iframe src={url} title={`Calculation report from ${block}`} sandbox="allow-scripts" loading="lazy" /><a href={url} target="_blank" rel="noreferrer">Open report full width <ArrowUpRight size={14} aria-hidden="true" /></a></div>;
        return /\.(png|jpg|jpeg|svg|webp)$/i.test(asset)
          ? <a className="output-image" href={url} key={asset}><img src={url} alt={`Calculation figure from ${block}`} loading="lazy" /></a>
          : <a href={url} key={asset} className="output-download">Download {asset.split('/').at(-1)}</a>;
      })}
    </> : <p className="output-note">No verified output has been captured for this block yet.</p>}
  </figure>;
}

export function LabHtml({ src, title = 'Rendered companion notebook' }: { src: string; title?: string }) {
  if (!src.startsWith('/lab-assets/') || src.includes('..')) throw new Error('Notebook HTML must be a local lab asset.');
  return <div className="lab-frame"><p><span>Notebook preview</span><a href={src} target="_blank" rel="noreferrer">Open full width <ArrowUpRight size={14} aria-hidden="true" /></a></p><iframe src={src} title={title} sandbox="allow-scripts" loading="lazy" /></div>;
}
