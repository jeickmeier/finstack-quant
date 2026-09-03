import defaultComponents from 'fumadocs-ui/mdx';
import type { MDXComponents } from 'mdx/types';
import * as lessonComponents from './lesson-components';

export function getMDXComponents(components: MDXComponents = {}): MDXComponents {
  return { ...defaultComponents, ...lessonComponents, ...components };
}
