import { notFound } from 'next/navigation';
import Link from 'next/link';
import { ArrowLeft, ArrowRight, BookOpen, FlaskConical } from 'lucide-react';
import { DocsBody, DocsPage, DocsTitle } from 'fumadocs-ui/layouts/docs/page';
import { getMDXComponents } from '@/components/mdx';
import { getLesson, getLessonLabs, getLessons, getProgression, groups, labHref } from '@/lib/curriculum';
import { labSource, lessonSource } from '@/lib/source';
import { referenceLink } from '@/lib/references';

export function generateStaticParams() { return getLessons().map(({ id }) => ({ id })); }
export const dynamicParams = false;

export async function generateMetadata({ params }: { params: Promise<{ id: string }> }) {
  const lesson = getLesson((await params).id);
  return { title: lesson?.title, description: lesson?.desk_question };
}

export default async function LessonPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const lesson = getLesson(id);
  const page = lessonSource.getPage([id]);
  if (!lesson || !page) notFound();
  const Body = page.data.body;
  const group = groups.find((item) => item.id === lesson.part);
  const groupLessons = getLessons().filter((item) => item.part === lesson.part);
  const position = groupLessons.findIndex((item) => item.id === id) + 1;
  const { previous, next } = getProgression(lesson);
  const lessonLabs = getLessonLabs(lesson);

  return <DocsPage toc={page.data.toc} footer={{ enabled: false }}>
    <div id="main-content" className="lesson-masthead">
      <div className="lesson-kicker"><Link href={`/#part-${lesson.part}`}>{group?.label} · {group?.title}</Link><span className={`status status-${lesson.status}`}>{lesson.status}</span></div>
      <DocsTitle>{lesson.title}</DocsTitle>
      <div className="lesson-progress"><span>Lesson {id === 'capstone' ? 'Capstone' : id}</span><div aria-hidden="true">{groupLessons.map((item) => <span key={item.id} className={item.id === id ? 'current' : ''} />)}</div><span>{position} of {groupLessons.length}</span></div>
      {lesson.status === 'draft' && <p className="draft-note">This lesson is a draft. Its content and calculations are still being verified.</p>}
      {lesson.requires.length > 0 && <div className="prerequisites"><BookOpen size={14} aria-hidden="true" /><span>Before you begin:</span>{lesson.requires.map((requirement) => { const item = getLesson(requirement); return item ? <Link key={requirement} href={`/learn/${item.id}`}>{item.id}</Link> : <span key={requirement}>{requirement}</span>; })}</div>}
      {lesson.variants && <div className="capstone-variants"><p>Complete the common program and the prerequisites for your chosen capstone.</p>{Object.entries(lesson.variants).map(([variant, requirements]) => <div className="prerequisites" key={variant}><span>{variant === 'credit' ? 'Credit capstone' : 'Volatility capstone'}:</span>{requirements.map((requirement) => <Link key={requirement} href={`/learn/${requirement}`}>{requirement}</Link>)}</div>)}</div>}
    </div>
    <DocsBody><Body components={getMDXComponents()} /></DocsBody>
    {lessonLabs.length > 0 && <section className="lesson-labs" aria-label="Companion labs"><h2><FlaskConical size={16} aria-hidden="true" /> Companion labs</h2>{lessonLabs.map((lab) => {
      const title = lab.split('/').at(-1)?.replace(/\.ipynb$/, '').replaceAll('_', ' ');
      return labSource.getPage(lab.replace(/\.ipynb$/, '').split('/'))
        ? <Link key={lab} href={labHref(lab)}>{title}<ArrowRight size={16} aria-hidden="true" /></Link>
        : <p className="pending-lab" key={lab}>{title}<span>Awaiting validation</span></p>;
    })}</section>}
    {lesson.references.length > 0 && <section className="lesson-references" aria-label="Supporting references"><h2>Supporting references</h2><ul>{lesson.references.map((reference) => { const item = referenceLink(reference); return <li key={reference}><a href={item.href}>{item.title}</a></li>; })}</ul></section>}
    {lesson.part === '4' && !next && <div className="track-choice"><h2>Continue into a specialization</h2><Link href="/learn/C1">Credit <ArrowRight size={16} /></Link><Link href="/learn/V1">Volatility & futures <ArrowRight size={16} /></Link></div>}
    <nav className="lesson-pagination" aria-label="Lesson progression">{previous ? <Link href={`/learn/${previous.id}`}><ArrowLeft size={17} aria-hidden="true" /><span><small>Previous · {previous.id}</small>{previous.title}</span></Link> : <span />}{next && <Link href={`/learn/${next.id}`}><span><small>Next · {next.id}</small>{next.title}</span><ArrowRight size={17} aria-hidden="true" /></Link>}</nav>
  </DocsPage>;
}
