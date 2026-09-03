'use client';

import { useDocsSearch } from 'fumadocs-core/search/client';
import { staticClient } from 'fumadocs-core/search/client/orama-static';
import { SearchDialog, SearchDialogClose, SearchDialogContent, SearchDialogFooter, SearchDialogHeader, SearchDialogIcon, SearchDialogInput, SearchDialogList, SearchDialogOverlay } from 'fumadocs-ui/components/dialog/search';
import type { SharedProps } from 'fumadocs-ui/contexts/search';

export default function ProgramSearch(props: SharedProps) {
  const { search, setSearch, query } = useDocsSearch({ client: staticClient({ from: '/api/search' }) });
  return <SearchDialog search={search} onSearchChange={setSearch} isLoading={query.isLoading} {...props}>
    <SearchDialogOverlay />
    <SearchDialogContent><SearchDialogHeader><SearchDialogIcon /><SearchDialogInput placeholder="Search lessons, concepts, and labs…" /><SearchDialogClose /></SearchDialogHeader>
      {query.error ? <p className="search-error">The search index could not be loaded. Please reload the page and try again.</p> : <SearchDialogList items={query.data !== 'empty' ? query.data : null} />}
      <SearchDialogFooter><span>Search this edition of the analyst program</span></SearchDialogFooter>
    </SearchDialogContent>
  </SearchDialog>;
}
