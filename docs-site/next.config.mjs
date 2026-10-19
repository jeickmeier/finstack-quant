import { fileURLToPath } from 'node:url';
import { createMDX } from 'fumadocs-mdx/next';

const withMDX = createMDX();

/** @type {import('next').NextConfig} */
export const config = {
  output: 'export',
  turbopack: { root: fileURLToPath(new URL('..', import.meta.url)) },
  basePath: process.env.NEXT_PUBLIC_BASE_PATH || '',
  trailingSlash: true,
  images: { unoptimized: true },
  reactStrictMode: true,
  // `next dev` otherwise writes AGENTS.md and CLAUDE.md into docs-site/.
  agentRules: false,
};

export default withMDX(config);
