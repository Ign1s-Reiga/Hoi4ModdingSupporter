'use client';

import * as React from 'react';
import { listen } from '@tauri-apps/api/event';
import { ClipboardList, FileText, Loader2, Plus, Save, Scale, Trash2, X } from 'lucide-react';
import { toast } from 'sonner';

import { BlockField } from '@/components/block-field';
import type { Reveal } from '@/components/script-editor';
import { Button } from '@/components/ui/button';
import { Dialog, DialogBody, DialogClose, DialogContent, DialogFooter } from '@/components/ui/dialog';
import { CodeInput, Field, Label } from '@/components/ui/form';
import { Badge, EmptyState, ListRow, Panel, PanelBody, PanelHeader } from '@/components/ui/panel';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { SourcePane, useStoredView, ViewToggle } from '@/components/view-toggle';
import { confirmDelete, confirmDiscard } from '@/lib/dialogs';
import { api, describeError } from '@/lib/ipc';
import { useAppStore } from '@/lib/store';
import { useLocalisedTexts, type TextMap } from '@/lib/use-localised-texts';
import { useSourceDocument } from '@/lib/use-source-document';
import { useUnsavedIn } from '@/lib/use-unsaved';
import {
  emptyIdeologyUpdate,
  toIdeologyUpdate,
  type AiBehaviour,
  type IdeologyFile,
  type IdeologyRule,
  type IdeologyUpdate,
  type ProjectFile,
} from '@/lib/types';
import { cn, isInFolder } from '@/lib/utils';

const IDEOLOGY_FOLDER = 'common/ideologies';

/** Sent by the backend when an MCP client writes a file. */
const FILE_CHANGED_EVENT = 'project://file-changed';

/** Where the panel's layout choice and the localisation language are remembered. */
const VIEW_KEY = 'hoi4ms.ideologies-view';
const LANGUAGE_KEY = 'hoi4ms.preview-language';

const AI_BEHAVIOURS: Array<{ value: AiBehaviour; label: string }> = [
  { value: 'democratic', label: 'Democratic' },
  { value: 'communist', label: 'Communist' },
  { value: 'fascist', label: 'Fascist' },
  { value: 'neutral', label: 'Neutral' },
];

/** The rules the game reads out of an ideology's `rules` block. */
const RULE_KEYS = [
  'can_force_government',
  'can_send_volunteers',
  'can_puppet',
  'can_lower_tension',
  'can_only_justify_war_on_threat_country',
  'can_guarantee_other_ideologies',
  'can_declare_war_on_same_ideology',
  'can_create_collaboration_government',
];

const FLAGS: Array<{
  key: 'canHostGovernmentInExile' | 'canBeBoosted' | 'canCollaborate';
  label: string;
  hint: string;
}> = [
  { key: 'canHostGovernmentInExile', label: 'Government in exile', hint: 'Can host one' },
  { key: 'canBeBoosted', label: 'Can be boosted', hint: 'By the boost ideology action' },
  { key: 'canCollaborate', label: 'Can collaborate', hint: 'Collaboration governments' },
];

function stored(key: string, fallback: string): string {
  try {
    return window.localStorage.getItem(key) ?? fallback;
  } catch {
    return fallback;
  }
}

function remember(key: string, value: string) {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // A private window forgets; the choice still holds for this session.
  }
}

/**
 * The CSS colour a `color = { }` body stands for: three numbers, 0-255 as
 * the game's files write them or 0-1 as some mods do. Anything else is no
 * colour, and the swatch shows nothing.
 */
function cssColour(body: string): string | null {
  const parts = body.trim().split(/\s+/);
  if (parts.length !== 3) return null;
  const numbers = parts.map(Number);
  if (numbers.some((number) => !Number.isFinite(number) || number < 0)) return null;
  const fraction = parts.some((part) => part.includes('.')) && numbers.every((number) => number <= 1);
  const [r, g, b] = numbers.map((number) => Math.round(fraction ? number * 255 : Math.min(number, 255)));
  return `rgb(${r} ${g} ${b})`;
}

function hexColour(body: string): string {
  const css = cssColour(body);
  const match = css ? /rgb\((\d+) (\d+) (\d+)\)/.exec(css) : null;
  if (!match) return '#808080';
  return (
    '#' +
    match
      .slice(1, 4)
      .map((part) => Number(part).toString(16).padStart(2, '0'))
      .join('')
  );
}

function bodyFromHex(hex: string): string {
  const number = Number.parseInt(hex.replace('#', ''), 16);
  return `${(number >> 16) & 255} ${(number >> 8) & 255} ${number & 255}`;
}

/** The localisation keys the game looks up for an ideology and its sub-ideologies. */
function localisationKeys(ideology: IdeologyUpdate): Array<{ key: string; role: string }> {
  const id = ideology.id.trim();
  const keys = id
    ? [
        { key: id, role: 'name' },
        { key: `${id}_noun`, role: 'noun' },
        { key: `${id}_desc`, role: 'description' },
      ]
    : [];
  for (const sub of ideology.types) {
    const subId = sub.id.trim();
    if (!subId) continue;
    keys.push({ key: subId, role: 'sub-ideology name' }, { key: `${subId}_desc`, role: 'sub-ideology description' });
  }
  return keys;
}

/**
 * The ideology editor: a file's ideology groups with their sub-ideologies,
 * colour, rules and modifiers, edited in a form or in the file's text, with
 * a check of the localisation keys each one needs.
 */
export default function IdeologiesPage() {
  const { project, scan, setError } = useAppStore();

  const [selectedFile, setSelectedFile] = React.useState<ProjectFile | null>(null);
  const [file, setFile] = React.useState<IdeologyFile | null>(null);
  const [selectedId, setSelectedId] = React.useState<string | null>(null);
  const [draft, setDraft] = React.useState<IdeologyUpdate | null>(null);
  const [isDirty, setIsDirty] = React.useState(false);
  const [isLoading, setIsLoading] = React.useState(false);
  const [isSaving, setIsSaving] = React.useState(false);
  const [isAdding, setIsAdding] = React.useState(false);
  // Counts open requests so a slower read can tell it has been superseded.
  const openRequest = React.useRef(0);

  const [view, chooseView] = useStoredView(VIEW_KEY);
  const [reveal, setReveal] = React.useState<Reveal | undefined>(undefined);
  const [language, setLanguage] = React.useState(() => stored(LANGUAGE_KEY, 'english'));
  const [languages, setLanguages] = React.useState<string[]>([]);

  const files = React.useMemo(
    () =>
      (scan?.files ?? []).filter(
        (entry) => isInFolder(entry.relativePath, IDEOLOGY_FOLDER) && entry.extension === 'txt',
      ),
    [scan],
  );

  const ideologies = React.useMemo(() => file?.ideologies ?? [], [file]);
  const selected = ideologies.find((ideology) => ideology.id === selectedId) ?? null;

  const latest = React.useRef({ isDirty, selectedId });
  React.useEffect(() => {
    latest.current = { isDirty, selectedId };
  });

  const openPath = selectedFile?.fullPath;
  const doc = useSourceDocument<IdeologyFile>({
    path: openPath,
    parse: api.parseIdeologySource,
    onParsed: (parsed) => {
      const current = latest.current.selectedId;
      const stillThere = current !== null && parsed.ideologies.some((ideology) => ideology.id === current);
      const nextId = stillThere ? current : (parsed.ideologies[0]?.id ?? null);

      setFile(parsed);
      setSelectedId(nextId);
      // A form the user is mid-way through editing keeps its draft; an
      // untouched one follows the new text.
      if (!latest.current.isDirty) {
        const ideology = parsed.ideologies.find((entry) => entry.id === nextId);
        setDraft(ideology ? toIdeologyUpdate(ideology) : null);
      }
    },
  });
  const { source, setSource, isDirty: sourceDirty, parseError } = doc;

  // With the code hidden and the text untouched, the form's Save writes the
  // file at once. Once the text has edits of its own the form can only
  // apply: writing would take those along uninvited.
  const formCommits = view === 'visual' && !sourceDirty;

  useUnsavedIn('ideology editor', isDirty || sourceDirty);

  // The list shows each ideology's name; the form checks every key of the
  // selected one.
  const wantedKeys = React.useMemo(() => {
    const keys = ideologies.map((ideology) => ideology.id);
    if (draft) keys.push(...localisationKeys(draft).map((entry) => entry.key));
    return keys;
  }, [draft, ideologies]);
  const texts = useLocalisedTexts(project?.folderPath, language, wantedKeys);

  const folder = project?.folderPath;
  React.useEffect(() => {
    if (!folder) return;

    let cancelled = false;
    api
      .listLocalisationFiles(folder)
      .then((entries) => {
        if (cancelled) return;
        const found = new Set(entries.map((entry) => entry.language.replace(/^l_/i, '')).filter(Boolean));
        setLanguages([...found].sort());
      })
      .catch(() => {
        // The picker then offers the language already chosen, nothing more.
      });

    return () => {
      cancelled = true;
    };
  }, [folder]);

  function chooseLanguage(next: string) {
    setLanguage(next);
    remember(LANGUAGE_KEY, next);
  }

  const { adopt: adoptFile, show } = doc;

  /** Shows a file the backend handed back, selecting `ideologyId`. */
  const applyFile = React.useCallback(
    (loaded: IdeologyFile, ideologyId: string | null) => {
      setFile(loaded);
      show(loaded);
      setSelectedId(ideologyId);
      const ideology = loaded.ideologies.find((entry) => entry.id === ideologyId);
      setDraft(ideology ? toIdeologyUpdate(ideology) : null);
      setIsDirty(false);
    },
    [show],
  );

  async function confirmLeavingEdits(): Promise<boolean> {
    if (isDirty && !(await confirmDiscard('Discard unsaved ideology changes?'))) return false;
    if (sourceDirty && !(await confirmDiscard('Discard unsaved changes to the file text?'))) return false;
    return true;
  }

  async function openFile(entry: ProjectFile) {
    if (!(await confirmLeavingEdits())) return;

    const request = ++openRequest.current;
    setSelectedFile(entry);
    setIsLoading(true);
    try {
      const loaded = await api.readIdeologyFile(entry.fullPath);
      if (openRequest.current !== request) return;

      adoptFile(loaded);
      applyFile(loaded, loaded.ideologies[0]?.id ?? null);
    } catch (error) {
      if (openRequest.current !== request) return;

      setError(describeError(error));
      setFile(null);
      setSelectedId(null);
      setDraft(null);
    } finally {
      if (openRequest.current === request) setIsLoading(false);
    }
  }

  // An assistant editing this file through MCP would otherwise leave the
  // list showing what the file used to say. Unsaved edits win: the reload
  // waits, and a toast says why.
  React.useEffect(() => {
    if (!openPath) return;

    let cancelled = false;
    const subscription = listen<{ path: string }>(FILE_CHANGED_EVENT, (event) => {
      if (cancelled || !samePath(event.payload.path, openPath)) return;

      if (isDirty || sourceDirty) {
        toast.message('This file was changed by an MCP client', {
          description: 'Your unsaved edits are kept; reopen the file to see the new version.',
        });
        return;
      }

      const request = ++openRequest.current;
      void api.readIdeologyFile(openPath).then((loaded) => {
        if (cancelled || openRequest.current !== request) return;
        adoptFile(loaded);
        applyFile(
          loaded,
          loaded.ideologies.some((entry) => entry.id === selectedId) ? selectedId : (loaded.ideologies[0]?.id ?? null),
        );
        toast.message('Reloaded: changed by an MCP client');
      });
    });

    return () => {
      cancelled = true;
      void subscription.then((unlisten) => unlisten());
    };
  }, [adoptFile, applyFile, openPath, isDirty, sourceDirty, selectedId]);

  async function selectIdeology(id: string) {
    if (id === selectedId) return;
    if (isDirty && !(await confirmDiscard('Discard unsaved ideology changes?'))) return;

    const ideology = ideologies.find((entry) => entry.id === id);
    setSelectedId(id);
    setDraft(ideology ? toIdeologyUpdate(ideology) : null);
    setIsDirty(false);
    // The code view jumps to the ideology, so the two stay in step.
    if (ideology) setReveal((current) => ({ line: ideology.line, key: (current?.key ?? 0) + 1 }));
  }

  function patch(changes: Partial<IdeologyUpdate>) {
    setDraft((current) => (current ? { ...current, ...changes } : current));
    setIsDirty(true);
  }

  /** Writes `text` as the file, reporting an encoding the text no longer fits. */
  async function writeFile(text: string) {
    if (!selectedFile) return;

    const used = await doc.write(text);
    if (used !== doc.encoding) {
      toast.warning(`Saved ${selectedFile.name} as UTF-8 - the new text does not fit Windows-1252`);
    }
  }

  /** The header's Save file: the whole buffer, edits from every source included. */
  async function saveFile() {
    if (!selectedFile || !sourceDirty) return;

    setIsSaving(true);
    try {
      await writeFile(source);
      toast.success(`Saved ${selectedFile.name}`);
    } catch (error) {
      const message = describeError(error);
      setError(message);
      toast.error(message);
    } finally {
      setIsSaving(false);
    }
  }

  /** Runs a buffer operation and, when the form commits, writes the result. */
  async function commit(
    operation: () => Promise<IdeologyFile>,
    ideologyId: (updated: IdeologyFile) => string | null,
    done: string,
  ) {
    if (!selectedFile) return;

    setIsSaving(true);
    try {
      const updated = await operation();
      const persisted = formCommits;
      if (persisted) await writeFile(updated.source);
      applyFile(updated, ideologyId(updated));
      toast.success(persisted ? `Saved ${done}` : `Applied ${done}`);
    } catch (error) {
      const message = describeError(error);
      setError(message);
      toast.error(message);
    } finally {
      setIsSaving(false);
    }
  }

  async function save() {
    if (!selectedFile || !draft || !selectedId) return;
    const path = selectedFile.fullPath;
    const id = selectedId;
    const change = draft;

    await commit(
      () => api.updateIdeologySource(path, source, doc.hasBom, doc.encoding, id, change),
      // The id itself may have changed, so follow the draft rather than the old id.
      () => change.id.trim() || id,
      change.id.trim() || id,
    );
  }

  async function remove() {
    if (!selectedFile || !selectedId) return;
    if (!(await confirmDelete(`Delete ideology "${selectedId}" from ${selectedFile.name}?`))) {
      return;
    }
    const path = selectedFile.fullPath;
    const id = selectedId;

    await commit(
      () => api.deleteIdeologySource(path, source, doc.hasBom, doc.encoding, id),
      (updated) => updated.ideologies[0]?.id ?? null,
      id,
    );
  }

  async function addIdeology(ideology: IdeologyUpdate) {
    if (!selectedFile) return;
    const path = selectedFile.fullPath;

    await commit(
      () => api.addIdeologySource(path, source, doc.hasBom, doc.encoding, ideology),
      () => ideology.id,
      ideology.id,
    );
  }

  const form =
    !draft || !selected ? (
      <EmptyState
        icon={<Scale />}
        title='Select an ideology'
        description='Pick one from the list to edit its colour, sub-ideologies, rules and modifiers.'
      />
    ) : (
      <div className='flex h-full min-h-0 flex-col'>
        <div className='flex shrink-0 items-center gap-2 border-b border-border px-3 py-2'>
          <span
            className='size-3.5 shrink-0 rounded-sm border border-border'
            style={{ background: cssColour(draft.color) ?? 'transparent' }}
          />
          <span className='truncate font-mono text-sm'>{selected.id}</span>
          <span className='text-xs text-muted'>line {selected.line}</span>
          <div className='ml-auto flex items-center gap-1.5'>
            {isDirty ? <Badge tone='accent'>Unsaved</Badge> : null}
            <Button variant='ghost' size='icon-sm' title='Delete ideology' onClick={() => void remove()}>
              <Trash2 />
            </Button>
            <Button
              variant='primary'
              size='sm'
              onClick={() => void save()}
              disabled={!isDirty || isSaving}
              title={
                formCommits
                  ? 'Write this ideology to the file'
                  : 'Put this ideology into the file text; Save file writes it'
              }
            >
              {isSaving ? <Loader2 className='animate-spin' /> : <Save />}
              {formCommits ? 'Save' : 'Apply'}
            </Button>
          </div>
        </div>
        <div className='min-h-0 flex-1 overflow-auto p-3'>
          <IdeologyForm
            draft={draft}
            texts={texts}
            language={language}
            languages={languages}
            onLanguage={chooseLanguage}
            onChange={patch}
          />
        </div>
      </div>
    );

  return (
    <div className='grid h-full grid-cols-[16rem_minmax(0,1fr)] gap-3 p-3'>
      <div className='grid min-h-0 grid-rows-[2fr_3fr] gap-3'>
        <Panel>
          <PanelHeader title='Ideology files' subtitle={`${files.length} in ${IDEOLOGY_FOLDER}`} />
          <PanelBody>
            {files.length === 0 ? (
              <EmptyState
                title='No ideology files'
                description={`This mod has no .txt files under ${IDEOLOGY_FOLDER}; the game's own four apply.`}
              />
            ) : (
              <ul className='py-1'>
                {files.map((entry) => (
                  <li key={entry.fullPath}>
                    <ListRow
                      active={selectedFile?.fullPath === entry.fullPath}
                      onClick={() => void openFile(entry)}
                      title={entry.relativePath}
                    >
                      <FileText className='size-3.5 shrink-0 opacity-70' />
                      <span className='truncate'>{entry.name}</span>
                    </ListRow>
                  </li>
                ))}
              </ul>
            )}
          </PanelBody>
        </Panel>

        <Panel>
          <PanelHeader
            title='Ideologies'
            subtitle={file ? `${ideologies.length} in this file` : 'No file open'}
            actions={
              file ? (
                <Button variant='ghost' size='icon-sm' title='Add ideology' onClick={() => setIsAdding(true)}>
                  <Plus />
                </Button>
              ) : null
            }
          />
          <PanelBody>
            {ideologies.length === 0 ? (
              <EmptyState title='Nothing to show' description='Open an ideology file first.' />
            ) : (
              <ul className='py-1'>
                {ideologies.map((ideology) => {
                  const name = texts.get(ideology.id);
                  return (
                    <li key={`${ideology.id}-${ideology.line}`}>
                      <ListRow
                        active={ideology.id === selectedId}
                        onClick={() => void selectIdeology(ideology.id)}
                        title={`line ${ideology.line}`}
                        className='py-1'
                      >
                        <span
                          className='size-3.5 shrink-0 rounded-sm border border-border'
                          style={{ background: cssColour(ideology.color) ?? 'transparent' }}
                        />
                        <span className='flex min-w-0 flex-col'>
                          <span className='truncate font-mono text-xs'>{ideology.id}</span>
                          <span
                            className={cn('truncate text-[0.6875rem]', name === null ? 'text-danger' : 'text-muted')}
                          >
                            {name === null
                              ? 'no name in localisation'
                              : (name ?? '…') +
                                ` · ${ideology.types.length} sub-ideolog${ideology.types.length === 1 ? 'y' : 'ies'}`}
                          </span>
                        </span>
                      </ListRow>
                    </li>
                  );
                })}
              </ul>
            )}
          </PanelBody>
        </Panel>
      </div>

      <Panel>
        <PanelHeader
          title={file ? selectedFile?.name : 'Ideologies'}
          subtitle={file ? `${ideologies.length} ideologies` : 'Open a file to edit its ideologies'}
          actions={
            <>
              {sourceDirty ? <Badge tone='accent'>Unsaved file</Badge> : null}
              {sourceDirty ? (
                <Button variant='primary' size='sm' onClick={() => void saveFile()} disabled={isSaving} title='Ctrl+S'>
                  {isSaving ? <Loader2 className='animate-spin' /> : <Save />}
                  Save file
                </Button>
              ) : null}
              <ViewToggle value={view} onChange={chooseView} visual={{ label: 'Form', icon: ClipboardList }} />
            </>
          }
        />
        <PanelBody className='overflow-hidden p-0'>
          {isLoading ? (
            <div className='flex h-full items-center justify-center gap-2 text-sm text-muted'>
              <Loader2 className='size-4 animate-spin' />
              Reading ideologies…
            </div>
          ) : !file || !selectedFile ? (
            <EmptyState
              icon={<Scale />}
              title='No ideology file open'
              description='Pick a file from common/ideologies to see the groups it defines.'
            />
          ) : (
            <div className={cn('grid h-full', view === 'split' ? 'grid-cols-2' : 'grid-cols-1')}>
              {view !== 'code' ? (
                <div className={cn('min-h-0', view === 'split' && 'border-r border-border')}>{form}</div>
              ) : null}
              {view !== 'visual' ? (
                <SourcePane
                  source={source}
                  parseError={parseError}
                  reveal={reveal}
                  onChange={setSource}
                  onSave={() => void saveFile()}
                />
              ) : null}
            </div>
          )}
        </PanelBody>
      </Panel>

      {/* Mounted only while open so its fields start fresh every time. */}
      {isAdding && file ? (
        <AddIdeologyDialog
          onOpenChange={setIsAdding}
          onAdd={async (ideology) => {
            await addIdeology(ideology);
            setIsAdding(false);
          }}
        />
      ) : null}
    </div>
  );
}

/** Paths from the backend use forward slashes; the scan's may not. */
function samePath(left: string, right: string): boolean {
  return left.replaceAll('\\', '/').toLowerCase() === right.replaceAll('\\', '/').toLowerCase();
}

function IdeologyForm({
  draft,
  texts,
  language,
  languages,
  onLanguage,
  onChange,
}: {
  draft: IdeologyUpdate;
  texts: TextMap;
  language: string;
  languages: string[];
  onLanguage: (language: string) => void;
  onChange: (changes: Partial<IdeologyUpdate>) => void;
}) {
  const ruleListId = React.useId();

  return (
    <div className='@container flex flex-col gap-4'>
      <div className='grid gap-3 @lg:grid-cols-[1fr_14rem]'>
        <Field label='Id' hint='The key of the block; what a country sets as its ruling party'>
          <CodeInput value={draft.id} onChange={(event) => onChange({ id: event.target.value })} />
        </Field>
        <Field label='Colour' hint='Three numbers, 0-255'>
          <div className='flex items-center gap-2'>
            <input
              type='color'
              value={hexColour(draft.color)}
              onChange={(event) => onChange({ color: bodyFromHex(event.target.value) })}
              className='h-9 w-11 shrink-0 cursor-pointer rounded-md border border-border bg-surface p-0.5'
              title='Pick a colour'
            />
            <CodeInput
              value={draft.color}
              placeholder='0 0 255'
              onChange={(event) => onChange({ color: event.target.value })}
            />
          </div>
        </Field>
      </div>

      <KeyedRows
        title='Sub-ideologies'
        addLabel='Add sub-ideology'
        hint='The ideologies a leader or party can hold; the game needs at least one.'
        rows={draft.types}
        onChange={(types) => onChange({ types })}
        create={() => ({ id: '', canBeRandomlySelected: '' })}
        render={(sub, update) => (
          <>
            <CodeInput
              value={sub.id}
              placeholder='conservatism'
              onChange={(event) => update({ id: event.target.value })}
            />
            <Select
              value={sub.canBeRandomlySelected || 'unset'}
              onValueChange={(value) => update({ canBeRandomlySelected: value === 'unset' ? '' : value })}
            >
              <SelectTrigger className='w-40 shrink-0' title='Whether a random leader can get it'>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value='unset'>Random: default</SelectItem>
                <SelectItem value='yes'>Random: yes</SelectItem>
                <SelectItem value='no'>Random: no</SelectItem>
              </SelectContent>
            </Select>
          </>
        )}
      />

      <KeyedRows
        title='Faction names'
        addLabel='Add name'
        hint='Localisation keys the game picks a faction name from.'
        rows={draft.dynamicFactionNames.map((name) => ({ name }))}
        onChange={(rows) => onChange({ dynamicFactionNames: rows.map((row) => row.name) })}
        create={() => ({
          name: draft.id.trim()
            ? `FACTION_NAME_${draft.id.trim().toUpperCase()}_${draft.dynamicFactionNames.length + 1}`
            : '',
        })}
        render={(row, update) => (
          <CodeInput
            value={row.name}
            placeholder='FACTION_NAME_X_1'
            onChange={(event) => update({ name: event.target.value })}
          />
        )}
      />

      <div className='grid gap-3 @md:grid-cols-2 @2xl:grid-cols-4'>
        <Field label='War tension' hint='Impact of its wars on world tension'>
          <CodeInput
            value={draft.warImpactOnWorldTension}
            placeholder='0.5'
            onChange={(event) => onChange({ warImpactOnWorldTension: event.target.value })}
          />
        </Field>
        <Field label='Faction tension' hint='Impact of its factions'>
          <CodeInput
            value={draft.factionImpactOnWorldTension}
            placeholder='0.5'
            onChange={(event) => onChange({ factionImpactOnWorldTension: event.target.value })}
          />
        </Field>
        <Field label='AI behaviour' hint='Which of the game AIs plays it'>
          <Select
            value={draft.aiBehaviour || 'unset'}
            onValueChange={(value) => onChange({ aiBehaviour: value === 'unset' ? '' : (value as AiBehaviour) })}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value='unset'>Not set</SelectItem>
              {AI_BEHAVIOURS.map((behaviour) => (
                <SelectItem key={behaviour.value} value={behaviour.value}>
                  {behaviour.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
        <Field label='Wanted units factor' hint='ai_ideology_wanted_units_factor'>
          <CodeInput
            value={draft.aiIdeologyWantedUnitsFactor}
            placeholder='1.0'
            onChange={(event) => onChange({ aiIdeologyWantedUnitsFactor: event.target.value })}
          />
        </Field>
      </div>

      <div className='grid gap-3 @md:grid-cols-3'>
        {FLAGS.map((flag) => (
          <Field key={flag.key} label={flag.label} hint={flag.hint}>
            <Select
              value={draft[flag.key] || 'unset'}
              onValueChange={(value) => onChange({ [flag.key]: value === 'unset' ? '' : value })}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value='unset'>Not set</SelectItem>
                <SelectItem value='yes'>yes</SelectItem>
                <SelectItem value='no'>no</SelectItem>
              </SelectContent>
            </Select>
          </Field>
        ))}
      </div>

      <KeyedRows
        title='Rules'
        addLabel='Add rule'
        hint='What a country of this ideology may do. A rule left out falls back to the game default.'
        rows={draft.rules}
        onChange={(rules) => onChange({ rules })}
        create={() => ({
          key: RULE_KEYS.find((key) => !draft.rules.some((rule) => rule.key === key)) ?? '',
          value: 'no',
        })}
        render={(rule: IdeologyRule, update) => (
          <>
            <CodeInput
              list={ruleListId}
              value={rule.key}
              placeholder='can_puppet'
              onChange={(event) => update({ key: event.target.value })}
            />
            <Select value={rule.value || 'no'} onValueChange={(value) => update({ value })}>
              <SelectTrigger className='w-24 shrink-0'>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value='yes'>yes</SelectItem>
                <SelectItem value='no'>no</SelectItem>
              </SelectContent>
            </Select>
          </>
        )}
      />
      <datalist id={ruleListId}>
        {RULE_KEYS.map((key) => (
          <option key={key} value={key} />
        ))}
      </datalist>

      <BlockField
        label='Modifiers'
        value={draft.modifiers}
        onChange={(modifiers) => onChange({ modifiers })}
        placeholder='generate_wargoal_tension = 0.5'
      />
      <BlockField
        label='Faction modifiers'
        value={draft.factionModifiers}
        onChange={(factionModifiers) => onChange({ factionModifiers })}
        placeholder='faction_trade_opinion_factor = 0.5'
      />

      <LocalisationCheck
        draft={draft}
        texts={texts}
        language={language}
        languages={languages}
        onLanguage={onLanguage}
      />
    </div>
  );
}

/** A list of rows keyed by what the user types, one card-less line each. */
function KeyedRows<T>({
  title,
  addLabel,
  hint,
  rows,
  onChange,
  create,
  render,
}: {
  title: string;
  addLabel: string;
  hint: string;
  rows: T[];
  onChange: (rows: T[]) => void;
  create: () => T;
  render: (row: T, update: (changes: Partial<T>) => void) => React.ReactNode;
}) {
  return (
    <div className='space-y-2 rounded-lg border border-border p-3'>
      <div className='flex items-center justify-between'>
        <Label>
          {title}
          {rows.length > 0 ? ` · ${rows.length}` : ''}
        </Label>
        <Button size='sm' onClick={() => onChange([...rows, create()])}>
          <Plus />
          {addLabel}
        </Button>
      </div>
      {rows.length === 0 ? <p className='text-xs text-muted'>None.</p> : null}
      <div className='flex flex-col gap-1.5'>
        {rows.map((row, index) => (
          <div key={index} className='flex items-center gap-1.5'>
            {render(row, (changes) => {
              const next = [...rows];
              next[index] = { ...row, ...changes };
              onChange(next);
            })}
            <Button
              variant='ghost'
              size='icon-sm'
              title='Remove'
              className='shrink-0'
              onClick={() => onChange(rows.filter((_, at) => at !== index))}
            >
              <X />
            </Button>
          </div>
        ))}
      </div>
      <p className='text-xs text-muted'>{hint}</p>
    </div>
  );
}

/**
 * The localisation keys the game looks up for this ideology, with the text
 * each resolves to in the chosen language, or a mark where there is none:
 * an ideology without them shows as its raw id in the game.
 */
function LocalisationCheck({
  draft,
  texts,
  language,
  languages,
  onLanguage,
}: {
  draft: IdeologyUpdate;
  texts: TextMap;
  language: string;
  languages: string[];
  onLanguage: (language: string) => void;
}) {
  const keys = localisationKeys(draft);
  const missing = keys.filter((entry) => texts.get(entry.key) === null).length;

  return (
    <div className='space-y-2 rounded-lg border border-border p-3'>
      <div className='flex items-center justify-between gap-2'>
        <Label>
          Localisation
          {keys.length > 0 ? (
            <Badge tone={missing ? 'danger' : 'neutral'} className='ml-2'>
              {missing ? `${missing} missing` : 'complete'}
            </Badge>
          ) : null}
        </Label>
        <Select value={language} onValueChange={onLanguage}>
          <SelectTrigger className='h-7 w-32 text-xs'>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {[...new Set([language, ...languages])].map((entry) => (
              <SelectItem key={entry} value={entry}>
                {entry}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      {keys.length === 0 ? (
        <p className='text-xs text-muted'>Give the ideology an id to see which keys it needs.</p>
      ) : null}
      <dl className='grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs'>
        {keys.map((entry) => {
          const text = texts.get(entry.key);
          return (
            <React.Fragment key={entry.key}>
              <dt className='truncate font-mono text-muted' title={entry.role}>
                {entry.key}
              </dt>
              <dd className={cn('truncate', text === null ? 'text-danger' : text === undefined ? 'text-muted' : '')}>
                {text === undefined ? '…' : text === null ? 'missing' : text}
              </dd>
            </React.Fragment>
          );
        })}
      </dl>
      <p className='text-xs text-muted'>Keys are looked up in the mod's files and the game's, in this language.</p>
    </div>
  );
}

function AddIdeologyDialog({
  onOpenChange,
  onAdd,
}: {
  onOpenChange: (open: boolean) => void;
  /** Adds the ideology; the page decides whether that also writes the file. */
  onAdd: (ideology: IdeologyUpdate) => Promise<void>;
}) {
  const [id, setId] = React.useState('');
  const [isSaving, setIsSaving] = React.useState(false);

  async function create() {
    const trimmed = id.trim();
    if (!trimmed) return;

    setIsSaving(true);
    try {
      // A stub the game will load: a colour, one sub-ideology named after
      // the group, and the rules a cautious ideology gets.
      await onAdd({
        ...emptyIdeologyUpdate(),
        id: trimmed,
        color: '128 128 128',
        types: [{ id: `${trimmed}_ideology`, canBeRandomlySelected: '' }],
        dynamicFactionNames: [`FACTION_NAME_${trimmed.toUpperCase()}_1`],
        warImpactOnWorldTension: '0.5',
        factionImpactOnWorldTension: '0.5',
        rules: RULE_KEYS.map((key) => ({ key, value: 'no' })),
      });
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <Dialog open onOpenChange={onOpenChange}>
      <DialogContent
        title='Add an ideology'
        description='A grey stub with one sub-ideology and every rule set to no is appended; its name, noun and description keys still need localisation entries.'
      >
        <DialogBody className='flex flex-col gap-3'>
          <Field label='Ideology id' hint='Letters, digits and underscores, e.g. monarchism'>
            <CodeInput value={id} autoFocus placeholder='monarchism' onChange={(event) => setId(event.target.value)} />
          </Field>
        </DialogBody>
        <DialogFooter>
          <DialogClose asChild>
            <Button>Cancel</Button>
          </DialogClose>
          <Button variant='primary' onClick={() => void create()} disabled={!id.trim() || isSaving}>
            {isSaving ? <Loader2 className='animate-spin' /> : <Plus />}
            Add ideology
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
