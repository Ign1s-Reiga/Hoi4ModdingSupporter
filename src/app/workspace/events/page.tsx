'use client';

import * as React from 'react';
import { listen } from '@tauri-apps/api/event';
import { CalendarClock, ClipboardList, FileText, ImageOff, Loader2, Plus, Save, Trash2, X } from 'lucide-react';
import { toast } from 'sonner';

import { BlockField } from '@/components/block-field';
import type { Reveal } from '@/components/script-editor';
import { Button } from '@/components/ui/button';
import { Dialog, DialogBody, DialogClose, DialogContent, DialogFooter } from '@/components/ui/dialog';
import { CodeInput, Field, Label, Textarea } from '@/components/ui/form';
import { Badge, EmptyState, ListRow, Panel, PanelBody, PanelHeader } from '@/components/ui/panel';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { SourcePane, useStoredView, ViewToggle } from '@/components/view-toggle';
import { confirmDelete, confirmDiscard } from '@/lib/dialogs';
import { api, describeError } from '@/lib/ipc';
import { useAppStore } from '@/lib/store';
import { useSourceDocument } from '@/lib/use-source-document';
import { useSpriteIcons } from '@/lib/use-sprite-icons';
import { useUnsavedIn } from '@/lib/use-unsaved';
import {
  emptyEventUpdate,
  toEventUpdate,
  type EventFile,
  type EventKind,
  type EventOption,
  type EventUpdate,
  type GameEvent,
  type ProjectFile,
  type SpriteIcon,
} from '@/lib/types';
import { cn, isInFolder } from '@/lib/utils';

const EVENT_FOLDER = 'events';

/** Sent by the backend when an MCP client writes a file. */
const FILE_CHANGED_EVENT = 'project://file-changed';

/** Where the panel's layout choice is remembered. */
const VIEW_KEY = 'hoi4ms.events-view';

const EVENT_KINDS: Array<{ value: EventKind; label: string }> = [
  { value: 'country_event', label: 'Country' },
  { value: 'news_event', label: 'News' },
  { value: 'state_event', label: 'State' },
  { value: 'unit_leader_event', label: 'Unit leader' },
  { value: 'operative_leader_event', label: 'Operative' },
];

const FLAGS: Array<{ key: 'fireOnlyOnce' | 'isTriggeredOnly' | 'hidden' | 'major'; label: string; hint: string }> = [
  { key: 'isTriggeredOnly', label: 'Triggered only', hint: 'Fired by script, never by its trigger' },
  { key: 'fireOnlyOnce', label: 'Fire only once', hint: 'Per country' },
  { key: 'hidden', label: 'Hidden', hint: 'No window; effects still happen' },
  { key: 'major', label: 'Major', hint: 'Shown to everyone who can see it' },
];

function kindLabel(kind: string): string {
  return EVENT_KINDS.find((entry) => entry.value === kind)?.label ?? kind;
}

/** `a`, `b`, `c` …: how the game's files name a second title or description. */
function letter(index: number): string {
  return String.fromCharCode(97 + Math.min(index, 25));
}

/** Whether a title or description is a triggered block rather than a key. */
function isScriptedText(value: string): boolean {
  return /[{=\s]/.test(value.trim());
}

/**
 * `<namespace>.<n>` one past the highest number the file uses in that
 * namespace, the way modders number events by hand.
 */
function nextEventId(events: GameEvent[], namespace: string): string {
  if (!namespace) return '';
  const prefix = `${namespace}.`;
  const highest = events
    .filter((event) => event.id.startsWith(prefix))
    .map((event) => Number.parseInt(event.id.slice(prefix.length), 10))
    .filter((number) => Number.isFinite(number))
    .reduce((max, number) => Math.max(max, number), 0);
  return `${prefix}${highest + 1}`;
}

/**
 * The event editor: a file's events listed by id and title, the selected one
 * edited in a form or in the file's text, like the focus editor.
 */
export default function EventsPage() {
  const { project, scan, setError } = useAppStore();

  const [selectedFile, setSelectedFile] = React.useState<ProjectFile | null>(null);
  const [file, setFile] = React.useState<EventFile | null>(null);
  const [selectedId, setSelectedId] = React.useState<string | null>(null);
  const [draft, setDraft] = React.useState<EventUpdate | null>(null);
  const [isDirty, setIsDirty] = React.useState(false);
  const [isLoading, setIsLoading] = React.useState(false);
  const [isSaving, setIsSaving] = React.useState(false);
  const [isAdding, setIsAdding] = React.useState(false);
  // Counts open requests so a slower read can tell it has been superseded.
  const openRequest = React.useRef(0);

  const [view, chooseView] = useStoredView(VIEW_KEY);
  const [reveal, setReveal] = React.useState<Reveal | undefined>(undefined);

  const files = React.useMemo(
    () =>
      (scan?.files ?? []).filter((entry) => isInFolder(entry.relativePath, EVENT_FOLDER) && entry.extension === 'txt'),
    [scan],
  );

  const events = React.useMemo(() => file?.events ?? [], [file]);
  const selected = events.find((event) => event.id === selectedId) ?? null;
  // An id two events share: the game would trip over it, and so would an
  // edit by id, so the list says so and the form stands back.
  const duplicates = React.useMemo(() => {
    const seen = new Map<string, number>();
    for (const event of events) seen.set(event.id, (seen.get(event.id) ?? 0) + 1);
    return new Set([...seen].filter(([, count]) => count > 1).map(([id]) => id));
  }, [events]);

  const latest = React.useRef({ isDirty, selectedId });
  React.useEffect(() => {
    latest.current = { isDirty, selectedId };
  });

  const openPath = selectedFile?.fullPath;
  const doc = useSourceDocument<EventFile>({
    path: openPath,
    parse: api.parseEventSource,
    onParsed: (parsed) => {
      const current = latest.current.selectedId;
      const stillThere = current !== null && parsed.events.some((event) => event.id === current);
      const nextId = stillThere ? current : (parsed.events[0]?.id ?? null);

      setFile(parsed);
      setSelectedId(nextId);
      // A form the user is mid-way through editing keeps its draft; an
      // untouched one follows the new text.
      if (!latest.current.isDirty) {
        const event = parsed.events.find((entry) => entry.id === nextId);
        setDraft(event ? toEventUpdate(event) : null);
      }
    },
  });
  const { source, setSource, isDirty: sourceDirty, parseError } = doc;

  // With the code hidden and the text untouched, the form's Save writes the
  // file at once. Once the text has edits of its own the form can only
  // apply: writing would take those along uninvited.
  const formCommits = view === 'visual' && !sourceDirty;

  useUnsavedIn('event editor', isDirty || sourceDirty);

  const icons = useSpriteIcons(project?.folderPath, draft ? [draft.picture] : []);

  const { adopt: adoptFile, show } = doc;

  /** Shows a file the backend handed back, selecting `eventId`. */
  const applyFile = React.useCallback(
    (loaded: EventFile, eventId: string | null) => {
      setFile(loaded);
      show(loaded);
      setSelectedId(eventId);
      const event = loaded.events.find((entry) => entry.id === eventId);
      setDraft(event ? toEventUpdate(event) : null);
      setIsDirty(false);
    },
    [show],
  );

  async function confirmLeavingEdits(): Promise<boolean> {
    if (isDirty && !(await confirmDiscard('Discard unsaved event changes?'))) return false;
    if (sourceDirty && !(await confirmDiscard('Discard unsaved changes to the file text?'))) return false;
    return true;
  }

  async function openFile(entry: ProjectFile) {
    if (!(await confirmLeavingEdits())) return;

    const request = ++openRequest.current;
    setSelectedFile(entry);
    setIsLoading(true);
    try {
      const loaded = await api.readEventFile(entry.fullPath);
      if (openRequest.current !== request) return;

      adoptFile(loaded);
      applyFile(loaded, loaded.events[0]?.id ?? null);
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
      void api.readEventFile(openPath).then((loaded) => {
        if (cancelled || openRequest.current !== request) return;
        adoptFile(loaded);
        applyFile(
          loaded,
          loaded.events.some((entry) => entry.id === selectedId) ? selectedId : (loaded.events[0]?.id ?? null),
        );
        toast.message('Reloaded: changed by an MCP client');
      });
    });

    return () => {
      cancelled = true;
      void subscription.then((unlisten) => unlisten());
    };
  }, [adoptFile, applyFile, openPath, isDirty, sourceDirty, selectedId]);

  async function selectEvent(id: string) {
    if (id === selectedId) return;
    if (isDirty && !(await confirmDiscard('Discard unsaved event changes?'))) return;

    const event = events.find((entry) => entry.id === id);
    setSelectedId(id);
    setDraft(event ? toEventUpdate(event) : null);
    setIsDirty(false);
    // The code view jumps to the event, so the two stay in step.
    if (event) setReveal((current) => ({ line: event.line, key: (current?.key ?? 0) + 1 }));
  }

  function patch(changes: Partial<EventUpdate>) {
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
    operation: () => Promise<EventFile>,
    eventId: (updated: EventFile) => string | null,
    done: string,
  ) {
    if (!selectedFile) return;

    setIsSaving(true);
    try {
      const updated = await operation();
      const persisted = formCommits;
      if (persisted) await writeFile(updated.source);
      applyFile(updated, eventId(updated));
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
      () => api.updateEventSource(path, source, doc.hasBom, doc.encoding, id, change),
      // The id itself may have changed, so follow the draft rather than the old id.
      () => change.id.trim() || id,
      change.id.trim() || id,
    );
  }

  async function remove() {
    if (!selectedFile || !selectedId) return;
    if (!(await confirmDelete(`Delete event "${selectedId}" from ${selectedFile.name}?`))) {
      return;
    }
    const path = selectedFile.fullPath;
    const id = selectedId;

    await commit(
      () => api.deleteEventSource(path, source, doc.hasBom, doc.encoding, id),
      (updated) => updated.events[0]?.id ?? null,
      id,
    );
  }

  async function addEvent(event: EventUpdate) {
    if (!selectedFile) return;
    const path = selectedFile.fullPath;

    await commit(
      () => api.addEventSource(path, source, doc.hasBom, doc.encoding, event),
      () => event.id,
      event.id,
    );
  }

  const form =
    !draft || !selected ? (
      <EmptyState
        icon={<CalendarClock />}
        title='Select an event'
        description='Pick one from the list to edit its text keys, picture, trigger and options.'
      />
    ) : duplicates.has(selected.id) ? (
      <EmptyState
        icon={<ClipboardList />}
        title='Two events share this id'
        description={`More than one event in this file is ${selected.id}. The game cannot tell them apart and neither can the form; give one a new id in the code view.`}
        action={
          view === 'visual' ? (
            <Button size='sm' onClick={() => chooseView('split')}>
              Open the code view
            </Button>
          ) : undefined
        }
      />
    ) : (
      <div className='flex h-full min-h-0 flex-col'>
        <div className='flex shrink-0 items-center gap-2 border-b border-border px-3 py-2'>
          <span className='truncate font-mono text-sm'>{selected.id}</span>
          <span className='text-xs text-muted'>line {selected.line}</span>
          <div className='ml-auto flex items-center gap-1.5'>
            {isDirty ? <Badge tone='accent'>Unsaved</Badge> : null}
            <Button variant='ghost' size='icon-sm' title='Delete event' onClick={() => void remove()}>
              <Trash2 />
            </Button>
            <Button
              variant='primary'
              size='sm'
              onClick={() => void save()}
              disabled={!isDirty || isSaving}
              title={
                formCommits ? 'Write this event to the file' : 'Put this event into the file text; Save file writes it'
              }
            >
              {isSaving ? <Loader2 className='animate-spin' /> : <Save />}
              {formCommits ? 'Save' : 'Apply'}
            </Button>
          </div>
        </div>
        <div className='min-h-0 flex-1 overflow-auto p-3'>
          <EventForm draft={draft} picture={icons.get(draft.picture.trim())} onChange={patch} />
        </div>
      </div>
    );

  return (
    <div className='grid h-full grid-cols-[16rem_minmax(0,1fr)] gap-3 p-3'>
      <div className='grid min-h-0 grid-rows-[2fr_3fr] gap-3'>
        <Panel>
          <PanelHeader title='Event files' subtitle={`${files.length} in ${EVENT_FOLDER}`} />
          <PanelBody>
            {files.length === 0 ? (
              <EmptyState title='No event files' description={`This mod has no .txt files under ${EVENT_FOLDER}.`} />
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
            title='Events'
            subtitle={
              file
                ? file.namespaces.length
                  ? `${events.length} in ${file.namespaces.join(', ')}`
                  : `${events.length} in this file`
                : 'No file open'
            }
            actions={
              file ? (
                <Button variant='ghost' size='icon-sm' title='Add event' onClick={() => setIsAdding(true)}>
                  <Plus />
                </Button>
              ) : null
            }
          />
          <PanelBody>
            {events.length === 0 ? (
              <EmptyState title='Nothing to show' description='Open an event file first.' />
            ) : (
              <ul className='py-1'>
                {events.map((event) => (
                  <li key={`${event.id}-${event.line}`}>
                    <ListRow
                      active={event.id === selectedId}
                      onClick={() => void selectEvent(event.id)}
                      title={`line ${event.line}`}
                      className='py-1'
                    >
                      <span className='flex min-w-0 flex-1 flex-col'>
                        <span className='truncate font-mono text-xs'>{event.id || '(no id)'}</span>
                        <span className='truncate text-[0.6875rem] text-muted'>
                          {event.titles[0]
                            ? isScriptedText(event.titles[0])
                              ? 'triggered title'
                              : event.titles[0]
                            : 'no title'}
                        </span>
                      </span>
                      {duplicates.has(event.id) ? (
                        <Badge tone='danger' className='shrink-0'>
                          duplicate
                        </Badge>
                      ) : event.kind !== 'country_event' ? (
                        <Badge tone='outline' className='shrink-0'>
                          {kindLabel(event.kind).toLowerCase()}
                        </Badge>
                      ) : null}
                    </ListRow>
                  </li>
                ))}
              </ul>
            )}
          </PanelBody>
        </Panel>
      </div>

      <Panel>
        <PanelHeader
          title={file ? selectedFile?.name : 'Events'}
          subtitle={file ? `${events.length} events` : 'Open a file to edit its events'}
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
              Reading events…
            </div>
          ) : !file || !selectedFile ? (
            <EmptyState
              icon={<CalendarClock />}
              title='No event file open'
              description='Pick a file from events to see what it fires.'
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
        <AddEventDialog
          suggestedId={nextEventId(events, file.namespaces[0] ?? '')}
          onOpenChange={setIsAdding}
          onAdd={async (event) => {
            await addEvent(event);
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

function EventForm({
  draft,
  picture,
  onChange,
}: {
  draft: EventUpdate;
  /** What the picture name resolves to, undefined while it is being looked up. */
  picture: SpriteIcon | undefined;
  onChange: (changes: Partial<EventUpdate>) => void;
}) {
  return (
    <div className='@container flex flex-col gap-4'>
      <div className='grid gap-3 @lg:grid-cols-[10rem_1fr]'>
        <Field label='Kind'>
          <Select value={draft.kind} onValueChange={(kind) => onChange({ kind: kind as EventKind })}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {EVENT_KINDS.map((kind) => (
                <SelectItem key={kind.value} value={kind.value}>
                  {kind.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
        <Field label='Id' hint='namespace.number; what the scripts that fire it name'>
          <CodeInput value={draft.id} onChange={(event) => onChange({ id: event.target.value })} />
        </Field>
      </div>

      <TextList
        label='Title'
        addLabel='Add triggered title'
        values={draft.titles}
        placeholder='my_events.1.t'
        template={`text = ${draft.id || 'my_events.1'}.t.${letter(draft.titles.length)}\ntrigger = { }`}
        onChange={(titles) => onChange({ titles })}
      />
      <TextList
        label='Description'
        addLabel='Add triggered description'
        values={draft.descs}
        placeholder='my_events.1.d'
        template={`text = ${draft.id || 'my_events.1'}.d.${letter(draft.descs.length)}\ntrigger = { }`}
        onChange={(descs) => onChange({ descs })}
      />

      <Field label='Picture' hint='A GFX_report_event_ or GFX_news_event_ sprite'>
        <CodeInput value={draft.picture} onChange={(event) => onChange({ picture: event.target.value })} />
      </Field>
      {draft.picture.trim() ? <PicturePreview picture={picture} /> : null}

      <div className='grid gap-3 @md:grid-cols-2 @2xl:grid-cols-5'>
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
        <Field label='Timeout days' hint='Before the first option is picked'>
          <CodeInput value={draft.timeoutDays} onChange={(event) => onChange({ timeoutDays: event.target.value })} />
        </Field>
      </div>

      <BlockField
        label='Trigger'
        value={draft.trigger}
        onChange={(trigger) => onChange({ trigger })}
        placeholder='tag = GER'
      />
      <BlockField
        label='Mean time to happen'
        value={draft.meanTimeToHappen}
        onChange={(meanTimeToHappen) => onChange({ meanTimeToHappen })}
        placeholder='days = 30'
      />
      <BlockField
        label='Immediate'
        value={draft.immediate}
        onChange={(immediate) => onChange({ immediate })}
        placeholder='hidden_effect = { set_country_flag = my_flag }'
      />

      <div className='space-y-2 rounded-lg border border-border p-3'>
        <div className='flex items-center justify-between'>
          <Label>
            Options
            {draft.options.length > 1 ? ` · ${draft.options.length}` : ''}
          </Label>
          <Button size='sm' onClick={() => onChange({ options: [...draft.options, { name: '', body: '' }] })}>
            <Plus />
            Add option
          </Button>
        </div>
        {draft.options.length === 0 ? (
          <p className='text-xs text-muted'>None. An event with no option cannot be closed.</p>
        ) : null}
        {draft.options.map((option, index) => (
          <OptionCard
            key={index}
            option={option}
            onChange={(next) => {
              const options = [...draft.options];
              options[index] = next;
              onChange({ options });
            }}
            onRemove={() => onChange({ options: draft.options.filter((_, at) => at !== index) })}
          />
        ))}
      </div>
    </div>
  );
}

/** The art the picture name resolves to, and the texture it came from. */
function PicturePreview({ picture }: { picture: SpriteIcon | undefined }) {
  return (
    <div className='-mt-2 flex items-center gap-2'>
      <span className='flex h-14 w-32 shrink-0 items-center justify-center overflow-hidden rounded-md border border-border bg-surface-sunken'>
        {picture?.url ? (
          // Data URL produced by the backend, so next/image cannot help here.
          // oxlint-disable-next-line next/no-img-element
          <img src={picture.url} alt='' className='size-full object-cover' />
        ) : (
          <ImageOff className='size-3.5 text-border-strong' />
        )}
      </span>
      <p className='min-w-0 truncate text-xs text-muted' title={picture?.path}>
        {picture === undefined ? '' : picture.path || 'No interface/*.gfx file defines this sprite'}
      </p>
    </div>
  );
}

/**
 * A title or description: usually one localisation key, sometimes several
 * `{ text = ... trigger = { } }` blocks the game picks between. A block is
 * edited as its body text.
 */
function TextList({
  label,
  addLabel,
  values,
  placeholder,
  template,
  onChange,
}: {
  label: string;
  addLabel: string;
  values: string[];
  placeholder: string;
  /** What a new triggered entry starts as: script the file will accept. */
  template: string;
  onChange: (values: string[]) => void;
}) {
  const set = (index: number, value: string) => {
    const next = [...values];
    next[index] = value;
    onChange(next);
  };

  return (
    <div className='flex flex-col gap-1.5'>
      <div className='flex items-center justify-between'>
        <Label>{label}</Label>
        <Button variant='ghost' size='sm' onClick={() => onChange([...values, values.length === 0 ? '' : template])}>
          <Plus />
          {values.length === 0 ? `Add ${label.toLowerCase()}` : addLabel}
        </Button>
      </div>
      {values.length === 0 ? <p className='text-xs text-muted'>None: the game shows the key itself.</p> : null}
      {values.map((value, index) => (
        <div key={index} className='flex items-start gap-1.5'>
          {isScriptedText(value) ? (
            <Textarea rows={3} value={value} onChange={(event) => set(index, event.target.value)} className='min-h-0' />
          ) : (
            <CodeInput value={value} placeholder={placeholder} onChange={(event) => set(index, event.target.value)} />
          )}
          <Button
            variant='ghost'
            size='icon-sm'
            title='Remove'
            className='mt-1 shrink-0'
            onClick={() => onChange(values.filter((_, at) => at !== index))}
          >
            <X />
          </Button>
        </div>
      ))}
    </div>
  );
}

function OptionCard({
  option,
  onChange,
  onRemove,
}: {
  option: EventOption;
  onChange: (option: EventOption) => void;
  onRemove: () => void;
}) {
  return (
    <div className='relative flex flex-col gap-3 rounded-md border border-border bg-surface-raised/40 p-3'>
      <Button
        variant='ghost'
        size='icon-sm'
        title='Remove this option'
        className='absolute right-1.5 top-1.5'
        onClick={onRemove}
      >
        <X />
      </Button>
      <Field label='Name' hint='Localisation key of the button' className='pr-8'>
        <CodeInput value={option.name} onChange={(event) => onChange({ ...option, name: event.target.value })} />
      </Field>
      <Field label='Effects' hint='Everything else in the option: ai_chance, trigger and the effects'>
        <Textarea
          rows={4}
          value={option.body}
          placeholder={'ai_chance = { factor = 50 }\nadd_political_power = 50'}
          onChange={(event) => onChange({ ...option, body: event.target.value })}
        />
      </Field>
    </div>
  );
}

function AddEventDialog({
  suggestedId,
  onOpenChange,
  onAdd,
}: {
  suggestedId: string;
  onOpenChange: (open: boolean) => void;
  /** Adds the event; the page decides whether that also writes the file. */
  onAdd: (event: EventUpdate) => Promise<void>;
}) {
  const [kind, setKind] = React.useState<EventKind>('country_event');
  const [id, setId] = React.useState(suggestedId);
  const [isSaving, setIsSaving] = React.useState(false);

  async function create() {
    const trimmed = id.trim();
    if (!trimmed) return;

    setIsSaving(true);
    try {
      // A stub the game will accept: text keys named after the id, one
      // option, and triggered-only so nothing fires it by accident.
      await onAdd({
        ...emptyEventUpdate(kind),
        id: trimmed,
        titles: [`${trimmed}.t`],
        descs: [`${trimmed}.d`],
        picture: kind === 'news_event' ? 'GFX_news_event_generic_read_write' : 'GFX_report_event_generic_read_write',
        isTriggeredOnly: 'yes',
        options: [{ name: `${trimmed}.a`, body: '' }],
      });
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <Dialog open onOpenChange={onOpenChange}>
      <DialogContent
        title='Add an event'
        description='A triggered-only stub with one option is appended to the file; its title, description and option keys still need localisation entries.'
      >
        <DialogBody className='flex flex-col gap-3'>
          <Field label='Kind'>
            <Select value={kind} onValueChange={(value) => setKind(value as EventKind)}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {EVENT_KINDS.map((entry) => (
                  <SelectItem key={entry.value} value={entry.value}>
                    {entry.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>
          <Field label='Event id' hint="The file's namespace, a dot and the next free number">
            <CodeInput value={id} autoFocus placeholder='my_events.1' onChange={(event) => setId(event.target.value)} />
          </Field>
        </DialogBody>
        <DialogFooter>
          <DialogClose asChild>
            <Button>Cancel</Button>
          </DialogClose>
          <Button variant='primary' onClick={() => void create()} disabled={!id.trim() || isSaving}>
            {isSaving ? <Loader2 className='animate-spin' /> : <Plus />}
            Add event
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
