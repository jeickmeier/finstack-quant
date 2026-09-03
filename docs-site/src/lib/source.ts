import { docs, labs, lessons } from 'collections/server';
import { loader } from 'fumadocs-core/source';

export const lessonSource = loader({
  baseUrl: '/learn',
  source: lessons.toFumadocsSource(),
  slugs: (file) => [(file.data as { id: string }).id],
});

export const docsSource = loader({ baseUrl: '/docs', source: docs.toFumadocsSource() });
export const labSource = loader({ baseUrl: '/labs', source: labs.toFumadocsSource() });
