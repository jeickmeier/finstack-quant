import { defineConfig, defineDocs, frontmatterSchema } from 'fumadocs-mdx/config';
import remarkMath from 'remark-math';
import rehypeKatex from 'rehype-katex';
import { z } from 'zod';

export const lessons = defineDocs({
  dir: 'content/learn',
  docs: {
    schema: frontmatterSchema.extend({
      id: z.string(),
      desk_question: z.string(),
      references: z.array(z.string()).default([]),
      api: z.array(z.string()).default([]),
      status: z.enum(['draft', 'published']),
    }),
  },
});

export const docs = defineDocs({ dir: 'content/docs' });
export const labs = defineDocs({ dir: 'content/labs' });

export default defineConfig({
  mdxOptions: {
    remarkPlugins: [remarkMath],
    // Render math before Fumadocs sends remaining code blocks to Shiki.
    rehypePlugins: (plugins) => [rehypeKatex, ...plugins],
  },
});
