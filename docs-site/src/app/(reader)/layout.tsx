import { DocsLayout } from 'fumadocs-ui/layouts/docs';
import { Brand } from '@/components/brand';
import { getNavigation } from '@/lib/navigation';

export default function ReaderLayout({ children }: { children: React.ReactNode }) {
  const registryOnly = process.env.REGISTRY_ONLY === '1';
  const navigation = getNavigation();
  const tree = registryOnly ? { ...navigation, name: 'Component registry', children: navigation.children.filter((entry) => entry.type === 'folder' && entry.name === 'Component registry') } : navigation;
  return <DocsLayout tree={tree} nav={{ title: <Brand />, url: registryOnly ? '/docs/registry/workbench' : '/' }} links={registryOnly ? [] : [{ text: 'Program', url: '/' }, { text: 'Labs', url: '/labs' }]} sidebar={{ defaultOpenLevel: 0 }}>{children}</DocsLayout>;
}
