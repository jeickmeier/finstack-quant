import Link from 'next/link';
import { ArrowDown, ArrowRight, ArrowUpRight, BookOpen, FlaskConical } from 'lucide-react';
import { BrandLink } from '@/components/brand';
import { SiteControls } from '@/components/site-controls';
import { getLessons, groups } from '@/lib/curriculum';

export default function OverviewPage() {
  const lessons = getLessons();
  const published = lessons.filter((lesson) => lesson.status === 'published').length;
  const labCount = new Set(lessons.flatMap((lesson) => lesson.labs)).size;

  return <div className="program-shell">
    <header className="program-header"><BrandLink /><nav aria-label="Main navigation"><a href="#curriculum">Curriculum</a><Link href="/labs">Labs</Link><Link href="/docs/setup">Setup</Link></nav><SiteControls /></header>
    <main id="main-content">
      <section className="program-hero" aria-labelledby="program-title">
        <div className="hero-copy"><p className="section-eyebrow"><span className="eyebrow-rule" /> Finstack Quant · Learning</p><h1 id="program-title">The analyst<br /><em>program.</em></h1><p className="hero-description">Understand the market.<br />Build the calculation. Defend the result.</p><p className="hero-detail">A practical education in pricing, risk, and credit—built around one evolving portfolio and the questions an investment desk actually asks.</p><div className="hero-actions"><Link className="primary-link" href="/learn/1.1">Start with the foundations <ArrowRight size={18} aria-hidden="true" /></Link><Link className="text-link" href="/docs/setup">Set up your workspace <ArrowUpRight size={15} aria-hidden="true" /></Link></div></div>
        <aside className="book-journey" aria-label="The analyst book workflow"><div className="journey-label"><span>The analyst book</span><span>01 → 33</span></div><div className="journey-statement">One portfolio.<br />A complete perspective.</div><ol>{[
          ['01', 'Price the cashflows', 'Conventions · curves · instruments'],
          ['02', 'Understand the exposure', 'Performance · risk · scenarios'],
          ['03', 'Make the investment case', 'Fundamentals · credit · construction'],
          ['04', 'Explain the decision', 'Attribution · reporting · capstone'],
        ].map(([number, title, detail]) => <li key={number}><span className="journey-number">{number}</span><div><strong>{title}</strong><span>{detail}</span></div></li>)}</ol><div className="journey-foot"><span>Read. Run. Reconcile.</span><BookOpen size={18} aria-hidden="true" /></div></aside>
      </section>
      <div className="program-facts" aria-label="Program at a glance"><div><strong>{lessons.length}</strong><span>learning units</span></div><div><strong>02</strong><span>specialist tracks</span></div><div><strong>{String(labCount).padStart(2, '0')}</strong><span>companion labs</span></div><div className="publication-fact"><span className="publication-dot" /><span><strong>{published} of {lessons.length} published</strong><small>Drafts are labeled throughout</small></span></div></div>
      <section id="curriculum" className="curriculum-section" aria-labelledby="curriculum-title">
        <div className="curriculum-heading"><div><p className="section-eyebrow">The curriculum</p><h2 id="curriculum-title">Your course of work.</h2></div><p>Follow the common core, then deepen your practice in credit or volatility. Every lesson connects the market standard to a working calculation.</p></div>
        <div className="curriculum-layout"><nav className="curriculum-rail" aria-label="Jump to a curriculum part"><span className="section-eyebrow">In this program</span>{groups.map((group) => <a key={group.id} href={`#part-${group.id}`}><span>{group.label}</span><ArrowDown size={13} aria-hidden="true" /></a>)}<Link className="rail-labs" href="/labs"><FlaskConical size={16} aria-hidden="true" />Browse the labs</Link></nav><div className="curriculum-groups">{groups.map((group) => {
          const items = lessons.filter((lesson) => lesson.part === group.id);
          return <section className="curriculum-group" id={`part-${group.id}`} key={group.id}><div className="group-heading"><p className="section-eyebrow">{group.label}<span>{items.length} {items.length === 1 ? 'unit' : 'units'}</span></p><h3>{group.title}</h3><p>{group.description}</p></div><ol className="lesson-list">{items.map((lesson) => <li key={lesson.id}><Link href={`/learn/${lesson.id}`}><span className="lesson-number">{lesson.id === 'capstone' ? '↗' : lesson.id}</span><span className="lesson-name">{lesson.title}</span><span className={`status status-${lesson.status}`}>{lesson.status}</span><ArrowUpRight className="lesson-arrow" size={17} aria-hidden="true" /></Link></li>)}</ol></section>;
        })}</div></div>
      </section>
      <section className="practice-note"><p className="section-eyebrow">A working education</p><div><h2>The output is only<br />part of the answer.</h2><p>Dates, units, market conventions, and model assumptions belong beside every number. Run the labs, complete the exercises, and learn to explain what changes the result—and what does not.</p><Link href="/docs/setup">Prepare your workspace <ArrowRight size={17} aria-hidden="true" /></Link></div></section>
    </main>
    <footer className="program-footer"><BrandLink /><p>Markets, made explicit.</p><Link href="#curriculum">Back to curriculum ↑</Link></footer>
  </div>;
}
