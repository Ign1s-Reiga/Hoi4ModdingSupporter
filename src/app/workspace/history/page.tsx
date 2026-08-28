'use client';

import * as React from 'react';
import { Landmark, Loader2, Save, Search, TriangleAlert, X } from 'lucide-react';
import { toast } from 'sonner';

import { MapCanvas, type MapPickPoint } from '@/components/map-canvas';
import { Button } from '@/components/ui/button';
import { CodeInput, Field, Label } from '@/components/ui/form';
import { Badge, EmptyState, Panel, PanelBody, PanelHeader } from '@/components/ui/panel';
import { confirmDiscard } from '@/lib/dialogs';
import { api, describeError } from '@/lib/ipc';
import { useAppStore } from '@/lib/store';
import { useProvinceMap } from '@/lib/use-map';
import { useUnsavedIn } from '@/lib/use-unsaved';
import {
  toCountryHistoryUpdate,
  type CountryHistory,
  type CountryHistoryInfo,
  type CountryHistoryUpdate,
  type ProvincePick,
} from '@/lib/types';

export default function CountryHistoryPage() {
  const project = useAppStore((state) => state.project);
  const setError = useAppStore((state) => state.setError);
  const map = useProvinceMap('owners');

  const [listing, setListing] = React.useState<{ folder: string; files: CountryHistoryInfo[] } | null>(null);
  const [picked, setPicked] = React.useState<ProvincePick | null>(null);
  const [marker, setMarker] = React.useState<MapPickPoint | null>(null);
  const [country, setCountry] = React.useState<CountryHistory | null>(null);
  const [draft, setDraft] = React.useState<CountryHistoryUpdate | null>(null);
  const [isDirty, setIsDirty] = React.useState(false);
  const [isLoading, setIsLoading] = React.useState(false);
  const [isSaving, setIsSaving] = React.useState(false);
  const openRequest = React.useRef(0);

  useUnsavedIn('country history editor', isDirty);

  const folder = project?.folderPath ?? '';
  const files = React.useMemo(() => (listing?.folder === folder ? listing.files : []), [folder, listing]);

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

  async function onPick(point: MapPickPoint) {
    if (isDirty && !(await confirmDiscard('Discard unsaved country changes?'))) return;

    const request = ++openRequest.current;
    setMarker(point);
    setIsLoading(true);
    try {
      const hit = await map.pick(point.x, point.y);
      if (openRequest.current !== request) return;
      setPicked(hit);

      // The map knows the owning tag; the country file is found by that tag.
      const file = hit?.owner ? files.find((entry) => entry.tag === hit.owner) : undefined;
      if (!file) {
        setCountry(null);
        setDraft(null);
        return;
      }

      await openCountry(file, request);
    } catch (error) {
      if (openRequest.current !== request) return;
      setError(describeError(error));
    } finally {
      if (openRequest.current === request) setIsLoading(false);
    }
  }

  /**
   * Opens a country file directly, for the ones the map cannot reach: a
   * releasable or exiled country owns no province to click.
   */
  async function openFromList(file: CountryHistoryInfo) {
    if (isDirty && !(await confirmDiscard('Discard unsaved country changes?'))) return;

    const request = ++openRequest.current;
    setIsLoading(true);
    try {
      setPicked(null);
      setMarker(null);
      await openCountry(file, request);
    } catch (error) {
      if (openRequest.current !== request) return;
      setError(describeError(error));
    } finally {
      if (openRequest.current === request) setIsLoading(false);
    }
  }

  async function openCountry(file: CountryHistoryInfo, request: number) {
    const loaded = await api.readCountryHistory(file.path);
    if (openRequest.current !== request) return;

    setCountry(loaded);
    setDraft(toCountryHistoryUpdate(loaded));
    setIsDirty(false);
  }

  function patch(changes: Partial<CountryHistoryUpdate>) {
    setDraft((current) => (current ? { ...current, ...changes } : current));
    setIsDirty(true);
  }

  async function save() {
    if (!country || !draft) return;

    setIsSaving(true);
    try {
      const updated = await api.updateCountryHistory(country.path, draft);
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
    <div className='grid h-full grid-cols-[minmax(0,1fr)_24rem] gap-3 p-3'>
      <Panel>
        <PanelHeader
          title='Starting ownership'
          subtitle={
            map.summary ? `${map.summary.stateCount} states painted in their owner colours` : 'Coloured by owner'
          }
        />
        <PanelBody className='overflow-hidden p-0'>
          {map.error ? (
            <EmptyState icon={<Landmark />} title='The map could not be loaded' description={map.error} />
          ) : (
            <MapCanvas
              source={map.source}
              width={map.summary?.width ?? 0}
              height={map.summary?.height ?? 0}
              marker={marker}
              onPick={(point) => void onPick(point)}
              isBusy={map.isLoading}
            />
          )}
        </PanelBody>
      </Panel>

      <Panel>
        <PanelHeader
          title={country ? `${country.tag} · ${country.fileName}` : 'No country selected'}
          subtitle={
            picked
              ? `province ${picked.provinceId}${picked.stateName ? ` · ${picked.stateName}` : ''}${picked.owner ? ` · owned by ${picked.owner}` : ' · unowned'}`
              : 'Click a province to open the country that owns it'
          }
          actions={
            draft ? (
              <>
                {country && country.unclosedBlocks > 0 ? (
                  <Badge tone='danger' title='The game tolerates this, but a brace is missing'>
                    <TriangleAlert className='size-3' />
                    Unclosed
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
        <PanelBody className='grid content-start gap-3 p-3'>
          <CountryPicker files={files} onOpen={(file) => void openFromList(file)} />

          {isLoading ? (
            <div className='flex h-full items-center justify-center gap-2 text-sm text-muted'>
              <Loader2 className='size-4 animate-spin' />
              Reading country…
            </div>
          ) : !draft ? (
            <EmptyState
              icon={<Landmark />}
              title={picked ? 'No country file for this province' : 'Country setup editor'}
              description={
                picked
                  ? picked.owner
                    ? `${picked.owner} owns this state but has no file under history/countries.`
                    : 'No state owns this province, so there is no country to edit.'
                  : 'Click a province to edit where its owner starts: capital, order of battle, politics and party support.'
              }
            />
          ) : (
            <CountryForm draft={draft} onChange={patch} />
          )}
        </PanelBody>
      </Panel>
    </div>
  );
}

/**
 * Finds a country file by tag or name.
 *
 * The map cannot reach every country: releasable, exiled and formable ones own
 * no province at the start, so clicking will never open them.
 */
function CountryPicker({ files, onOpen }: { files: CountryHistoryInfo[]; onOpen: (file: CountryHistoryInfo) => void }) {
  const [query, setQuery] = React.useState('');

  const matches = React.useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return [];

    return files
      .filter((file) => file.tag.toLowerCase().includes(needle) || file.fileName.toLowerCase().includes(needle))
      .slice(0, 8);
  }, [files, query]);

  return (
    <div className='grid gap-1.5'>
      <div className='relative'>
        <Search className='pointer-events-none absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-muted' />
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={`Find one of ${files.length} country files`}
          className='h-8 w-full rounded-md border border-border bg-surface pl-7 pr-7 text-xs outline-none focus:border-accent'
        />
        {query ? (
          <button
            type='button'
            onClick={() => setQuery('')}
            title='Clear'
            className='absolute right-1.5 top-1/2 -translate-y-1/2 rounded p-0.5 text-muted hover:text-fg'
          >
            <X className='size-3.5' />
          </button>
        ) : null}
      </div>

      {query.trim() && matches.length === 0 ? <p className='text-xs text-muted'>No country file matches.</p> : null}

      {matches.map((file) => (
        <button
          key={file.path}
          type='button'
          onClick={() => {
            setQuery('');
            onOpen(file);
          }}
          className='flex items-center gap-2 rounded-md border border-border px-2 py-1 text-left text-xs hover:border-accent'
        >
          <span className='font-mono text-accent'>{file.tag || '???'}</span>
          <span className='truncate text-muted'>{file.fileName}</span>
        </button>
      ))}
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
