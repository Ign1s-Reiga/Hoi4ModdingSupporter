import type { NextConfig } from 'next';

// Tauri serves the frontend as static files from `out/`, so the app is
// exported statically. Every route must therefore be resolvable at build time.
const nextConfig: NextConfig = {
  output: 'export',
  images: { unoptimized: true },
  // Trailing slashes keep the exported directory layout resolvable by the
  // Tauri asset protocol (`/settings/` -> `settings/index.html`).
  trailingSlash: true,
};

export default nextConfig;
