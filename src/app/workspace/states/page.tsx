'use client';

import * as React from 'react';
import { FileText, Loader2, Map, Save, Search, TriangleAlert } from 'lucide-react';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import { CodeInput, Field, Input, Label, Textarea } from '@/components/ui/form';
import { Badge, EmptyState, ListRow, Panel, PanelBody, PanelHeader } from '@/components/ui/panel';
import { confirmDiscard } from '@/lib/dialogs';
import { api, describeError } from '@/lib/ipc';
import { useAppStore } from '@/lib/store';
import { useUnsavedIn } from '@/lib/use-unsaved';
import { toStateUpdate, type ProjectFile, type StateFile, type StateUpdate } from '@/lib/types';
import { isInFolder } from '@/lib/utils';

const STATE_FOLDER = 'history/states';
/** A total conversion ships a file per state, so the list has to be filtered. */
const MAX_ROWS = 300;

export default function StatesPage() {
  const { scan, setError } = useAppStore();

  const [search, setSearch] = React.useState('');
  const [selectedFile, setSelectedFile] = React.useState<ProjectFile | null>(null);
  const [stateFile, setStateFile] = React.useState<StateFile | null>(null);
  const [selectedId, setSelectedId] = React.useState<string | null>(null);
  const [draft, setDraft] = React.useState<StateUpdate | null>(null);
  const [isDirty, setIsDirty] = React.useState(false);
  const [isLoading, setIsLoading] = React.useState(false);
  const [isSaving, setIsSaving] = React.useState(false);
  // Counts open requests so a slower read can tell it has been superseded.
  const openRequest = React.useRef(0);

  useUnsavedIn('state editor', isDirty);

  const files = React.useMemo(
    () => (scan?.files ?? []).filter((file) => isInFolder(file.relativePath, STATE_FOLDER) && file.extension === 'txt'),
    [scan],
  );

  const matches = React.useMemo(() => {
    const needle = search.trim().toLowerCase();
    if (!needle) return files;
    return files.filter((file) => file.name.toLowerCase().includes(needle));
  }, [files, search]);

  const states = stateFile?.states ?? [];
  const selected = states.find((state) => state.id === selectedId) ?? null;

  async function openFile(file: ProjectFile) {
    if (isDirty && !(await confirmDiscard('Discard unsaved state changes?'))) return;

    const request = ++openRequest.current;
    setSelectedFile(file);
    setIsLoading(true);
    try {
      const loaded = await api.readStateFile(file.fullPath);
      if (openRequest.current !== request) return;

      applyFile(loaded, loaded.states[0]?.id ?? null);
    } catch (error) {
      if (openRequest.current !== request) return;

      setError(describeError(error));
      setStateFile(null);
      setSelectedId(null);
      setDraft(null);
    } finally {
      if (openRequest.current === request) setIsLoading(false);
    }
  }

  function applyFile(loaded: StateFile, stateId: string | null) {
    setStateFile(loaded);
    setSelectedId(stateId);
    const state = loaded.states.find((entry) => entry.id === stateId);
    setDraft(state ? toStateUpdate(state) : null);
    setIsDirty(false);
  }

  async function selectState(id: string) {
    if (id === selectedId) return;
    if (isDirty && !(await confirmDiscard('Discard unsaved state changes?'))) return;

    const state = states.find((entry) => entry.id === id);
    setSelectedId(id);
    setDraft(state ? toStateUpdate(state) : null);
    setIsDirty(false);
  }

  function patch(changes: Partial<StateUpdate>) {
    setDraft((current) => (current ? { ...current, ...changes } : current));
    setIsDirty(true);
  }

  async function save() {
    if (!selectedFile || !draft || !selectedId) return;

    setIsSaving(true);
    try {
      const updated = await api.updateState(selectedFile.fullPath, selectedId, draft);
      applyFile(updated, draft.id || selectedId);
      toast.success(`Saved state ${draft.id || selectedId}`);
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
        <PanelHeader title='State files' subtitle={`${matches.length} of ${files.length} in ${STATE_FOLDER}`} />
        <div className='border-b border-border p-2'>
          <div className='relative'>
            <Search className='pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted' />
            <Input
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder='Filter by file name'
              className='pl-8'
            />
          </div>
        </div>
        <PanelBody>
          {files.length === 0 ? (
            <EmptyState
              icon={<Map />}
              title='No state files'
              description={`This mod has no .txt files under ${STATE_FOLDER}.`}
            />
          ) : matches.length === 0 ? (
            <EmptyState title='Nothing here' description='No state file matches that filter.' />
          ) : (
            <>
              <ul className='py-1'>
                {matches.slice(0, MAX_ROWS).map((file) => (
                  <li key={file.fullPath}>
                    <ListRow
                      active={selectedFile?.fullPath === file.fullPath}
                      onClick={() => void openFile(file)}
                      title={file.relativePath}
                    >
                      <FileText className='size-3.5 shrink-0 opacity-70' />
                      <span className='truncate'>{file.name}</span>
                    </ListRow>
                  </li>
                ))}
              </ul>
              {matches.length > MAX_ROWS ? (
                <p className='px-3 py-2 text-xs text-muted'>
                  Showing the first {MAX_ROWS} of {matches.length}. Narrow the filter to see the rest.
                </p>
              ) : null}
            </>
          )}
        </PanelBody>
      </Panel>

      <Panel>
        <PanelHeader
          title={selected ? `${selected.name || 'unnamed state'} · id ${selected.id}` : 'No state selected'}
          subtitle={selectedFile ? selectedFile.relativePath : 'Pick a file from the list'}
          actions={
            selected ? (
              <>
                {stateFile && stateFile.unclosedBlocks > 0 ? (
                  <Badge tone='danger' title='The game tolerates this, but a brace is missing'>
                    <TriangleAlert className='size-3' />
                    Unclosed block
                  </Badge>
                ) : null}
                {isDirty ? <Badge tone='accent'>Unsaved</Badge> : null}
                <Button variant='primary' size='sm' onClick={() => void save()} disabled={!isDirty || isSaving}>
                  {isSaving ? <Loader2 className='animate-spin' /> : <Save />}
                  Save
                </Button>
              </>
            ) : null
          }
        />
        <PanelBody className='p-3'>
          {isLoading ? (
            <div className='flex h-full items-center justify-center gap-2 text-sm text-muted'>
              <Loader2 className='size-4 animate-spin' />
              Reading state…
            </div>
          ) : !draft ? (
            <EmptyState
              icon={<Map />}
              title='State editor'
              description='Open a file from history/states to edit its ownership, cores, victory points and buildings.'
            />
          ) : (
            <>
              {states.length > 1 ? (
                <div className='mb-3 flex flex-wrap gap-1.5'>
                  {states.map((state) => (
                    <Button
                      key={state.id}
                      size='sm'
                      variant={state.id === selectedId ? 'primary' : 'default'}
                      onClick={() => void selectState(state.id)}
                    >
                      {state.id}
                    </Button>
                  ))}
                </div>
              ) : null}

              <StateForm draft={draft} onChange={patch} />
            </>
          )}
        </PanelBody>
      </Panel>
    </div>
  );
}

function StateForm({ draft, onChange }: { draft: StateUpdate; onChange: (changes: Partial<StateUpdate>) => void }) {
  return (
    <div className='grid gap-3 lg:grid-cols-2'>
      <Field label='Id'>
        <CodeInput value={draft.id} onChange={(event) => onChange({ id: event.target.value })} />
      </Field>
      <Field label='Name' hint='Localisation key, e.g. STATE_1'>
        <CodeInput value={draft.name} onChange={(event) => onChange({ name: event.target.value })} />
      </Field>

      <Field label='Owner'>
        <CodeInput value={draft.owner} onChange={(event) => onChange({ owner: event.target.value })} />
      </Field>
      <Field label='Controller' hint='Leave empty when the owner controls it'>
        <CodeInput value={draft.controller} onChange={(event) => onChange({ controller: event.target.value })} />
      </Field>

      <Field label='Manpower'>
        <CodeInput value={draft.manpower} onChange={(event) => onChange({ manpower: event.target.value })} />
      </Field>
      <Field label='State category'>
        <CodeInput value={draft.stateCategory} onChange={(event) => onChange({ stateCategory: event.target.value })} />
      </Field>

      <Field label='Cores' hint='Country tags, separated by spaces'>
        <CodeInput
          value={draft.cores.join(' ')}
          onChange={(event) => onChange({ cores: event.target.value.split(/\s+/).filter(Boolean) })}
        />
      </Field>
      <Field label='Claims' hint='Country tags, separated by spaces'>
        <CodeInput
          value={draft.claims.join(' ')}
          onChange={(event) => onChange({ claims: event.target.value.split(/\s+/).filter(Boolean) })}
        />
      </Field>

      <Field label='Local supplies'>
        <CodeInput value={draft.localSupplies} onChange={(event) => onChange({ localSupplies: event.target.value })} />
      </Field>
      <Field label='Buildings max level factor'>
        <CodeInput
          value={draft.buildingsMaxLevelFactor}
          onChange={(event) => onChange({ buildingsMaxLevelFactor: event.target.value })}
        />
      </Field>

      <Field label='Provinces' hint='Province ids, separated by spaces' className='lg:col-span-2'>
        <Textarea
          rows={3}
          value={draft.provinces.join(' ')}
          onChange={(event) => onChange({ provinces: event.target.value.split(/\s+/).filter(Boolean) })}
        />
      </Field>

      <div className='flex flex-col gap-1.5 lg:col-span-2'>
        <Label>Victory points</Label>
        {draft.victoryPoints.length === 0 ? <p className='text-xs text-muted'>None.</p> : null}
        {draft.victoryPoints.map((point, index) => (
          <CodeInput
            key={index}
            value={point}
            placeholder='province value'
            onChange={(event) => {
              const next = [...draft.victoryPoints];
              next[index] = event.target.value;
              onChange({ victoryPoints: next });
            }}
          />
        ))}
        <div className='flex gap-2'>
          <Button size='sm' onClick={() => onChange({ victoryPoints: [...draft.victoryPoints, ''] })}>
            Add point
          </Button>
          {draft.victoryPoints.length > 0 ? (
            <Button size='sm' onClick={() => onChange({ victoryPoints: draft.victoryPoints.slice(0, -1) })}>
              Remove last
            </Button>
          ) : null}
        </div>
      </div>

      <Field label='Resources' hint='One per line, e.g. oil = 5' className='lg:col-span-2'>
        <Textarea rows={3} value={draft.resources} onChange={(event) => onChange({ resources: event.target.value })} />
      </Field>

      <Field label='Buildings' hint='Province-keyed blocks are kept as written' className='lg:col-span-2'>
        <Textarea rows={6} value={draft.buildings} onChange={(event) => onChange({ buildings: event.target.value })} />
      </Field>
    </div>
  );
}
