import { DocsLayout } from 'fumadocs-ui/layouts/docs';
import { Brand } from '@/components/brand';
import { getNavigation } from '@/lib/navigation';

export default function ReaderLayout({ children }: { children: React.ReactNode }) {
  return <DocsLayout tree={getNavigation()} nav={{ title: <Brand />, url: '/' }} links={[{ text: 'Program', url: '/' }, { text: 'Labs', url: '/labs' }]} sidebar={{ defaultOpenLevel: 0 }}>{children}</DocsLayout>;
}
