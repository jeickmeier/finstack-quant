import { createSearchAPI } from 'fumadocs-core/search/server';
import { docsSource, labSource, lessonSource } from '@/lib/source';

export const dynamic = 'force-static';
export const revalidate = false;

export const { staticGET: GET } = createSearchAPI('advanced', {
  indexes: [...lessonSource.getPages(), ...docsSource.getPages(), ...labSource.getPages()].map((page) => ({
    id: page.url,
    title: page.data.title,
    description: page.data.description,
    url: page.url,
    structuredData: page.data.structuredData,
  })),
});
