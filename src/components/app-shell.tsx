'use client';

import * as React from 'react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import {
  AlertCircle,
  FolderTree,
  Home,
  Images,
  Languages,
  Library,
  RefreshCw,
  Settings as SettingsIcon,
  X,
} from 'lucide-react';

import { Toaster } from 'sonner';

import { Button } from '@/components/ui/button';
import { useAppStore } from '@/lib/store';
import { cn } from '@/lib/utils';

interface NavItem {
  href: string;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  /** Workspace sections are unreachable until a mod is open. */
  needsProject?: boolean;
}

const NAV_ITEMS: NavItem[] = [
  { href: '/', label: 'Home', icon: Home },
  { href: '/workspace/scripts', label: 'Scripts', icon: Library, needsProject: true },
  { href: '/workspace/focus', label: 'Focus Trees', icon: FolderTree, needsProject: true },
  { href: '/workspace/localisation', label: 'Localisation', icon: Languages, needsProject: true },
  { href: '/workspace/assets', label: 'Assets', icon: Images, needsProject: true },
];

/** Keeps the document theme in step with the saved setting. */
function useThemeClass(theme: string | undefined) {
  React.useEffect(() => {
    const mode = theme ?? 'system';
    const query = window.matchMedia('(prefers-color-scheme: dark)');

    const apply = () => {
      const dark = mode === 'dark' || (mode === 'system' && query.matches);
      document.documentElement.classList.toggle('dark', dark);
    };

    apply();
    if (mode !== 'system') return;

    query.addEventListener('change', apply);
    return () => query.removeEventListener('change', apply);
  }, [theme]);
}

export function AppShell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const { settings, project, scan, isScanning, error, initialise, refreshScan, closeProject, setError } = useAppStore();

  useThemeClass(settings?.theme);

  React.useEffect(() => {
    void initialise();
  }, [initialise]);

  return (
    <div className='flex h-screen w-screen overflow-hidden bg-background'>
      <nav className='flex w-52 shrink-0 flex-col border-r border-border bg-surface-sunken'>
        <div className='flex h-12 items-center gap-2 px-4'>
          <AppMark />
          <span className='text-sm font-semibold tracking-tight'>Hoi4 Supporter</span>
        </div>

        <div className='flex flex-1 flex-col gap-0.5 p-2'>
          {NAV_ITEMS.map((item) => (
            <NavLink
              key={item.href}
              item={item}
              active={item.href === '/' ? pathname === '/' : pathname.startsWith(item.href)}
              disabled={Boolean(item.needsProject) && !project}
            />
          ))}
        </div>

        <div className='p-2'>
          <NavLink
            item={{ href: '/settings', label: 'Settings', icon: SettingsIcon }}
            active={pathname.startsWith('/settings')}
          />
        </div>
      </nav>

      <div className='flex min-w-0 flex-1 flex-col'>
        <header className='flex h-12 shrink-0 items-center gap-3 border-b border-border bg-surface px-4'>
          {project ? (
            <>
              <div className='flex min-w-0 flex-col'>
                <span className='truncate text-sm font-medium'>{project.name}</span>
                <span className='truncate text-xs text-muted'>{project.folderPath}</span>
              </div>
              <div className='ml-auto flex items-center gap-1.5'>
                <Button
                  variant='ghost'
                  size='icon-sm'
                  onClick={() => void refreshScan()}
                  disabled={isScanning}
                  title='Rescan project files'
                >
                  <RefreshCw className={cn(isScanning && 'animate-spin')} />
                </Button>
                <Button variant='ghost' size='icon-sm' onClick={closeProject} title='Close project'>
                  <X />
                </Button>
              </div>
            </>
          ) : (
            <span className='text-sm text-muted'>No mod project open</span>
          )}
        </header>

        <main className='min-h-0 flex-1 overflow-hidden'>{children}</main>

        <footer className='flex h-7 shrink-0 items-center gap-3 border-t border-border bg-surface-sunken px-4 text-xs text-muted'>
          {error ? (
            <button
              type='button'
              className='flex items-center gap-1.5 text-danger'
              onClick={() => setError(null)}
              title='Dismiss'
            >
              <AlertCircle className='size-3.5' />
              <span className='max-w-[70vw] truncate'>{error}</span>
            </button>
          ) : (
            <span>
              {isScanning
                ? 'Scanning project files…'
                : scan
                  ? `${scan.files.length.toLocaleString()} files${scan.truncated ? ' (limit reached)' : ''}`
                  : 'Ready'}
            </span>
          )}
          {settings?.gameRootPath ? (
            <span className='ml-auto truncate'>Game: {settings.gameRootPath}</span>
          ) : (
            <span className='ml-auto'>Game folder not set</span>
          )}
        </footer>
      </div>

      <Toaster
        position='bottom-right'
        offset={36}
        toastOptions={{
          style: {
            background: 'var(--surface-raised)',
            border: '1px solid var(--border)',
            color: 'var(--foreground)',
          },
        }}
      />
    </div>
  );
}

function NavLink({ item, active, disabled }: { item: NavItem; active: boolean; disabled?: boolean }) {
  const Icon = item.icon;
  const className = cn(
    'flex items-center gap-2.5 rounded-md px-2.5 py-2 text-sm transition-colors',
    active ? 'bg-accent-soft font-medium text-foreground' : 'text-muted hover:bg-surface-raised hover:text-foreground',
    disabled && 'pointer-events-none opacity-40',
  );

  if (disabled) {
    return (
      <span className={className} aria-disabled>
        <Icon className='size-4' />
        {item.label}
      </span>
    );
  }

  return (
    <Link href={item.href} className={className}>
      <Icon className='size-4' />
      {item.label}
    </Link>
  );
}

/** The focus-tree mark from the application icon. */
function AppMark() {
  return (
    <svg viewBox='0 0 24 24' className='size-5' aria-hidden>
      <circle cx='12' cy='5.5' r='3' fill='var(--accent)' />
      <path
        d='M12 8.5v3.25M6.5 15v-3.25h11V15'
        stroke='var(--accent)'
        strokeWidth='1.5'
        fill='none'
        strokeLinecap='round'
      />
      <circle cx='6.5' cy='18' r='2.6' stroke='var(--accent)' strokeWidth='1.5' fill='none' />
      <circle cx='17.5' cy='18' r='2.6' stroke='var(--accent)' strokeWidth='1.5' fill='none' />
    </svg>
  );
}
