import { notFound } from 'next/navigation';
import { DocsBody, DocsDescription, DocsPage, DocsTitle } from 'fumadocs-ui/layouts/docs/page';
import { getMDXComponents } from '@/components/mdx';
import { labSource } from '@/lib/source';

export function generateStaticParams() { return labSource.generateParams(); }
export const dynamicParams = false;

export async function generateMetadata({ params }: { params: Promise<{ slug: string[] }> }) {
  const page = labSource.getPage((await params).slug);
  return { title: page?.data.title, description: page?.data.description };
}

export default async function LabPage({ params }: { params: Promise<{ slug: string[] }> }) {
  const page = labSource.getPage((await params).slug);
  if (!page) notFound();
  const Body = page.data.body;
  return <DocsPage toc={page.data.toc} full footer={{ enabled: false }}><div id="main-content"><p className="section-eyebrow">Companion notebook</p><DocsTitle>{page.data.title}</DocsTitle><DocsDescription>{page.data.description}</DocsDescription></div><DocsBody><Body components={getMDXComponents()} /></DocsBody></DocsPage>;
}
