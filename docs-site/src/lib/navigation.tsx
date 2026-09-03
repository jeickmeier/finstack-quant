import type * as PageTree from 'fumadocs-core/page-tree';
import { getLessons, groups } from './curriculum';

export function getNavigation(): PageTree.Root {
  const lessons = getLessons();
  return {
    name: 'Analyst program',
    children: [
      { type: 'page', name: 'Program overview', url: '/' },
      { type: 'page', name: 'Set up your workspace', url: '/docs/setup' },
      ...groups.map((group): PageTree.Folder => ({
        type: 'folder',
        name: `${group.label} · ${group.title}`,
        defaultOpen: false,
        children: lessons.filter((lesson) => lesson.part === group.id).map((lesson) => ({
          type: 'page',
          name: <span className="nav-lesson"><span>{lesson.id === 'capstone' ? '↗' : lesson.id}</span><span>{lesson.title}</span></span>,
          url: `/learn/${lesson.id}`,
        })),
      })),
      { type: 'page', name: 'Companion labs', url: '/labs' },
    ],
  };
}
