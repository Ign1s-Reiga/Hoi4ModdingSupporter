'use client';

import * as React from 'react';
import { Loader2, Map, Save, TriangleAlert } from 'lucide-react';
import { toast } from 'sonner';

import { MapCanvas, type MapPickPoint } from '@/components/map-canvas';
import { Button } from '@/components/ui/button';
import { CodeInput, Field, Label, Textarea } from '@/components/ui/form';
import { Badge, EmptyState, Panel, PanelBody, PanelHeader } from '@/components/ui/panel';
import { confirmDiscard } from '@/lib/dialogs';
import { api, describeError } from '@/lib/ipc';
import { useAppStore } from '@/lib/store';
import { useProvinceMap } from '@/lib/use-map';
import { useUnsavedIn } from '@/lib/use-unsaved';
import { toStateUpdate, type ProvincePick, type StateFile, type StateUpdate } from '@/lib/types';

export default function StatesPage() {
  const setError = useAppStore((state) => state.setError);
  const map = useProvinceMap('states');

  const [picked, setPicked] = React.useState<ProvincePick | null>(null);
  const [marker, setMarker] = React.useState<MapPickPoint | null>(null);
  const [stateFile, setStateFile] = React.useState<StateFile | null>(null);
  const [selectedId, setSelectedId] = React.useState<string | null>(null);
  const [draft, setDraft] = React.useState<StateUpdate | null>(null);
  const [isDirty, setIsDirty] = React.useState(false);
  const [isLoading, setIsLoading] = React.useState(false);
  const [isSaving, setIsSaving] = React.useState(false);
  // Counts open requests so a slower read can tell it has been superseded.
  const openRequest = React.useRef(0);

  useUnsavedIn('state editor', isDirty);

  const states = stateFile?.states ?? [];
  const selected = states.find((state) => state.id === selectedId) ?? null;
  // A mod need not override every state; the rest come from the game install,
  // which this tool has no business writing to.
  const isEditable = picked?.editable ?? true;

  async function onPick(point: MapPickPoint) {
    if (isDirty && !(await confirmDiscard('Discard unsaved state changes?'))) return;

    const request = ++openRequest.current;
    setMarker(point);
    setIsLoading(true);
    try {
      const hit = await map.pick(point.x, point.y);
      if (openRequest.current !== request) return;
      setPicked(hit);

      if (!hit?.statePath) {
        setStateFile(null);
        setSelectedId(null);
        setDraft(null);
        return;
      }

      const loaded = await api.readStateFile(hit.statePath);
      if (openRequest.current !== request) return;
      applyFile(loaded, hit.stateId || (loaded.states[0]?.id ?? null));
    } catch (error) {
      if (openRequest.current !== request) return;
      setError(describeError(error));
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

  function patch(changes: Partial<StateUpdate>) {
    setDraft((current) => (current ? { ...current, ...changes } : current));
    setIsDirty(true);
  }

  async function save() {
    if (!stateFile || !draft || !selectedId || !isEditable) return;

    setIsSaving(true);
    try {
      const updated = await api.updateState(stateFile.path, selectedId, draft);
      applyFile(updated, draft.id || selectedId);
      // Owner, id and provinces all change what the map draws and what a click
      // resolves to, so the render has to be thrown away with the cache.
      map.reload();
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
    <div className='grid h-full grid-cols-[minmax(0,1fr)_24rem] gap-3 p-3'>
      <Panel>
        <PanelHeader
          title='Province map'
          subtitle={
            map.summary
              ? `${map.summary.stateCount} states · ${map.summary.provinceCount} provinces${map.summary.unassignedLand > 0 ? ` · ${map.summary.unassignedLand} land provinces with no state` : ''}`
              : 'Coloured by state'
          }
        />
        <PanelBody className='overflow-hidden p-0'>
          {map.error ? (
            <EmptyState icon={<Map />} title='The map could not be loaded' description={map.error} />
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
          title={selected ? `${selected.name || 'unnamed state'} · id ${selected.id}` : 'No state selected'}
          subtitle={
            picked
              ? `province ${picked.provinceId} · ${picked.kind}${picked.terrain ? ` · ${picked.terrain}` : ''}`
              : 'Click a province on the map'
          }
          actions={
            selected ? (
              <>
                {stateFile && stateFile.unclosedBlocks > 0 ? (
                  <Badge tone='danger' title='The game tolerates this, but a brace is missing'>
                    <TriangleAlert className='size-3' />
                    Unclosed
                  </Badge>
                ) : null}
                {!isEditable ? (
                  <Badge tone='danger' title='This state is defined in the game folder, not in the mod'>
                    <TriangleAlert className='size-3' />
                    Base game
                  </Badge>
                ) : null}
                {isDirty ? <Badge tone='accent'>Unsaved</Badge> : null}
                <Button
                  variant='primary'
                  size='sm'
                  onClick={() => void save()}
                  disabled={!isDirty || isSaving || !isEditable}
                  title={isEditable ? undefined : 'The mod does not override this state'}
                >
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
              title={picked ? 'No state owns this province' : 'State editor'}
              description={
                picked
                  ? `Province ${picked.provinceId} is ${picked.kind} and no state lists it.`
                  : 'Click a province to open the state that holds it.'
              }
            />
          ) : (
            <div className='grid gap-3'>
              {!isEditable ? (
                <p className='rounded-md border border-danger/40 bg-danger/10 px-3 py-2 text-xs text-muted'>
                  This state comes from the game folder, so it cannot be saved here. Copy the file into the mod under{' '}
                  <code>history/states</code> to override it.
                </p>
              ) : null}
              <StateForm draft={draft} onChange={patch} />
            </div>
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
