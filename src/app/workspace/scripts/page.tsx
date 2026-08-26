'use client';

import * as React from 'react';
import { FileText, Loader2, RotateCcw, Save, Search } from 'lucide-react';
import { toast } from 'sonner';

import { ScriptEditor } from '@/components/script-editor';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/form';
import { Badge, EmptyState, ListRow, Panel, PanelBody, PanelHeader } from '@/components/ui/panel';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { confirmDiscard } from '@/lib/dialogs';
import { SCRIPT_GROUPS, filterFiles } from '@/lib/groups';
import { api, describeError } from '@/lib/ipc';
import { useAppStore } from '@/lib/store';
import { useUnsavedIn } from '@/lib/use-unsaved';
import { formatBytes } from '@/lib/utils';
import type { Encoding, ProjectFile } from '@/lib/types';

/** Rendering every row of a total conversion would stall the list. */
const MAX_ROWS = 400;

export default function ScriptsPage() {
  const { scan, isScanning, setError } = useAppStore();

  const [groupId, setGroupId] = React.useState('all');
  const [search, setSearch] = React.useState('');
  const [selected, setSelected] = React.useState<ProjectFile | null>(null);

  const [content, setContent] = React.useState('');
  const [saved, setSaved] = React.useState('');
  const [hasBom, setHasBom] = React.useState(false);
  const [encoding, setEncoding] = React.useState<Encoding>('utf8');
  const [isLoading, setIsLoading] = React.useState(false);
  const [isSaving, setIsSaving] = React.useState(false);
  // Counts open requests so a slower read can tell it has been superseded.
  // A path would not be enough: opening A, then B, then A again would let the
  // first read pass while the third is still in flight.
  const openRequest = React.useRef(0);

  const group = SCRIPT_GROUPS.find((entry) => entry.id === groupId) ?? SCRIPT_GROUPS[0];
  const textFiles = React.useMemo(() => (scan?.files ?? []).filter((file) => file.kind === 'text'), [scan]);
  const matches = React.useMemo(() => filterFiles(textFiles, group, search), [textFiles, group, search]);
  const isDirty = content !== saved;

  useUnsavedIn('script editor', isDirty);

  async function openFile(file: ProjectFile) {
    if (isDirty && !(await confirmDiscard(`Discard unsaved changes to ${selected?.name}?`))) {
      return;
    }

    // Reads can finish out of order. A slow one landing last would show its
    // text under the newer file name, and Save would write it there.
    const request = ++openRequest.current;
    setSelected(file);
    setIsLoading(true);
    try {
      const loaded = await api.readTextFile(file.fullPath);
      if (openRequest.current !== request) return;

      setContent(loaded.content);
      setSaved(loaded.content);
      setHasBom(loaded.hasBom);
      setEncoding(loaded.encoding);
    } catch (error) {
      if (openRequest.current !== request) return;
      setError(describeError(error));
      setSelected(null);
    } finally {
      if (openRequest.current === request) setIsLoading(false);
    }
  }

  const save = React.useCallback(async () => {
    if (!selected) return;

    setIsSaving(true);
    try {
      const used = await api.writeTextFile(selected.fullPath, content, encoding, hasBom);
      setSaved(content);

      if (used === encoding) {
        toast.success(`Saved ${selected.name}`);
      } else {
        // The text outgrew Windows-1252, so the backend fell back to UTF-8.
        setEncoding(used);
        toast.warning(`Saved ${selected.name} as UTF-8 - the new text does not fit Windows-1252`);
      }
    } catch (error) {
      const message = describeError(error);
      setError(message);
      toast.error(message);
    } finally {
      setIsSaving(false);
    }
  }, [content, encoding, hasBom, selected, setError]);

  return (
    <div className='grid h-full grid-cols-[minmax(16rem,22rem)_1fr] gap-3 p-3'>
      <Panel>
        <PanelHeader
          title='Project files'
          subtitle={`${matches.length.toLocaleString()} of ${textFiles.length.toLocaleString()} text files`}
        />
        <div className='flex flex-col gap-2 border-b border-border p-2'>
          <Select value={groupId} onValueChange={setGroupId}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {SCRIPT_GROUPS.map((entry) => (
                <SelectItem key={entry.id} value={entry.id}>
                  {entry.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <div className='relative'>
            <Search className='pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted' />
            <Input
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder='Filter by path'
              className='pl-8'
            />
          </div>
        </div>

        <PanelBody>
          {isScanning ? (
            <div className='flex items-center gap-2 p-4 text-sm text-muted'>
              <Loader2 className='size-4 animate-spin' />
              Scanning…
            </div>
          ) : matches.length === 0 ? (
            <EmptyState title='Nothing here' description='No text files match this filter.' />
          ) : (
            <>
              <ul className='py-1'>
                {matches.slice(0, MAX_ROWS).map((file) => (
                  <li key={file.fullPath}>
                    <ListRow
                      active={selected?.fullPath === file.fullPath}
                      onClick={() => void openFile(file)}
                      title={file.relativePath}
                    >
                      <FileText className='size-3.5 shrink-0 opacity-70' />
                      <span className='truncate'>{file.relativePath}</span>
                      <span className='ml-auto shrink-0 text-[0.6875rem] text-muted'>
                        {formatBytes(file.sizeBytes)}
                      </span>
                    </ListRow>
                  </li>
                ))}
              </ul>
              {matches.length > MAX_ROWS ? (
                <p className='px-3 py-2 text-xs text-muted'>
                  Showing the first {MAX_ROWS} matches. Narrow the filter to see the rest.
                </p>
              ) : null}
            </>
          )}
        </PanelBody>
      </Panel>

      <Panel>
        <PanelHeader
          title={selected ? selected.relativePath : 'No file selected'}
          subtitle={
            selected
              ? `${formatBytes(selected.sizeBytes)}${hasBom ? ' · BOM' : ''}${encoding === 'windows1252' ? ' · Windows-1252' : ''}`
              : 'Pick a file from the list'
          }
          actions={
            selected ? (
              <>
                {isDirty ? <Badge tone='accent'>Unsaved</Badge> : null}
                <Button
                  variant='ghost'
                  size='icon-sm'
                  title='Reload from disk'
                  onClick={() => void openFile(selected)}
                  disabled={isLoading}
                >
                  <RotateCcw />
                </Button>
                <Button variant='primary' size='sm' onClick={() => void save()} disabled={!isDirty || isSaving}>
                  {isSaving ? <Loader2 className='animate-spin' /> : <Save />}
                  Save
                </Button>
              </>
            ) : null
          }
        />
        <PanelBody className='overflow-hidden'>
          {!selected ? (
            <EmptyState
              icon={<FileText />}
              title='Script editor'
              description='Open any .txt, .gfx, .gui or .yml file from the mod to edit it here. Ctrl+S saves, Ctrl+F searches.'
            />
          ) : isLoading ? (
            <div className='flex h-full items-center justify-center gap-2 text-sm text-muted'>
              <Loader2 className='size-4 animate-spin' />
              Loading…
            </div>
          ) : (
            <ScriptEditor value={content} onChange={setContent} onSave={() => void save()} className='h-full' />
          )}
        </PanelBody>
      </Panel>
    </div>
  );
}
