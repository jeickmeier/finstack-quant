import type { Metadata } from 'next';
import { Providers } from '@/components/providers';
import './globals.css';

export const metadata: Metadata = {
  title: { default: 'The Analyst Program · Finstack Quant', template: '%s · Finstack Quant' },
  description: 'A working analyst’s education in markets, pricing, risk, and credit. Read the standard, build the calculation, and verify the result.',
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return <html lang="en" suppressHydrationWarning><body><a className="skip-link" href="#main-content">Skip to content</a><Providers>{children}</Providers></body></html>;
}
