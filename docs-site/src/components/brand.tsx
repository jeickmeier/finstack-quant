import Link from 'next/link';

export function Brand() {
  return <span className="brand"><span className="brand-mark" aria-hidden="true">f<span>q</span></span><span>finstack<span className="brand-quant"> / quant</span></span></span>;
}

export function BrandLink() {
  return <Link href="/" aria-label="Finstack Quant analyst program"><Brand /></Link>;
}
