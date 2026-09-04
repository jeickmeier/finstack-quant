import Link from 'next/link';
import { ArrowUpRight } from 'lucide-react';
import { DocsPage, DocsTitle } from 'fumadocs-ui/layouts/docs/page';
import { getLessonLabs, getLessons, labHref } from '@/lib/curriculum';

export const metadata = { title: 'Companion labs' };

export default function LabsPage() {
  const lessons = getLessons();
  const labs = [...new Set(lessons.flatMap(getLessonLabs))];
  return <DocsPage full footer={{ enabled: false }}><div id="main-content"><p className="section-eyebrow">Read · run · investigate</p><DocsTitle>Companion labs</DocsTitle><p className="page-intro">The working notebooks behind the program. Open a rendered lab, then download it to explore the calculations in your own workspace.</p></div><div className="lab-directory">{labs.map((lab) => <Link href={labHref(lab)} key={lab}><span><strong>{lab.split('/').at(-1)?.replace(/\.ipynb$/, '').replaceAll('_', ' ')}</strong><small>{lessons.filter((lesson) => getLessonLabs(lesson).includes(lab)).map((lesson) => lesson.id).join(' · ')}</small></span><ArrowUpRight size={18} aria-hidden="true" /></Link>)}</div></DocsPage>;
}
