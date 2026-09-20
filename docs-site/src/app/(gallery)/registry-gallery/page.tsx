import { Suspense } from "react";
import { RegistryGallery } from "@/components/registry-gallery/gallery";
export const metadata = { title: "Component registry gallery" };
export default function GalleryPage() {
  return (
    <Suspense fallback={<p>Loading registry gallery…</p>}>
      <RegistryGallery />
    </Suspense>
  );
}
