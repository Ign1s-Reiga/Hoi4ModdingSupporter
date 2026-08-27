'use client';

import * as React from 'react';
import { Landmark, Loader2, Save, Search, TriangleAlert, X } from 'lucide-react';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import { CodeInput, Field, Input, Label } from '@/components/ui/form';
import { Badge, EmptyState, ListRow, Panel, PanelBody, PanelHeader } from '@/components/ui/panel';
import { confirmDiscard } from '@/lib/dialogs';
import { api, describeError } from '@/lib/ipc';
import { useAppStore } from '@/lib/store';
import { useUnsavedIn } from '@/lib/use-unsaved';
import {
  toCountryHistoryUpdate,
  type CountryHistory,
  type CountryHistoryInfo,
  type CountryHistoryUpdate,
} from '@/lib/types';

const MAX_ROWS = 300;

export default function CountryHistoryPage() {
  const { project, setError } = useAppStore();

  const [listing, setListing] = React.useState<{ folder: string; files: CountryHistoryInfo[] } | null>(null);
  const [search, setSearch] = React.useState('');
  const [selected, setSelected] = React.useState<CountryHistoryInfo | null>(null);
  const [country, setCountry] = React.useState<CountryHistory | null>(null);
  const [draft, setDraft] = React.useState<CountryHistoryUpdate | null>(null);
  const [isDirty, setIsDirty] = React.useState(false);
  const [isLoading, setIsLoading] = React.useState(false);
  const [isSaving, setIsSaving] = React.useState(false);
  const openRequest = React.useRef(0);

  useUnsavedIn('country history editor', isDirty);

  const folder = project?.folderPath ?? '';
  const files = React.useMemo(() => (listing?.folder === folder ? listing.files : []), [folder, listing]);
  const isListing = Boolean(folder) && listing?.folder !== folder;

  React.useEffect(() => {
    if (!folder) return;

    let cancelled = false;
    api
      .listCountryHistory(folder)
      .then((result) => {
        if (!cancelled) setListing({ folder, files: result });
      })
      .catch((error) => {
        if (cancelled) return;
        setListing({ folder, files: [] });
        setError(describeError(error));
      });

    return () => {
      cancelled = true;
    };
  }, [folder, setError]);

  const matches = React.useMemo(() => {
    const needle = search.trim().toLowerCase();
    if (!needle) return files;
    return files.filter((file) => file.fileName.toLowerCase().includes(needle));
  }, [files, search]);

  async function openCountry(file: CountryHistoryInfo) {
    if (isDirty && !(await confirmDiscard('Discard unsaved country changes?'))) return;

    const request = ++openRequest.current;
    setSelected(file);
    setIsLoading(true);
    try {
      const loaded = await api.readCountryHistory(file.path);
      if (openRequest.current !== request) return;

      setCountry(loaded);
      setDraft(toCountryHistoryUpdate(loaded));
      setIsDirty(false);
    } catch (error) {
      if (openRequest.current !== request) return;

      setError(describeError(error));
      setCountry(null);
      setDraft(null);
    } finally {
      if (openRequest.current === request) setIsLoading(false);
    }
  }

  function patch(changes: Partial<CountryHistoryUpdate>) {
    setDraft((current) => (current ? { ...current, ...changes } : current));
    setIsDirty(true);
  }

  async function save() {
    if (!selected || !draft) return;

    setIsSaving(true);
    try {
      const updated = await api.updateCountryHistory(selected.path, draft);
      setCountry(updated);
      setDraft(toCountryHistoryUpdate(updated));
      setIsDirty(false);
      toast.success(`Saved ${updated.tag || updated.fileName}`);
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
        <PanelHeader title='Countries' subtitle={`${matches.length} of ${files.length} in history/countries`} />
        <div className='border-b border-border p-2'>
          <div className='relative'>
            <Search className='pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted' />
            <Input
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder='Filter by tag or name'
              className='pl-8'
            />
          </div>
        </div>
        <PanelBody>
          {isListing ? (
            <div className='flex items-center gap-2 p-4 text-sm text-muted'>
              <Loader2 className='size-4 animate-spin' />
              Reading countries…
            </div>
          ) : files.length === 0 ? (
            <EmptyState
              icon={<Landmark />}
              title='No country files'
              description='This mod has no .txt files under history/countries.'
            />
          ) : (
            <>
              <ul className='py-1'>
                {matches.slice(0, MAX_ROWS).map((file) => (
                  <li key={file.path}>
                    <ListRow
                      active={selected?.path === file.path}
                      onClick={() => void openCountry(file)}
                      title={file.relativePath}
                    >
                      <span className='w-10 shrink-0 font-mono text-[0.6875rem] text-muted'>{file.tag}</span>
                      <span className='truncate'>{file.fileName}</span>
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
          title={country ? `${country.tag} · ${country.fileName}` : 'No country selected'}
          subtitle={
            country
              ? 'Only the fields below are written; technology, characters and if blocks are left alone'
              : 'Pick a country from the list'
          }
          actions={
            draft ? (
              <>
                {country && country.unclosedBlocks > 0 ? (
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
              Reading country…
            </div>
          ) : !draft ? (
            <EmptyState
              icon={<Landmark />}
              title='Country setup editor'
              description='Open a country to edit where it starts: capital, order of battle, research slots, politics and party support.'
            />
          ) : (
            <CountryForm draft={draft} onChange={patch} />
          )}
        </PanelBody>
      </Panel>
    </div>
  );
}

function CountryForm({
  draft,
  onChange,
}: {
  draft: CountryHistoryUpdate;
  onChange: (changes: Partial<CountryHistoryUpdate>) => void;
}) {
  const total = draft.popularities.reduce((sum, entry) => sum + (Number.parseFloat(entry.value) || 0), 0);

  return (
    <div className='flex flex-col gap-4'>
      <div className='grid gap-3 lg:grid-cols-3'>
        <Field label='Capital' hint='Province id'>
          <CodeInput value={draft.capital} onChange={(event) => onChange({ capital: event.target.value })} />
        </Field>
        <Field label='Order of battle' hint='Name of the file under history/units'>
          <CodeInput value={draft.oob} onChange={(event) => onChange({ oob: event.target.value })} />
        </Field>
        <Field label='Research slots'>
          <CodeInput
            value={draft.researchSlots}
            onChange={(event) => onChange({ researchSlots: event.target.value })}
          />
        </Field>

        <Field label='Convoys'>
          <CodeInput value={draft.convoys} onChange={(event) => onChange({ convoys: event.target.value })} />
        </Field>
        <Field label='Stability' hint='0 to 1'>
          <CodeInput value={draft.stability} onChange={(event) => onChange({ stability: event.target.value })} />
        </Field>
        <Field label='War support' hint='0 to 1'>
          <CodeInput value={draft.warSupport} onChange={(event) => onChange({ warSupport: event.target.value })} />
        </Field>
      </div>

      <div className='space-y-2 rounded-lg border border-border p-3'>
        <Label>Politics</Label>
        <div className='grid gap-3 lg:grid-cols-2'>
          <Field label='Ruling party'>
            <CodeInput
              value={draft.politics.rulingParty}
              onChange={(event) => onChange({ politics: { ...draft.politics, rulingParty: event.target.value } })}
            />
          </Field>
          <Field label='Last election' hint='Date, e.g. 1935.11.14'>
            <CodeInput
              value={draft.politics.lastElection}
              onChange={(event) => onChange({ politics: { ...draft.politics, lastElection: event.target.value } })}
            />
          </Field>
          <Field label='Election frequency' hint='Months'>
            <CodeInput
              value={draft.politics.electionFrequency}
              onChange={(event) => onChange({ politics: { ...draft.politics, electionFrequency: event.target.value } })}
            />
          </Field>
          <Field label='Elections allowed' hint='yes or no'>
            <CodeInput
              value={draft.politics.electionsAllowed}
              onChange={(event) => onChange({ politics: { ...draft.politics, electionsAllowed: event.target.value } })}
            />
          </Field>
        </div>
      </div>

      <div className='space-y-2 rounded-lg border border-border p-3'>
        <div className='flex items-center justify-between'>
          <Label>Party popularity</Label>
          <Badge tone={Math.round(total) === 100 ? 'neutral' : 'danger'}>{total || 0} total</Badge>
        </div>

        {draft.popularities.length === 0 ? (
          <p className='text-xs text-muted'>This country sets no popularities.</p>
        ) : null}

        <div className='flex flex-col gap-1.5'>
          {draft.popularities.map((popularity, index) => (
            <div key={index} className='flex items-center gap-1.5'>
              <CodeInput
                value={popularity.ideology}
                placeholder='ideology'
                onChange={(event) => {
                  const next = [...draft.popularities];
                  next[index] = { ...next[index], ideology: event.target.value };
                  onChange({ popularities: next });
                }}
              />
              <CodeInput
                value={popularity.value}
                placeholder='0'
                className='w-24 shrink-0'
                onChange={(event) => {
                  const next = [...draft.popularities];
                  next[index] = { ...next[index], value: event.target.value };
                  onChange({ popularities: next });
                }}
              />
              <Button
                variant='ghost'
                size='icon-sm'
                title='Remove'
                onClick={() => onChange({ popularities: draft.popularities.filter((_, at) => at !== index) })}
              >
                <X />
              </Button>
            </div>
          ))}
        </div>

        <Button
          size='sm'
          onClick={() => onChange({ popularities: [...draft.popularities, { ideology: '', value: '' }] })}
        >
          Add ideology
        </Button>
        <p className='text-xs text-muted'>
          The game expects these to add up to 100. Removing a row deletes it from the file.
        </p>
      </div>
    </div>
  );
}
