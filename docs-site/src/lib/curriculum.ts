import { readFileSync } from 'node:fs';
import path from 'node:path';
import { cache } from 'react';

export type Lesson = {
  id: string;
  part: string;
  title: string;
  path: string;
  requires: string[];
  fixtures: string[];
  labs: string[];
  examples: string[];
  introduces?: string[];
  status: 'draft' | 'published';
  desk_question: string;
  references: string[];
  api: string[];
  variants?: Record<string, string[]>;
  variant_labs?: Record<string, string[]>;
};

export const groups = [
  { id: '1', label: 'Part I', title: 'The language of markets', description: 'Money, dates, rates, curves, and the conventions behind every calculation.' },
  { id: '2', label: 'Part II', title: 'Pricing and risk', description: 'From a bond’s cashflows to derivatives, simulation, and structured products.' },
  { id: '3', label: 'Part III', title: 'From trades to portfolios', description: 'Measure performance, explain P&L, stress the book, and understand margin.' },
  { id: '4', label: 'Part IV', title: 'The analyst’s craft', description: 'Connect company fundamentals, credit decisions, portfolio construction, and reporting.' },
  { id: 'C', label: 'Track C', title: 'Credit', description: 'Complex bonds, lending facilities, securitization, index credit, and converts.' },
  { id: 'V', label: 'Track V', title: 'Volatility & futures', description: 'Listed hedges, variance and VIX, and commodity market structure.' },
  { id: 'capstone', label: 'Capstone', title: 'Bring the book together', description: 'A complete investment workflow, with separate credit and volatility variants.' },
] as const;

export const getLessons = cache((): Lesson[] => {
  const file = path.join(process.cwd(), '.build', 'curriculum.json');
  const result = JSON.parse(readFileSync(file, 'utf8')) as { lessons: Lesson[] };
  return result.lessons;
});

export function getLesson(id: string) {
  return getLessons().find((lesson) => lesson.id === id);
}

export function getLessonLabs(lesson: Lesson) {
  return [...lesson.labs, ...Object.values(lesson.variant_labs ?? {}).flat()];
}

export function getProgression(lesson: Lesson) {
  const lessons = getLessons();
  const route = ['C', 'V'].includes(lesson.part)
    ? [...lessons.filter((item) => ['1', '2', '3', '4', lesson.part, 'capstone'].includes(item.part))]
    : lessons.filter((item) => ['1', '2', '3', '4'].includes(item.part));
  const index = route.findIndex((item) => item.id === lesson.id);
  return { previous: route[index - 1], next: route[index + 1] };
}

export function labHref(notebook: string) {
  return `/labs/${notebook.replace(/\.ipynb$/, '')}`;
}
