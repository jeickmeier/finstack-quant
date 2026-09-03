import Link from 'next/link';

export default function NotFound() {
  return <main id="main-content" className="not-found"><p className="section-eyebrow">404 · Page not found</p><h1>This page is not in the program.</h1><p>Return to the curriculum to find the lesson or companion lab you need.</p><Link className="primary-link" href="/">Open the curriculum →</Link></main>;
}
