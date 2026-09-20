import { notFound } from 'next/navigation';
import { DocsBody, DocsDescription, DocsPage, DocsTitle } from 'fumadocs-ui/layouts/docs/page';
import { getMDXComponents } from '@/components/mdx';
import { docsSource } from '@/lib/source';

export function generateStaticParams() { return docsSource.generateParams(); }
export const dynamicParams = false;

export async function generateMetadata({ params }: { params: Promise<{ slug?: string[] }> }) {
  const page = docsSource.getPage((await params).slug);
  return { title: page?.data.title, description: page?.data.description };
}

export default async function GuidePage({ params }: { params: Promise<{ slug?: string[] }> }) {
  const slug = (await params).slug;
  const workbench = slug?.join("/") === "registry/workbench";
  const page = docsSource.getPage(slug);
  if (!page) notFound();
  const Body = page.data.body;
  return <DocsPage full={workbench} tableOfContent={{ enabled: !workbench }} tableOfContentPopover={{ enabled: !workbench }} toc={page.data.toc} footer={{ enabled: false }}><div id="main-content"><p className="section-eyebrow">Workspace guide</p><DocsTitle>{page.data.title}</DocsTitle><DocsDescription>{page.data.description}</DocsDescription></div><DocsBody><Body components={getMDXComponents()} /></DocsBody></DocsPage>;
}
