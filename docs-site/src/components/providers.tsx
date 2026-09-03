'use client';

import { RootProvider } from 'fumadocs-ui/provider/next';
import ProgramSearch from './search';

export function Providers({ children }: { children: React.ReactNode }) {
  return <RootProvider search={{ SearchDialog: ProgramSearch }}>{children}</RootProvider>;
}
