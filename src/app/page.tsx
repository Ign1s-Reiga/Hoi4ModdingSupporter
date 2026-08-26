'use client';

import * as React from 'react';
import { useRouter } from 'next/navigation';
import { open } from '@tauri-apps/plugin-dialog';
import { FolderOpen, Loader2, Trash2 } from 'lucide-react';

import { ModThumbnail } from '@/components/mod-thumbnail';
import { Button } from '@/components/ui/button';
import { Badge, EmptyState } from '@/components/ui/panel';
import { describeError } from '@/lib/ipc';
import { useAppStore } from '@/lib/store';
import { useIsDesktop } from '@/lib/use-desktop';
import { formatTimestamp } from '@/lib/utils';

export default function HomePage() {
  const router = useRouter();
  const { settings, isRestoring, openProject, forgetRecent, setError } = useAppStore();
  const [isOpening, setIsOpening] = React.useState(false);
  const isDesktopWindow = useIsDesktop();

  const recents = settings?.recentProjects ?? [];

  async function pickDescriptor() {
    setIsOpening(true);
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        title: 'Select a mod descriptor',
        filters: [{ name: 'Mod descriptor', extensions: ['mod'] }],
      });

      if (typeof selected === 'string') {
        await load(selected);
      }
    } catch (error) {
      setError(describeError(error));
    } finally {
      setIsOpening(false);
    }
  }

  async function load(modFilePath: string) {
    const project = await openProject(modFilePath);
    if (project) router.push('/workspace/scripts');
  }

  if (!isDesktopWindow) {
    return (
      <EmptyState
        title='Desktop window required'
        description='This is the Tauri frontend. Start the app with `pnpm desktop` so the backend commands are available.'
      />
    );
  }

  return (
    <div className='h-full overflow-auto'>
      <div className='mx-auto flex max-w-4xl flex-col gap-8 px-8 py-10'>
        <header className='space-y-2'>
          <h1 className='text-2xl font-semibold tracking-tight'>Hoi4 Modding Supporter</h1>
          <p className='max-w-xl text-sm leading-relaxed text-muted'>
            Open a mod descriptor to browse its files, edit national focus trees and keep localisation tidy. Everything
            is written straight back to the mod folder.
          </p>
        </header>

        <div className='flex flex-wrap items-center gap-3'>
          <Button variant='primary' size='lg' onClick={() => void pickDescriptor()} disabled={isOpening}>
            {isOpening ? <Loader2 className='animate-spin' /> : <FolderOpen />}
            Open mod descriptor
          </Button>
          {!settings?.gameRootPath ? (
            <Button variant='link' onClick={() => router.push('/settings')}>
              Set the Hearts of Iron IV folder to browse game assets
            </Button>
          ) : null}
        </div>

        <section className='space-y-3'>
          <div className='flex items-baseline justify-between'>
            <h2 className='text-sm font-medium'>Recent projects</h2>
            {recents.length > 0 ? <span className='text-xs text-muted'>{recents.length} saved</span> : null}
          </div>

          {isRestoring ? (
            <div className='flex items-center gap-2 py-8 text-sm text-muted'>
              <Loader2 className='size-4 animate-spin' />
              Loading settings…
            </div>
          ) : recents.length === 0 ? (
            <div className='rounded-lg border border-dashed border-border'>
              <EmptyState
                icon={<FolderOpen />}
                title='No projects yet'
                description='Pick the .mod file that sits next to your mod folder — usually in Documents/Paradox Interactive/Hearts of Iron IV/mod.'
              />
            </div>
          ) : (
            <ul className='grid gap-3 sm:grid-cols-2'>
              {recents.map((recent) => (
                <li key={recent.folderPath}>
                  <div className='group relative flex overflow-hidden rounded-lg border border-border bg-surface transition-colors hover:border-border-strong'>
                    <button
                      type='button'
                      className='flex min-w-0 flex-1 items-stretch gap-3 text-left'
                      onClick={() => void load(recent.modFilePath)}
                    >
                      <ModThumbnail
                        path={recent.imagePath}
                        name={recent.displayName}
                        className='h-full w-24 shrink-0'
                      />
                      <div className='flex min-w-0 flex-col justify-center gap-1 py-3 pr-3'>
                        <span className='truncate text-sm font-medium'>{recent.displayName}</span>
                        <span className='truncate text-xs text-muted' title={recent.folderPath}>
                          {recent.folderPath}
                        </span>
                        <Badge tone='outline' className='w-fit'>
                          {formatTimestamp(recent.lastOpened)}
                        </Badge>
                      </div>
                    </button>
                    <Button
                      variant='ghost'
                      size='icon-sm'
                      className='absolute right-2 top-2 opacity-0 transition-opacity group-hover:opacity-100'
                      title='Remove from recents'
                      onClick={() => void forgetRecent(recent.folderPath)}
                    >
                      <Trash2 />
                    </Button>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
    </div>
  );
}
