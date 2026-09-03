'use client';

import { Search } from 'lucide-react';
import { useSearchContext } from 'fumadocs-ui/contexts/search';
import { ThemeSwitch } from 'fumadocs-ui/layouts/shared/slots/theme-switch';

export function SiteControls() {
  const { setOpenSearch } = useSearchContext();
  return <div className="site-controls"><button type="button" className="search-button" onClick={() => setOpenSearch(true)} aria-label="Search the program"><Search size={17} aria-hidden="true" /><span>Search</span><kbd>⌘ K</kbd></button><ThemeSwitch mode="light-dark" /></div>;
}
