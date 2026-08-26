import type { Metadata, Viewport } from 'next';

import { AppShell } from '@/components/app-shell';

import './globals.css';

export const metadata: Metadata = {
  title: 'Hoi4 Modding Supporter',
  description: 'Browse, edit and organise a Hearts of Iron IV mod project.',
};

export const viewport: Viewport = {
  width: 'device-width',
  initialScale: 1,
  maximumScale: 1,
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    // The shell picks the real theme on mount; dark is the default so the
    // window never flashes white on start-up.
    <html lang='en' className='dark' suppressHydrationWarning>
      <body>
        <AppShell>{children}</AppShell>
      </body>
    </html>
  );
}
