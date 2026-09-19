'use client';

import * as React from 'react';
import { listen } from '@tauri-apps/api/event';
import {
  Code2,
  Columns2,
  FileText,
  FolderTree,
  ImageOff,
  LayoutTemplate,
  Loader2,
  Plus,
  Save,
  Trash2,
  TriangleAlert,
  X,
} from 'lucide-react';
import { toast } from 'sonner';

import { FocusCanvas } from '@/components/focus-canvas';
import { ScriptEditor, type Reveal } from '@/components/script-editor';
import { Button } from '@/components/ui/button';
import { Dialog, DialogBody, DialogClose, DialogContent, DialogFooter } from '@/components/ui/dialog';
import { CodeInput, Field, Label, Textarea } from '@/components/ui/form';
import { Badge, EmptyState, ListRow, Panel, PanelBody, PanelHeader } from '@/components/ui/panel';
import { confirmDelete, confirmDiscard } from '@/lib/dialogs';
import { api, describeError } from '@/lib/ipc';
import { useAppStore } from '@/lib/store';
import { isScriptedIcon, spriteName, useSpriteIcons } from '@/lib/use-sprite-icons';
import { useUnsavedIn } from '@/lib/use-unsaved';
import {
  emptyFocusUpdate,
  toFocusUpdate,
  type Encoding,
  type FocusFile,
  type FocusTree,
  type FocusUpdate,
  type ProjectFile,
  type SpriteIcon,
} from '@/lib/types';
import { cn, isInFolder } from '@/lib/utils';

const FOCUS_FOLDER = 'common/national_focus';

/** Sent by the backend when an MCP client writes a file. */
const FILE_CHANGED_EVENT = 'project://file-changed';

/** How the tree panel is laid out. Remembered per machine. */
type View = 'visual' | 'split' | 'code';
const VIEW_KEY = 'hoi4ms.focus-view';
const VIEWS: Array<{ value: View; label: string; icon: React.ComponentType<{ className?: string }> }> = [
  { value: 'visual', label: 'Visual', icon: LayoutTemplate },
  { value: 'split', label: 'Split', icon: Columns2 },
  { value: 'code', label: 'Code', icon: Code2 },
];

/** How long the buffer sits still before it is parsed again. */
const PARSE_DELAY_MS = 250;

type Newline = '\n' | '\r\n';

/**
 * The buffer is kept with `\n` line breaks, which is the only kind the code
 * editor produces; the file's own kind is remembered and put back on write.
 * Without this, opening a CRLF file marked it changed on the spot, and saving
 * would have rewritten every line ending.
 */
function toBuffer(text: string): string {
  return text.replaceAll('\r\n', '\n');
}

function newlineOf(text: string): Newline {
  return text.includes('\r\n') ? '\r\n' : '\n';
}

function storedView(): View {
  try {
    const stored = window.localStorage.getItem(VIEW_KEY);
    return VIEWS.some((view) => view.value === stored) ? (stored as View) : 'visual';
  } catch {
    return 'visual';
  }
}

/**
 * The focus editor.
 *
 * The file's text is the document. The tree, the focus list and the form are
 * all read out of it, and every change — a field applied from the form, a
 * focus added or deleted, a key typed into the code view — goes back into it
 * through the backend's surgical editor. When the code view is hidden and
 * the text has no edits of its own, the form's Save writes the file at once,
 * as a form should; otherwise the form only applies, and the file is written
 * on request, the way a text editor works.
 */
export default function FocusPage() {
  const { project, scan, setError } = useAppStore();

  const [selectedFile, setSelectedFile] = React.useState<ProjectFile | null>(null);
  const [focusFile, setFocusFile] = React.useState<FocusFile | null>(null);
  const [selectedId, setSelectedId] = React.useState<string | null>(null);
  const [draft, setDraft] = React.useState<FocusUpdate | null>(null);
  const [isDirty, setIsDirty] = React.useState(false);
  const [isLoading, setIsLoading] = React.useState(false);
  const [isSaving, setIsSaving] = React.useState(false);
  const [isAdding, setIsAdding] = React.useState(false);
  // Counts open requests so a slower read can tell it has been superseded.
  // A path would not be enough: opening A, then B, then A again would let the
  // first read pass while the third is still in flight.
  const openRequest = React.useRef(0);

  const [view, setView] = React.useState<View>(storedView);
  // The document: what the code view shows and every edit goes through.
  const [source, setSource] = React.useState('');
  // What the file on disk last held, so the two can be compared.
  const [savedSource, setSavedSource] = React.useState('');
  const [encoding, setEncoding] = React.useState<Encoding>('utf8');
  const [hasBom, setHasBom] = React.useState(false);
  const [newline, setNewline] = React.useState<Newline>('\n');
  const [parseError, setParseError] = React.useState<string | null>(null);
  const [reveal, setReveal] = React.useState<Reveal | undefined>(undefined);

  const files = React.useMemo(
    () => (scan?.files ?? []).filter((file) => isInFolder(file.relativePath, FOCUS_FOLDER) && file.extension === 'txt'),
    [scan],
  );

  const sourceDirty = source !== savedSource;
  // With the code hidden and the text untouched, the form's Save writes the
  // file, the way it always has. Once the text has edits of its own the form
  // can only apply: writing would take those along uninvited.
  const formCommits = view === 'visual' && !sourceDirty;

  useUnsavedIn('focus editor', isDirty || sourceDirty);

  const focuses = React.useMemo(() => focusFile?.focuses ?? [], [focusFile]);
  const selected = focuses.find((focus) => focus.id === selectedId) ?? null;

  // The tree is drawn from the draft rather than the file, so a dragged node
  // and a retyped icon both show before anything is saved.
  const shown = React.useMemo(
    () => focuses.map((focus) => (focus.id === selectedId && draft ? { ...focus, ...draft } : focus)),
    [draft, focuses, selectedId],
  );
  const iconNames = React.useMemo(() => shown.map((focus) => spriteName(focus.icon)), [shown]);
  const icons = useSpriteIcons(project?.folderPath, iconNames);

  function chooseView(next: View) {
    setView(next);
    try {
      window.localStorage.setItem(VIEW_KEY, next);
    } catch {
      // A private window forgets; the choice still holds for this session.
    }
  }

  /**
   * Takes on a file as it was just read from disk: its text is what is
   * saved, and how it was written is how it will be written again. A file
   * handed back by a buffer operation goes through `applyFile` alone,
   * since its text describes the encoding it was asked for, not what the
   * disk ended up with.
   */
  const adoptFile = React.useCallback((loaded: FocusFile) => {
    setSavedSource(toBuffer(loaded.source));
    setEncoding(loaded.encoding);
    setHasBom(loaded.hasBom);
    setNewline(newlineOf(loaded.source));
  }, []);

  /** Shows a file the backend handed back, selecting `focusId`. */
  const applyFile = React.useCallback((loaded: FocusFile, focusId: string | null) => {
    const text = toBuffer(loaded.source);
    setFocusFile({ ...loaded, source: text });
    setSource(text);
    setParseError(null);
    setSelectedId(focusId);
    const focus = loaded.focuses.find((entry) => entry.id === focusId);
    setDraft(focus ? toFocusUpdate(focus) : null);
    setIsDirty(false);
  }, []);

  async function confirmLeavingEdits(): Promise<boolean> {
    if (isDirty && !(await confirmDiscard('Discard unsaved focus changes?'))) return false;
    if (sourceDirty && !(await confirmDiscard('Discard unsaved changes to the file text?'))) return false;
    return true;
  }

  async function openFile(file: ProjectFile) {
    if (!(await confirmLeavingEdits())) return;

    // A slower read must not replace the focuses of the file opened after it.
    const request = ++openRequest.current;
    setSelectedFile(file);
    setIsLoading(true);
    try {
      const loaded = await api.readFocusFile(file.fullPath);
      if (openRequest.current !== request) return;

      adoptFile(loaded);
      applyFile(loaded, loaded.focuses[0]?.id ?? null);
    } catch (error) {
      if (openRequest.current !== request) return;

      setError(describeError(error));
      setFocusFile(null);
      setSelectedId(null);
      setDraft(null);
    } finally {
      if (openRequest.current === request) setIsLoading(false);
    }
  }

  // What the parse callback below needs to know at the moment it lands, not
  // at the moment the timer was set.
  const latest = React.useRef({ isDirty, selectedId });
  React.useEffect(() => {
    latest.current = { isDirty, selectedId };
  });

  // The tree follows the code view. The buffer is parsed once it sits still,
  // and a text that does not parse keeps the last tree up with the error
  // shown beside the code, so a half-typed block does not blank the canvas.
  const openPath = selectedFile?.fullPath;
  React.useEffect(() => {
    if (!openPath || !focusFile || source === focusFile.source) return;

    let cancelled = false;
    const timer = window.setTimeout(() => {
      void api
        .parseFocusSource(openPath, source, hasBom, encoding)
        .then((parsed) => {
          if (cancelled) return;

          const current = latest.current.selectedId;
          const stillThere = current !== null && parsed.focuses.some((focus) => focus.id === current);
          const nextId = stillThere ? current : (parsed.focuses[0]?.id ?? null);

          setFocusFile(parsed);
          setParseError(null);
          setSelectedId(nextId);
          // A form the user is mid-way through editing keeps its draft; an
          // untouched one follows the new text.
          if (!latest.current.isDirty) {
            const focus = parsed.focuses.find((entry) => entry.id === nextId);
            setDraft(focus ? toFocusUpdate(focus) : null);
          }
        })
        .catch((error: unknown) => {
          if (!cancelled) setParseError(describeError(error));
        });
    }, PARSE_DELAY_MS);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [encoding, focusFile, hasBom, openPath, source]);

  // An assistant editing this file through MCP would otherwise leave the
  // tree showing what the file used to say. Unsaved edits win: the reload
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
      void api.readFocusFile(openPath).then((loaded) => {
        if (cancelled || openRequest.current !== request) return;
        adoptFile(loaded);
        // Keep the selection where it was when the focus still exists.
        applyFile(
          loaded,
          loaded.focuses.some((focus) => focus.id === selectedId) ? selectedId : (loaded.focuses[0]?.id ?? null),
        );
        toast.message('Reloaded: changed by an MCP client');
      });
    });

    return () => {
      cancelled = true;
      void subscription.then((unlisten) => unlisten());
    };
  }, [adoptFile, applyFile, openPath, isDirty, sourceDirty, selectedId]);

  async function selectFocus(id: string) {
    if (id === selectedId) return;
    if (isDirty && !(await confirmDiscard('Discard unsaved focus changes?'))) return;

    const focus = focuses.find((entry) => entry.id === id);
    setSelectedId(id);
    setDraft(focus ? toFocusUpdate(focus) : null);
    setIsDirty(false);
    // The code view jumps to the focus, so the two stay in step.
    if (focus) setReveal((current) => ({ line: focus.line, key: (current?.key ?? 0) + 1 }));
  }

  function patch(changes: Partial<FocusUpdate>) {
    setDraft((current) => (current ? { ...current, ...changes } : current));
    setIsDirty(true);
  }

  /** Dragging on the canvas edits the focus that was dragged. */
  async function moveFocus(id: string, x: number, y: number) {
    if (id !== selectedId) {
      if (isDirty && !(await confirmDiscard('Discard unsaved focus changes?'))) return;
      const focus = focuses.find((entry) => entry.id === id);
      if (!focus) return;
      setSelectedId(id);
      setDraft({ ...toFocusUpdate(focus), x: String(x), y: String(y) });
      setIsDirty(true);
      return;
    }

    patch({ x: String(x), y: String(y) });
  }

  /** Writes `text` as the file, reporting an encoding the text no longer fits. */
  async function writeFile(text: string) {
    if (!selectedFile) return;

    const used = await api.writeTextFile(selectedFile.fullPath, text.replaceAll('\n', newline), encoding, hasBom);
    setSavedSource(text);
    if (used !== encoding) {
      // The text outgrew Windows-1252, so the backend fell back to UTF-8.
      setEncoding(used);
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
    operation: () => Promise<FocusFile>,
    focusId: (updated: FocusFile) => string | null,
    done: string,
  ) {
    if (!selectedFile) return;

    setIsSaving(true);
    try {
      const updated = await operation();
      const persisted = formCommits;
      if (persisted) await writeFile(updated.source);
      applyFile(updated, focusId(updated));
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
      () => api.updateFocusSource(path, source, hasBom, encoding, id, change),
      // The id itself may have changed, so follow the draft rather than the old id.
      () => change.id || id,
      change.id || id,
    );
  }

  async function remove() {
    if (!selectedFile || !selectedId) return;
    if (!(await confirmDelete(`Delete focus "${selectedId}" from ${selectedFile.name}?`))) {
      return;
    }
    const path = selectedFile.fullPath;
    const id = selectedId;

    await commit(
      () => api.deleteFocusSource(path, source, hasBom, encoding, id),
      (updated) => updated.focuses[0]?.id ?? null,
      id,
    );
  }

  async function addFocus(treeId: string, focus: FocusUpdate) {
    if (!selectedFile) return;
    const path = selectedFile.fullPath;

    await commit(
      () => api.addFocusSource(path, source, hasBom, encoding, treeId, focus),
      () => focus.id,
      focus.id,
    );
  }

  return (
    <div className='grid h-full grid-cols-[15rem_minmax(0,1fr)_20rem] gap-3 p-3'>
      <div className='grid min-h-0 grid-rows-2 gap-3'>
        <Panel>
          <PanelHeader title='Focus files' subtitle={`${files.length} in ${FOCUS_FOLDER}`} />
          <PanelBody>
            {files.length === 0 ? (
              <EmptyState title='No focus files' description={`This mod has no .txt files under ${FOCUS_FOLDER}.`} />
            ) : (
              <ul className='py-1'>
                {files.map((file) => (
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
            )}
          </PanelBody>
        </Panel>

        <Panel>
          <PanelHeader
            title='Focuses'
            subtitle={focusFile ? `${focuses.length} in this file` : 'No file open'}
            actions={
              focusFile ? (
                <Button variant='ghost' size='icon-sm' title='Add focus' onClick={() => setIsAdding(true)}>
                  <Plus />
                </Button>
              ) : null
            }
          />
          <PanelBody>
            {focuses.length === 0 ? (
              <EmptyState title='Nothing to show' description='Open a focus file first.' />
            ) : (
              <ul className='py-1'>
                {focuses.map((focus) => (
                  <li key={`${focus.id}-${focus.line}`}>
                    <ListRow
                      active={focus.id === selectedId}
                      onClick={() => void selectFocus(focus.id)}
                      title={`line ${focus.line}`}
                    >
                      <span className='truncate font-mono text-xs'>{focus.id || '(no id)'}</span>
                      {focus.shared ? (
                        <Badge tone='outline' className='ml-auto shrink-0'>
                          shared
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
          title={focusFile ? selectedFile?.name : 'Focus tree'}
          subtitle={
            focusFile?.trees.length
              ? focusFile.trees.map((tree) => tree.id || '(unnamed tree)').join(', ')
              : 'Open a file to see its tree'
          }
          actions={
            <>
              {sourceDirty ? <Badge tone='accent'>Unsaved file</Badge> : null}
              {sourceDirty ? (
                <Button variant='primary' size='sm' onClick={() => void saveFile()} disabled={isSaving} title='Ctrl+S'>
                  {isSaving ? <Loader2 className='animate-spin' /> : <Save />}
                  Save file
                </Button>
              ) : null}
              <div role='radiogroup' aria-label='Layout' className='flex rounded-md border border-border p-0.5'>
                {VIEWS.map((option) => {
                  const Icon = option.icon;
                  const active = option.value === view;
                  return (
                    <button
                      key={option.value}
                      type='button'
                      role='radio'
                      aria-checked={active}
                      title={option.label}
                      onClick={() => chooseView(option.value)}
                      className={cn(
                        'flex size-7 items-center justify-center rounded transition-colors',
                        active ? 'bg-surface-sunken text-foreground' : 'text-muted hover:text-foreground',
                      )}
                    >
                      <Icon className='size-4' />
                    </button>
                  );
                })}
              </div>
            </>
          }
        />
        <PanelBody className='overflow-hidden p-0'>
          {isLoading ? (
            <div className='flex h-full items-center justify-center gap-2 text-sm text-muted'>
              <Loader2 className='size-4 animate-spin' />
              Reading focuses…
            </div>
          ) : !focusFile || !selectedFile ? (
            <EmptyState
              icon={<FolderTree />}
              title='No focus tree loaded'
              description='Pick a file from common/national_focus to see its focuses laid out on the game grid.'
            />
          ) : (
            <div className={cn('grid h-full', view === 'split' ? 'grid-cols-2' : 'grid-cols-1')}>
              {view !== 'visual' ? (
                <div className={cn('flex min-h-0 flex-col', view === 'split' && 'border-r border-border')}>
                  {parseError ? (
                    <div className='flex shrink-0 items-start gap-2 border-b border-border bg-danger-soft px-3 py-1.5 text-xs text-danger'>
                      <TriangleAlert className='mt-0.5 size-3.5 shrink-0' />
                      <span className='min-w-0 break-words'>{parseError}</span>
                    </div>
                  ) : null}
                  <ScriptEditor
                    value={source}
                    onChange={setSource}
                    onSave={() => void saveFile()}
                    reveal={reveal}
                    className='min-h-0 flex-1 overflow-hidden'
                  />
                </div>
              ) : null}
              {view !== 'code' ? (
                focuses.length === 0 ? (
                  <EmptyState
                    icon={<FolderTree />}
                    title='No focuses in this file'
                    description='Add one with the + button, or write one in the code view.'
                  />
                ) : (
                  <FocusCanvas
                    focuses={shown}
                    icons={icons}
                    selectedId={selectedId}
                    onSelect={(id) => void selectFocus(id)}
                    onMove={(id, x, y) => void moveFocus(id, x, y)}
                  />
                )
              ) : null}
            </div>
          )}
        </PanelBody>
      </Panel>

      <Panel>
        <PanelHeader
          title={selected ? 'Focus properties' : 'Nothing selected'}
          subtitle={selected ? `line ${selected.line}` : undefined}
          actions={
            selected ? (
              <>
                {isDirty ? <Badge tone='accent'>Unsaved</Badge> : null}
                <Button variant='ghost' size='icon-sm' title='Delete focus' onClick={() => void remove()}>
                  <Trash2 />
                </Button>
                <Button
                  variant='primary'
                  size='sm'
                  onClick={() => void save()}
                  disabled={!isDirty || isSaving}
                  title={
                    formCommits
                      ? 'Write this focus to the file'
                      : 'Put this focus into the file text; Save file writes it'
                  }
                >
                  {isSaving ? <Loader2 className='animate-spin' /> : <Save />}
                  {formCommits ? 'Save' : 'Apply'}
                </Button>
              </>
            ) : null
          }
        />
        <PanelBody className='p-3'>
          {!draft ? (
            <EmptyState title='Select a focus' description='Pick one from the list or the tree to edit its fields.' />
          ) : (
            <FocusForm
              draft={draft}
              icon={icons.get(spriteName(draft.icon))}
              focusIds={focuses.map((focus) => focus.id).filter(Boolean)}
              onChange={patch}
            />
          )}
        </PanelBody>
      </Panel>

      {/* Mounted only while open so its fields start empty every time. */}
      {isAdding && focusFile ? (
        <AddFocusDialog
          trees={focusFile.trees}
          onOpenChange={setIsAdding}
          onAdd={async (treeId, focus) => {
            await addFocus(treeId, focus);
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

function FocusForm({
  draft,
  icon,
  focusIds,
  onChange,
}: {
  draft: FocusUpdate;
  /** What the sprite name resolves to, undefined while it is being looked up. */
  icon: SpriteIcon | undefined;
  focusIds: string[];
  onChange: (changes: Partial<FocusUpdate>) => void;
}) {
  const listId = React.useId();

  return (
    <div className='flex flex-col gap-3'>
      <datalist id={listId}>
        {focusIds.map((id) => (
          <option key={id} value={id} />
        ))}
      </datalist>

      <Field label='Id'>
        <CodeInput value={draft.id} onChange={(event) => onChange({ id: event.target.value })} />
      </Field>

      <Field
        label='Icon'
        hint={
          draft.icon.trim()
            ? isScriptedIcon(draft.icon)
              ? 'Picked by trigger; the first sprite is previewed. Replace the whole text with a name for a plain icon.'
              : undefined
            : 'Sprite name, e.g. GFX_goal_generic_army_doctrines'
        }
      >
        {isScriptedIcon(draft.icon) ? (
          <Textarea rows={5} value={draft.icon} onChange={(event) => onChange({ icon: event.target.value })} />
        ) : (
          <CodeInput value={draft.icon} onChange={(event) => onChange({ icon: event.target.value })} />
        )}
      </Field>
      {draft.icon.trim() ? <IconPreview icon={icon} /> : null}

      <div className='grid grid-cols-3 gap-2'>
        <Field label='X'>
          <CodeInput value={draft.x} onChange={(event) => onChange({ x: event.target.value })} />
        </Field>
        <Field label='Y'>
          <CodeInput value={draft.y} onChange={(event) => onChange({ y: event.target.value })} />
        </Field>
        <Field label='Cost'>
          <CodeInput value={draft.cost} onChange={(event) => onChange({ cost: event.target.value })} />
        </Field>
      </div>

      <Field label='Relative to' hint='Leave empty for absolute coordinates'>
        <CodeInput
          list={listId}
          value={draft.relativePositionId}
          onChange={(event) => onChange({ relativePositionId: event.target.value })}
        />
      </Field>

      <div className='flex flex-col gap-1.5'>
        <div className='flex items-center justify-between'>
          <Label>Prerequisites</Label>
          <Button
            variant='ghost'
            size='icon-sm'
            title='Add prerequisite group'
            onClick={() => onChange({ prerequisites: [...draft.prerequisites, ['']] })}
          >
            <Plus />
          </Button>
        </div>
        {draft.prerequisites.length === 0 ? (
          <p className='text-xs text-muted'>None. The focus is available from the start.</p>
        ) : (
          draft.prerequisites.map((group, index) => (
            <div key={index} className='flex items-center gap-1.5'>
              <CodeInput
                list={listId}
                value={group.join(' ')}
                placeholder='focus_a focus_b'
                onChange={(event) => {
                  const next = [...draft.prerequisites];
                  next[index] = event.target.value.split(/\s+/).filter(Boolean);
                  onChange({ prerequisites: next });
                }}
              />
              <Button
                variant='ghost'
                size='icon-sm'
                title='Remove group'
                onClick={() =>
                  onChange({
                    prerequisites: draft.prerequisites.filter((_, at) => at !== index),
                  })
                }
              >
                <X />
              </Button>
            </div>
          ))
        )}
        <p className='text-xs text-muted'>Ids in one row satisfy each other; each row must be satisfied.</p>
      </div>

      <Field label='Mutually exclusive'>
        <CodeInput
          list={listId}
          value={draft.mutuallyExclusive.join(' ')}
          placeholder='focus_c focus_d'
          onChange={(event) => onChange({ mutuallyExclusive: event.target.value.split(/\s+/).filter(Boolean) })}
        />
      </Field>

      <BlockField
        label='Completion reward'
        value={draft.completionReward}
        onChange={(value) => onChange({ completionReward: value })}
        open
      />
      <BlockField label='Available' value={draft.available} onChange={(value) => onChange({ available: value })} />
      <BlockField label='Bypass' value={draft.bypass} onChange={(value) => onChange({ bypass: value })} />
      <BlockField
        label='Allow branch'
        value={draft.allowBranch}
        onChange={(value) => onChange({ allowBranch: value })}
      />
      <BlockField label='AI will do' value={draft.aiWillDo} onChange={(value) => onChange({ aiWillDo: value })} />
    </div>
  );
}

/**
 * The art the sprite name resolves to, and the texture it came from.
 *
 * The path is worth the line it takes: a name resolving to a file inside the
 * game folder is how a mod that meant to ship its own art shows up.
 */
function IconPreview({ icon }: { icon: SpriteIcon | undefined }) {
  return (
    <div className='-mt-1 flex items-center gap-2'>
      <span className='flex size-9 shrink-0 items-center justify-center overflow-hidden rounded-md border border-border bg-surface-sunken'>
        {icon?.url ? (
          // Data URL produced by the backend, so next/image cannot help here.
          // oxlint-disable-next-line next/no-img-element
          <img src={icon.url} alt='' className='max-h-full max-w-full object-contain' />
        ) : (
          <ImageOff className='size-3.5 text-border-strong' />
        )}
      </span>
      {/* Nothing is said until the lookup answers, so a name being typed does
          not flash "not found" between keystrokes. */}
      <p className='min-w-0 truncate text-xs text-muted' title={icon?.path}>
        {icon === undefined ? '' : icon.path || 'No interface/*.gfx file defines this sprite'}
      </p>
    </div>
  );
}

/** A script block shown as plain text, collapsed unless it has content. */
function BlockField({
  label,
  value,
  onChange,
  open,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  open?: boolean;
}) {
  return (
    <details open={open || value.trim().length > 0} className='group'>
      <summary className='cursor-pointer list-none text-xs font-medium uppercase tracking-wide text-muted marker:content-none hover:text-foreground'>
        {label}
        {value.trim() ? '' : ' · empty'}
      </summary>
      <Textarea
        className='mt-1.5'
        rows={4}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder='add_political_power = 120'
      />
    </details>
  );
}

function AddFocusDialog({
  onOpenChange,
  trees,
  onAdd,
}: {
  onOpenChange: (open: boolean) => void;
  trees: FocusTree[];
  /** Adds the focus; the page decides whether that also writes the file. */
  onAdd: (treeId: string, focus: FocusUpdate) => Promise<void>;
}) {
  const [id, setId] = React.useState('');
  const [treeId, setTreeId] = React.useState(trees[0]?.id ?? '');
  const [isSaving, setIsSaving] = React.useState(false);

  async function create() {
    if (!id.trim()) return;

    setIsSaving(true);
    try {
      await onAdd(treeId, {
        ...emptyFocusUpdate(),
        id: id.trim(),
        icon: 'GFX_goal_unknown',
        x: '0',
        y: '0',
        cost: '10',
      });
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <Dialog open onOpenChange={onOpenChange}>
      <DialogContent
        title='Add a focus'
        description='A stub focus is appended to the tree; fill in the rest on the right.'
      >
        <DialogBody className='flex flex-col gap-3'>
          <Field label='Focus id'>
            <CodeInput
              value={id}
              autoFocus
              placeholder='my_country_rearmament'
              onChange={(event) => setId(event.target.value)}
            />
          </Field>
          {trees.length > 1 ? (
            <Field label='Focus tree'>
              <CodeInput value={treeId} onChange={(event) => setTreeId(event.target.value)} />
            </Field>
          ) : null}
        </DialogBody>
        <DialogFooter>
          <DialogClose asChild>
            <Button>Cancel</Button>
          </DialogClose>
          <Button variant='primary' onClick={() => void create()} disabled={!id.trim() || isSaving}>
            {isSaving ? <Loader2 className='animate-spin' /> : <Plus />}
            Add focus
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
