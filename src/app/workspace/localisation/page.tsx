'use client';

import * as React from 'react';
import { AlertTriangle, Languages, Loader2, Plus, Save, Search, Trash2 } from 'lucide-react';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import { CodeInput, Input } from '@/components/ui/form';
import { Badge, EmptyState, ListRow, Panel, PanelBody, PanelHeader } from '@/components/ui/panel';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { confirmDiscard } from '@/lib/dialogs';
import { api, describeError } from '@/lib/ipc';
import { useAppStore } from '@/lib/store';
import type { LocalisationEntry, LocalisationFileInfo } from '@/lib/types';

const MAX_ROWS = 300;
const ALL_LANGUAGES = 'all';

export default function LocalisationPage() {
  const { project, setError } = useAppStore();

  // The listing is stored with the folder it belongs to, so a project change
  // shows the loading state without an effect resetting it first.
  const [listing, setListing] = React.useState<{
    folder: string;
    files: LocalisationFileInfo[];
  } | null>(null);
  const [language, setLanguage] = React.useState(ALL_LANGUAGES);
  const [selected, setSelected] = React.useState<LocalisationFileInfo | null>(null);
  const [entries, setEntries] = React.useState<LocalisationEntry[]>([]);
  const [saved, setSaved] = React.useState<LocalisationEntry[]>([]);
  const [fileLanguage, setFileLanguage] = React.useState('');
  const [hasBom, setHasBom] = React.useState(true);
  const [search, setSearch] = React.useState('');
  const [isLoading, setIsLoading] = React.useState(false);
  const [isSaving, setIsSaving] = React.useState(false);

  const folder = project?.folderPath ?? '';
  const files = React.useMemo(() => (listing?.folder === folder ? listing.files : []), [folder, listing]);
  const isListing = Boolean(folder) && listing?.folder !== folder;
  const isDirty = JSON.stringify(entries) !== JSON.stringify(saved);

  const languages = React.useMemo(() => {
    const found = new Set(files.map((file) => file.language).filter(Boolean));
    return [ALL_LANGUAGES, ...[...found].sort()];
  }, [files]);

  const visibleFiles = React.useMemo(
    () => files.filter((file) => language === ALL_LANGUAGES || file.language === language),
    [files, language],
  );

  const visibleEntries = React.useMemo(() => {
    const needle = search.trim().toLowerCase();
    if (!needle) return entries;
    return entries.filter(
      (entry) => entry.key.toLowerCase().includes(needle) || entry.value.toLowerCase().includes(needle),
    );
  }, [entries, search]);

  const refreshFiles = React.useCallback(async () => {
    if (!folder) return;

    try {
      setListing({ folder, files: await api.listLocalisationFiles(folder) });
    } catch (error) {
      setListing({ folder, files: [] });
      setError(describeError(error));
    }
  }, [folder, setError]);

  React.useEffect(() => {
    // refreshFiles awaits the backend before it touches any state, so this
    // does not set state synchronously during the effect.
    // oxlint-disable-next-line react/set-state-in-effect
    void refreshFiles();
  }, [refreshFiles]);

  async function openFile(file: LocalisationFileInfo) {
    if (isDirty && !(await confirmDiscard('Discard unsaved localisation changes?'))) return;

    setSelected(file);
    setIsLoading(true);
    try {
      const loaded = await api.readLocalisationFile(file.path);
      setEntries(loaded.entries);
      setSaved(loaded.entries);
      setFileLanguage(loaded.language);
      setHasBom(loaded.hasBom);
    } catch (error) {
      setError(describeError(error));
      setSelected(null);
    } finally {
      setIsLoading(false);
    }
  }

  function patchEntry(target: LocalisationEntry, changes: Partial<LocalisationEntry>) {
    setEntries((current) => current.map((entry) => (entry === target ? { ...entry, ...changes } : entry)));
  }

  function addEntry() {
    setEntries((current) => [...current, { key: '', version: '0', value: '', line: null }]);
  }

  function removeEntry(target: LocalisationEntry) {
    setEntries((current) => current.filter((entry) => entry !== target));
  }

  async function save() {
    if (!selected) return;

    setIsSaving(true);
    try {
      const result = await api.writeLocalisationFile(selected.path, fileLanguage, entries);
      setEntries(result.entries);
      setSaved(result.entries);
      setHasBom(result.hasBom);
      toast.success(`Saved ${result.entries.length} entries`);
      void refreshFiles();
    } catch (error) {
      const message = describeError(error);
      setError(message);
      toast.error(message);
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className='grid h-full grid-cols-[minmax(15rem,20rem)_1fr] gap-3 p-3'>
      <Panel>
        <PanelHeader title='Localisation files' subtitle={`${visibleFiles.length} of ${files.length} files`} />
        <div className='border-b border-border p-2'>
          <Select value={language} onValueChange={setLanguage}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {languages.map((entry) => (
                <SelectItem key={entry} value={entry}>
                  {entry === ALL_LANGUAGES ? 'All languages' : entry}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <PanelBody>
          {isListing ? (
            <div className='flex items-center gap-2 p-4 text-sm text-muted'>
              <Loader2 className='size-4 animate-spin' />
              Reading localisation…
            </div>
          ) : visibleFiles.length === 0 ? (
            <EmptyState
              icon={<Languages />}
              title='No localisation files'
              description='This mod has no .yml files under localisation/.'
            />
          ) : (
            <ul className='py-1'>
              {visibleFiles.map((file) => (
                <li key={file.path}>
                  <ListRow
                    active={selected?.path === file.path}
                    onClick={() => void openFile(file)}
                    title={file.relativePath}
                  >
                    <span className='truncate'>{file.relativePath}</span>
                    <span className='ml-auto flex shrink-0 items-center gap-1'>
                      {!file.hasBom ? <AlertTriangle className='size-3.5 text-warning' /> : null}
                      <span className='text-[0.6875rem] text-muted'>{file.entryCount}</span>
                    </span>
                  </ListRow>
                </li>
              ))}
            </ul>
          )}
        </PanelBody>
      </Panel>

      <Panel>
        <PanelHeader
          title={selected ? selected.relativePath : 'No file selected'}
          subtitle={
            selected
              ? `${fileLanguage || 'no language header'} · ${entries.length} entries`
              : 'Pick a file to edit its keys'
          }
          actions={
            selected ? (
              <>
                {isDirty ? <Badge tone='accent'>Unsaved</Badge> : null}
                <Button variant='ghost' size='sm' onClick={addEntry}>
                  <Plus />
                  Add key
                </Button>
                <Button variant='primary' size='sm' onClick={() => void save()} disabled={!isDirty || isSaving}>
                  {isSaving ? <Loader2 className='animate-spin' /> : <Save />}
                  Save
                </Button>
              </>
            ) : null
          }
        />

        {selected ? (
          <div className='flex items-center gap-2 border-b border-border p-2'>
            <div className='relative flex-1'>
              <Search className='pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted' />
              <Input
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                placeholder='Filter by key or text'
                className='pl-8'
              />
            </div>
            {!hasBom ? (
              <Badge tone='danger' className='shrink-0'>
                <AlertTriangle className='size-3' />
                No BOM — the game ignores this file. Saving adds one.
              </Badge>
            ) : null}
          </div>
        ) : null}

        <PanelBody>
          {!selected ? (
            <EmptyState
              icon={<Languages />}
              title='Localisation editor'
              description='Keys, versions and text are edited in place. Comments and untouched lines are preserved, and every save writes the BOM the game needs.'
            />
          ) : isLoading ? (
            <div className='flex h-full items-center justify-center gap-2 text-sm text-muted'>
              <Loader2 className='size-4 animate-spin' />
              Loading entries…
            </div>
          ) : entries.length === 0 ? (
            <EmptyState
              title='No entries'
              description='This file has no localisation keys yet.'
              action={
                <Button onClick={addEntry}>
                  <Plus />
                  Add the first key
                </Button>
              }
            />
          ) : (
            <table className='w-full border-collapse text-sm'>
              <thead className='sticky top-0 bg-surface-raised text-left text-xs uppercase tracking-wide text-muted'>
                <tr>
                  <th className='w-[30%] px-3 py-2 font-medium'>Key</th>
                  <th className='w-14 px-2 py-2 font-medium'>Ver</th>
                  <th className='px-3 py-2 font-medium'>Text</th>
                  <th className='w-10' />
                </tr>
              </thead>
              <tbody>
                {visibleEntries.slice(0, MAX_ROWS).map((entry, index) => (
                  <tr key={entry.line ?? `new-${index}`} className='border-t border-border'>
                    <td className='px-2 py-1'>
                      <CodeInput
                        value={entry.key}
                        onChange={(event) => patchEntry(entry, { key: event.target.value })}
                        className='h-8 border-transparent bg-transparent hover:border-border'
                      />
                    </td>
                    <td className='px-1 py-1'>
                      <CodeInput
                        value={entry.version}
                        onChange={(event) => patchEntry(entry, { version: event.target.value })}
                        className='h-8 border-transparent bg-transparent px-1 text-center hover:border-border'
                      />
                    </td>
                    <td className='px-2 py-1'>
                      <Input
                        value={entry.value}
                        onChange={(event) => patchEntry(entry, { value: event.target.value })}
                        className='h-8 border-transparent bg-transparent hover:border-border'
                      />
                    </td>
                    <td className='px-1 py-1'>
                      <Button variant='ghost' size='icon-sm' title='Remove key' onClick={() => removeEntry(entry)}>
                        <Trash2 />
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}

          {visibleEntries.length > MAX_ROWS ? (
            <p className='px-3 py-2 text-xs text-muted'>
              Showing the first {MAX_ROWS} of {visibleEntries.length} entries. Filter to reach the rest — every entry is
              still saved.
            </p>
          ) : null}
        </PanelBody>
      </Panel>
    </div>
  );
}
