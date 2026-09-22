'use client';

import * as React from 'react';
import { listen } from '@tauri-apps/api/event';
import { Code2, FileText, FlaskConical, LayoutTemplate, Loader2, Plus, Save, Trash2, X } from 'lucide-react';
import { toast } from 'sonner';

import { BlockField } from '@/components/block-field';
import type { Reveal } from '@/components/script-editor';
import { TreeCanvas, type TreeEdge, type TreeNode } from '@/components/tree-canvas';
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
  emptyTechnologyUpdate,
  toTechnologyUpdate,
  type ProjectFile,
  type TechFolder,
  type TechnologyFile,
  type TechnologyUpdate,
  type TechPath,
  type TechVariable,
} from '@/lib/types';
import { cn, isInFolder } from '@/lib/utils';

const TECH_FOLDER = 'common/technologies';

/** Sent by the backend when an MCP client writes a file. */
const FILE_CHANGED_EVENT = 'project://file-changed';

/** Where the panel's layout choice is remembered. */
const VIEW_KEY = 'hoi4ms.technologies-view';

const XP_TYPES = ['army', 'navy', 'air'];

/** The picture the game draws for a technology, by convention. */
function techSprite(id: string): string {
  return id.trim() ? `GFX_${id.trim()}_medium` : '';
}

/**
 * A position coordinate as a number: a plain one, or a `@variable` the file
 * declares. Anything else lands at zero rather than off the canvas.
 */
function resolveCoordinate(value: string, variables: TechVariable[]): number {
  const trimmed = value.trim();
  const looked = trimmed.startsWith('@') ? variables.find((entry) => entry.name === trimmed.slice(1))?.value : trimmed;
  const number = Number.parseFloat(looked ?? '');
  return Number.isFinite(number) ? number : 0;
}

/**
 * How to write a coordinate a drag produced: the `@variable` that stands for
 * it when the file spells positions that way, else the number.
 */
function spellCoordinate(number: number, original: string, variables: TechVariable[]): string {
  if (original.trim().startsWith('@')) {
    const named = variables.find((entry) => Number.parseFloat(entry.value) === number);
    if (named) return `@${named.name}`;
  }
  return String(number);
}

/**
 * The form's copy of a technology. Word lists are edited as one text each,
 * so the space being typed between two words is not swallowed by a split.
 */
type ListKey =
  | 'categories'
  | 'xor'
  | 'subTechnologies'
  | 'enableEquipments'
  | 'enableEquipmentModules'
  | 'enableSubunits';
type TechDraft = Omit<TechnologyUpdate, ListKey> & Record<ListKey, string>;

const LIST_KEYS: Array<{ key: ListKey; label: string; hint: string }> = [
  { key: 'categories', label: 'Categories', hint: 'Research categories, for bonuses and the AI' },
  { key: 'xor', label: 'Excludes', hint: 'Technologies this one rules out' },
  { key: 'subTechnologies', label: 'Sub-technologies', hint: 'Researched together with this one' },
  { key: 'enableEquipments', label: 'Enables equipment', hint: '' },
  { key: 'enableEquipmentModules', label: 'Enables modules', hint: '' },
  { key: 'enableSubunits', label: 'Enables subunits', hint: '' },
];

function toDraft(update: TechnologyUpdate): TechDraft {
  return {
    ...update,
    categories: update.categories.join(' '),
    xor: update.xor.join(' '),
    subTechnologies: update.subTechnologies.join(' '),
    enableEquipments: update.enableEquipments.join(' '),
    enableEquipmentModules: update.enableEquipmentModules.join(' '),
    enableSubunits: update.enableSubunits.join(' '),
  };
}

function toUpdate(draft: TechDraft): TechnologyUpdate {
  const words = (text: string) => text.split(/\s+/).filter(Boolean);
  return {
    ...draft,
    categories: words(draft.categories),
    xor: words(draft.xor),
    subTechnologies: words(draft.subTechnologies),
    enableEquipments: words(draft.enableEquipments),
    enableEquipmentModules: words(draft.enableEquipmentModules),
    enableSubunits: words(draft.enableSubunits),
  };
}

/**
 * The technology tree editor: a file's technologies drawn on the game's
 * grid, one folder at a time, with the selected one edited in a form or in
 * the file's text, like the focus editor.
 */
export default function TechnologiesPage() {
  const { project, scan, setError } = useAppStore();

  const [selectedFile, setSelectedFile] = React.useState<ProjectFile | null>(null);
  const [file, setFile] = React.useState<TechnologyFile | null>(null);
  const [selectedId, setSelectedId] = React.useState<string | null>(null);
  const [draft, setDraft] = React.useState<TechDraft | null>(null);
  const [isDirty, setIsDirty] = React.useState(false);
  const [isLoading, setIsLoading] = React.useState(false);
  const [isSaving, setIsSaving] = React.useState(false);
  const [isAdding, setIsAdding] = React.useState(false);
  const [folder, setFolder] = React.useState<string>('');
  // Counts open requests so a slower read can tell it has been superseded.
  const openRequest = React.useRef(0);

  const [view, chooseView] = useStoredView(VIEW_KEY);
  const [reveal, setReveal] = React.useState<Reveal | undefined>(undefined);

  const files = React.useMemo(
    () =>
      (scan?.files ?? []).filter((entry) => isInFolder(entry.relativePath, TECH_FOLDER) && entry.extension === 'txt'),
    [scan],
  );

  const technologies = React.useMemo(() => file?.technologies ?? [], [file]);
  const variables = React.useMemo(() => file?.variables ?? [], [file]);
  const selected = technologies.find((tech) => tech.id === selectedId) ?? null;

  // Every folder the file's technologies sit on, in first-seen order; the
  // canvas shows one at a time, as the game does.
  const folders = React.useMemo(() => {
    const seen: string[] = [];
    for (const tech of technologies) {
      for (const entry of tech.folders) {
        if (entry.name && !seen.includes(entry.name)) seen.push(entry.name);
      }
    }
    return seen;
  }, [technologies]);
  const shownFolder = folders.includes(folder) ? folder : (folders[0] ?? '');

  const latest = React.useRef({ isDirty, selectedId });
  React.useEffect(() => {
    latest.current = { isDirty, selectedId };
  });

  const openPath = selectedFile?.fullPath;
  const doc = useSourceDocument<TechnologyFile>({
    path: openPath,
    parse: api.parseTechnologySource,
    onParsed: (parsed) => {
      const current = latest.current.selectedId;
      const stillThere = current !== null && parsed.technologies.some((tech) => tech.id === current);
      const nextId = stillThere ? current : (parsed.technologies[0]?.id ?? null);

      setFile(parsed);
      setSelectedId(nextId);
      // A form the user is mid-way through editing keeps its draft; an
      // untouched one follows the new text.
      if (!latest.current.isDirty) {
        const tech = parsed.technologies.find((entry) => entry.id === nextId);
        setDraft(tech ? toDraft(toTechnologyUpdate(tech)) : null);
      }
    },
  });
  const { source, setSource, isDirty: sourceDirty, parseError } = doc;

  // With the code hidden and the text untouched, the form's Save writes the
  // file at once. Once the text has edits of its own the form can only
  // apply: writing would take those along uninvited.
  const formCommits = view === 'visual' && !sourceDirty;

  useUnsavedIn('technology editor', isDirty || sourceDirty);

  // The tree is drawn from the draft rather than the file, so a dragged node
  // shows where it landed before anything is saved.
  const shown = React.useMemo(
    () => technologies.map((tech) => (tech.id === selectedId && draft ? { ...tech, ...toUpdate(draft) } : tech)),
    [draft, selectedId, technologies],
  );
  const inFolder = React.useMemo(
    () => shown.filter((tech) => tech.folders.some((entry) => entry.name === shownFolder)),
    [shown, shownFolder],
  );
  const spriteNames = React.useMemo(() => inFolder.map((tech) => techSprite(tech.id)), [inFolder]);
  const icons = useSpriteIcons(project?.folderPath, spriteNames);

  const nodes = React.useMemo<TreeNode[]>(
    () =>
      inFolder.map((tech) => {
        const place = tech.folders.find((entry) => entry.name === shownFolder) ?? { x: '0', y: '0' };
        return {
          id: tech.id,
          x: resolveCoordinate(place.x, variables),
          y: resolveCoordinate(place.y, variables),
          label: tech.id,
          sublabel: tech.researchCost
            ? `cost ${tech.researchCost}${tech.startYear ? ` · ${tech.startYear}` : ''}`
            : tech.startYear,
          icon: icons.get(techSprite(tech.id)),
          title: `${tech.id} · x ${place.x} y ${place.y}`,
        };
      }),
    [icons, inFolder, shownFolder, variables],
  );

  const edges = React.useMemo<TreeEdge[]>(() => {
    const ids = new Set(inFolder.map((tech) => tech.id));
    const lines: TreeEdge[] = [];
    for (const tech of inFolder) {
      for (const path of tech.paths) {
        if (ids.has(path.leadsToTech)) {
          lines.push({
            key: `${tech.id}->${path.leadsToTech}`,
            from: tech.id,
            to: path.leadsToTech,
            kind: 'prerequisite',
          });
        }
      }
      for (const other of tech.xor) {
        // Each pair is drawn once, from the id that sorts first.
        if (ids.has(other) && other > tech.id) {
          lines.push({ key: `${tech.id}<->${other}`, from: tech.id, to: other, kind: 'exclusive' });
        }
      }
    }
    return lines;
  }, [inFolder]);

  const { adopt: adoptFile, show } = doc;

  /** Shows a file the backend handed back, selecting `techId`. */
  const applyFile = React.useCallback(
    (loaded: TechnologyFile, techId: string | null) => {
      setFile(loaded);
      show(loaded);
      setSelectedId(techId);
      const tech = loaded.technologies.find((entry) => entry.id === techId);
      setDraft(tech ? toDraft(toTechnologyUpdate(tech)) : null);
      setIsDirty(false);
    },
    [show],
  );

  async function confirmLeavingEdits(): Promise<boolean> {
    if (isDirty && !(await confirmDiscard('Discard unsaved technology changes?'))) return false;
    if (sourceDirty && !(await confirmDiscard('Discard unsaved changes to the file text?'))) return false;
    return true;
  }

  async function openFile(entry: ProjectFile) {
    if (!(await confirmLeavingEdits())) return;

    const request = ++openRequest.current;
    setSelectedFile(entry);
    setIsLoading(true);
    try {
      const loaded = await api.readTechnologyFile(entry.fullPath);
      if (openRequest.current !== request) return;

      adoptFile(loaded);
      applyFile(loaded, loaded.technologies[0]?.id ?? null);
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
      void api.readTechnologyFile(openPath).then((loaded) => {
        if (cancelled || openRequest.current !== request) return;
        adoptFile(loaded);
        applyFile(
          loaded,
          loaded.technologies.some((entry) => entry.id === selectedId)
            ? selectedId
            : (loaded.technologies[0]?.id ?? null),
        );
        toast.message('Reloaded: changed by an MCP client');
      });
    });

    return () => {
      cancelled = true;
      void subscription.then((unlisten) => unlisten());
    };
  }, [adoptFile, applyFile, openPath, isDirty, sourceDirty, selectedId]);

  async function selectTechnology(id: string) {
    if (id === selectedId) return;
    if (isDirty && !(await confirmDiscard('Discard unsaved technology changes?'))) return;

    const tech = technologies.find((entry) => entry.id === id);
    setSelectedId(id);
    setDraft(tech ? toDraft(toTechnologyUpdate(tech)) : null);
    setIsDirty(false);
    // The code view jumps to the technology, so the two stay in step.
    if (tech) setReveal((current) => ({ line: tech.line, key: (current?.key ?? 0) + 1 }));
  }

  function patch(changes: Partial<TechDraft>) {
    setDraft((current) => (current ? { ...current, ...changes } : current));
    setIsDirty(true);
  }

  /** Dragging on the canvas edits the technology that was dragged, on the shown folder. */
  async function moveTechnology(id: string, x: number, y: number) {
    let base = draft;
    if (id !== selectedId) {
      if (isDirty && !(await confirmDiscard('Discard unsaved technology changes?'))) return;
      const tech = technologies.find((entry) => entry.id === id);
      if (!tech) return;
      base = toDraft(toTechnologyUpdate(tech));
      setSelectedId(id);
    }
    if (!base) return;

    const folders = base.folders.map((entry) =>
      entry.name === shownFolder
        ? { ...entry, x: spellCoordinate(x, entry.x, variables), y: spellCoordinate(y, entry.y, variables) }
        : entry,
    );
    setDraft({ ...base, folders });
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
    operation: () => Promise<TechnologyFile>,
    techId: (updated: TechnologyFile) => string | null,
    done: string,
  ) {
    if (!selectedFile) return;

    setIsSaving(true);
    try {
      const updated = await operation();
      const persisted = formCommits;
      if (persisted) await writeFile(updated.source);
      applyFile(updated, techId(updated));
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
    const change = toUpdate(draft);

    await commit(
      () => api.updateTechnologySource(path, source, doc.hasBom, doc.encoding, id, change),
      // The id itself may have changed, so follow the draft rather than the old id.
      () => change.id.trim() || id,
      change.id.trim() || id,
    );
  }

  async function remove() {
    if (!selectedFile || !selectedId) return;
    if (!(await confirmDelete(`Delete technology "${selectedId}" from ${selectedFile.name}?`))) {
      return;
    }
    const path = selectedFile.fullPath;
    const id = selectedId;

    await commit(
      () => api.deleteTechnologySource(path, source, doc.hasBom, doc.encoding, id),
      (updated) => updated.technologies[0]?.id ?? null,
      id,
    );
  }

  async function addTechnology(technology: TechnologyUpdate) {
    if (!selectedFile) return;
    const path = selectedFile.fullPath;

    await commit(
      () => api.addTechnologySource(path, source, doc.hasBom, doc.encoding, technology),
      () => technology.id,
      technology.id,
    );
  }

  function showEffectsInCode() {
    if (view === 'visual') chooseView('split');
    if (selected) setReveal((current) => ({ line: selected.line, key: (current?.key ?? 0) + 1 }));
  }

  return (
    <div className='grid h-full grid-cols-[15rem_minmax(0,1fr)_21rem] gap-3 p-3'>
      <div className='grid min-h-0 grid-rows-[2fr_3fr] gap-3'>
        <Panel>
          <PanelHeader title='Technology files' subtitle={`${files.length} in ${TECH_FOLDER}`} />
          <PanelBody>
            {files.length === 0 ? (
              <EmptyState
                title='No technology files'
                description={`This mod has no .txt files under ${TECH_FOLDER}; the game's own trees apply.`}
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
            title='Technologies'
            subtitle={file ? `${technologies.length} in this file` : 'No file open'}
            actions={
              file ? (
                <Button variant='ghost' size='icon-sm' title='Add technology' onClick={() => setIsAdding(true)}>
                  <Plus />
                </Button>
              ) : null
            }
          />
          <PanelBody>
            {technologies.length === 0 ? (
              <EmptyState title='Nothing to show' description='Open a technology file first.' />
            ) : (
              <ul className='py-1'>
                {technologies.map((tech) => (
                  <li key={`${tech.id}-${tech.line}`}>
                    <ListRow
                      active={tech.id === selectedId}
                      onClick={() => void selectTechnology(tech.id)}
                      title={`line ${tech.line}`}
                      className='py-1'
                    >
                      <span className='flex min-w-0 flex-1 flex-col'>
                        <span className='truncate font-mono text-xs'>{tech.id}</span>
                        <span className='truncate text-[0.6875rem] text-muted'>
                          {tech.folders.map((entry) => entry.name.replace(/_folder$/, '')).join(', ') || 'on no folder'}
                        </span>
                      </span>
                      {tech.doctrine === 'yes' ? (
                        <Badge tone='outline' className='shrink-0'>
                          doctrine
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
          title={file ? selectedFile?.name : 'Technology tree'}
          subtitle={
            file
              ? folders.length
                ? `${inFolder.length} on ${shownFolder}`
                : 'No technology here sits on a folder'
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
              {folders.length > 1 ? (
                <Select value={shownFolder} onValueChange={setFolder}>
                  <SelectTrigger className='h-7 w-44 text-xs' title='Which tree folder to draw'>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {folders.map((name) => (
                      <SelectItem key={name} value={name}>
                        {name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              ) : null}
              <ViewToggle value={view} onChange={chooseView} visual={{ label: 'Tree', icon: LayoutTemplate }} />
            </>
          }
        />
        <PanelBody className='overflow-hidden p-0'>
          {isLoading ? (
            <div className='flex h-full items-center justify-center gap-2 text-sm text-muted'>
              <Loader2 className='size-4 animate-spin' />
              Reading technologies…
            </div>
          ) : !file || !selectedFile ? (
            <EmptyState
              icon={<FlaskConical />}
              title='No technology tree loaded'
              description='Pick a file from common/technologies to see its technologies laid out on the game grid.'
            />
          ) : (
            <div className={cn('grid h-full', view === 'split' ? 'grid-cols-2' : 'grid-cols-1')}>
              {view !== 'code' ? (
                inFolder.length === 0 ? (
                  <EmptyState
                    icon={<FlaskConical />}
                    title='Nothing on this folder'
                    description='Add a technology with the + button, or give one a folder entry in the form.'
                  />
                ) : (
                  <div className={cn('min-h-0', view === 'split' && 'border-r border-border')}>
                    <TreeCanvas
                      nodes={nodes}
                      edges={edges}
                      selectedId={selectedId}
                      onSelect={(id) => void selectTechnology(id)}
                      onMove={(id, x, y) => void moveTechnology(id, x, y)}
                      noun='technology'
                    />
                  </div>
                )
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

      <Panel>
        <PanelHeader
          title={selected ? 'Technology' : 'Nothing selected'}
          subtitle={selected ? `line ${selected.line}` : undefined}
          actions={
            selected ? (
              <>
                {isDirty ? <Badge tone='accent'>Unsaved</Badge> : null}
                <Button variant='ghost' size='icon-sm' title='Delete technology' onClick={() => void remove()}>
                  <Trash2 />
                </Button>
                <Button
                  variant='primary'
                  size='sm'
                  onClick={() => void save()}
                  disabled={!isDirty || isSaving}
                  title={
                    formCommits
                      ? 'Write this technology to the file'
                      : 'Put this technology into the file text; Save file writes it'
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
          {!draft || !selected ? (
            <EmptyState
              title='Select a technology'
              description='Pick one from the list or the tree to edit its fields.'
            />
          ) : (
            <TechnologyForm
              draft={draft}
              effects={selected.effects}
              techIds={technologies.map((tech) => tech.id)}
              folders={folders}
              onChange={patch}
              onShowEffects={showEffectsInCode}
            />
          )}
        </PanelBody>
      </Panel>

      {/* Mounted only while open so its fields start fresh every time. */}
      {isAdding && file ? (
        <AddTechnologyDialog
          folder={shownFolder}
          onOpenChange={setIsAdding}
          onAdd={async (technology) => {
            await addTechnology(technology);
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

function TechnologyForm({
  draft,
  effects,
  techIds,
  folders,
  onChange,
  onShowEffects,
}: {
  draft: TechDraft;
  /** The modifiers the technology grants, read here and edited in the code view. */
  effects: string;
  techIds: string[];
  folders: string[];
  onChange: (changes: Partial<TechDraft>) => void;
  onShowEffects: () => void;
}) {
  const techListId = React.useId();
  const folderListId = React.useId();

  return (
    <div className='flex flex-col gap-4'>
      <Field label='Id' hint='The key of the block'>
        <CodeInput value={draft.id} onChange={(event) => onChange({ id: event.target.value })} />
      </Field>
      <div className='grid grid-cols-2 gap-3'>
        <Field label='Research cost'>
          <CodeInput
            value={draft.researchCost}
            placeholder='1'
            onChange={(event) => onChange({ researchCost: event.target.value })}
          />
        </Field>
        <Field label='Start year'>
          <CodeInput
            value={draft.startYear}
            placeholder='1936'
            onChange={(event) => onChange({ startYear: event.target.value })}
          />
        </Field>
      </div>

      <RowList
        title='Folders'
        addLabel='Add'
        hint='Where the technology sits: a tree folder and a grid position; y is often a @year the file declares.'
        rows={draft.folders}
        onChange={(entries) => onChange({ folders: entries })}
        create={() => ({ name: folders[0] ?? '', x: '0', y: '0' })}
        render={(entry: TechFolder, update) => (
          <>
            <CodeInput
              list={folderListId}
              value={entry.name}
              placeholder='infantry_folder'
              onChange={(event) => update({ name: event.target.value })}
            />
            <CodeInput
              value={entry.x}
              placeholder='x'
              className='w-16 shrink-0'
              onChange={(event) => update({ x: event.target.value })}
            />
            <CodeInput
              value={entry.y}
              placeholder='y'
              className='w-20 shrink-0'
              onChange={(event) => update({ y: event.target.value })}
            />
          </>
        )}
      />
      <datalist id={folderListId}>
        {folders.map((name) => (
          <option key={name} value={name} />
        ))}
      </datalist>

      <RowList
        title='Leads to'
        addLabel='Add'
        hint='The technologies this one unlocks, with a cost coefficient each.'
        rows={draft.paths}
        onChange={(entries) => onChange({ paths: entries })}
        create={() => ({ leadsToTech: '', researchCostCoeff: '1' })}
        render={(entry: TechPath, update) => (
          <>
            <CodeInput
              list={techListId}
              value={entry.leadsToTech}
              placeholder='next_technology'
              onChange={(event) => update({ leadsToTech: event.target.value })}
            />
            <CodeInput
              value={entry.researchCostCoeff}
              placeholder='1'
              className='w-16 shrink-0'
              title='research_cost_coeff'
              onChange={(event) => update({ researchCostCoeff: event.target.value })}
            />
          </>
        )}
      />
      <datalist id={techListId}>
        {techIds.map((id) => (
          <option key={id} value={id} />
        ))}
      </datalist>

      {LIST_KEYS.map((entry) => (
        <Field key={entry.key} label={entry.label} hint={entry.hint || 'Separated by spaces'}>
          <CodeInput value={draft[entry.key]} onChange={(event) => onChange({ [entry.key]: event.target.value })} />
        </Field>
      ))}

      <div className='space-y-2 rounded-lg border border-border p-3'>
        <Label>Doctrine</Label>
        <div className='grid grid-cols-2 gap-3'>
          <Field label='Is a doctrine'>
            <Select
              value={draft.doctrine || 'unset'}
              onValueChange={(value) => onChange({ doctrine: value === 'unset' ? '' : value })}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value='unset'>No</SelectItem>
                <SelectItem value='yes'>yes</SelectItem>
              </SelectContent>
            </Select>
          </Field>
          <Field label='XP type'>
            <Select
              value={draft.xpResearchType || 'unset'}
              onValueChange={(value) => onChange({ xpResearchType: value === 'unset' ? '' : value })}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value='unset'>None</SelectItem>
                {XP_TYPES.map((type) => (
                  <SelectItem key={type} value={type}>
                    {type}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>
          <Field label='XP cost'>
            <CodeInput
              value={draft.xpResearchCost}
              onChange={(event) => onChange({ xpResearchCost: event.target.value })}
            />
          </Field>
          <Field label='XP bonus'>
            <CodeInput
              value={draft.xpResearchBonus}
              onChange={(event) => onChange({ xpResearchBonus: event.target.value })}
            />
          </Field>
        </div>
        <Field label='Doctrine name' hint='Localisation key of the doctrine tree'>
          <CodeInput value={draft.doctrineName} onChange={(event) => onChange({ doctrineName: event.target.value })} />
        </Field>
      </div>

      <BlockField
        label='Allow'
        value={draft.allow}
        onChange={(allow) => onChange({ allow })}
        placeholder='always = yes'
      />
      <BlockField
        label='Allow branch'
        value={draft.allowBranch}
        onChange={(allowBranch) => onChange({ allowBranch })}
        placeholder='has_dlc = "No Step Back"'
      />
      <BlockField
        label='On research complete'
        value={draft.onResearchComplete}
        onChange={(onResearchComplete) => onChange({ onResearchComplete })}
        placeholder='add_tech_bonus = { ... }'
      />
      <BlockField
        label='AI will do'
        value={draft.aiWillDo}
        onChange={(aiWillDo) => onChange({ aiWillDo })}
        placeholder='factor = 1'
      />
      <BlockField
        label='AI research weights'
        value={draft.aiResearchWeights}
        onChange={(aiResearchWeights) => onChange({ aiResearchWeights })}
        placeholder='mobile_warfare_focus = 1'
      />

      <details open={effects.trim().length > 0} className='group'>
        <summary className='cursor-pointer list-none text-xs font-medium uppercase tracking-wide text-muted marker:content-none hover:text-foreground'>
          Effects{effects.trim() ? '' : ' · none'}
        </summary>
        <Textarea className='mt-1.5 opacity-80' rows={6} value={effects} readOnly />
        <p className='mt-1 text-xs text-muted'>
          What the technology grants: unit and equipment modifiers, buildings, tactics. Open-ended script, so it is
          edited in the code view.{' '}
          <button
            type='button'
            className='inline-flex items-center gap-1 text-accent hover:underline'
            onClick={onShowEffects}
          >
            <Code2 className='size-3' />
            Open in code
          </button>
        </p>
      </details>
    </div>
  );
}

/** A list of small rows, one line each, added and removed at will. */
function RowList<T>({
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

function AddTechnologyDialog({
  folder,
  onOpenChange,
  onAdd,
}: {
  /** The folder the canvas shows, which the new technology joins. */
  folder: string;
  onOpenChange: (open: boolean) => void;
  /** Adds the technology; the page decides whether that also writes the file. */
  onAdd: (technology: TechnologyUpdate) => Promise<void>;
}) {
  const [id, setId] = React.useState('');
  const [isSaving, setIsSaving] = React.useState(false);

  async function create() {
    const trimmed = id.trim();
    if (!trimmed) return;

    setIsSaving(true);
    try {
      // A stub the tree will show: a cost, a place on the shown folder, and
      // a weight so the AI considers it. Give it a category and effects on
      // the right.
      await onAdd({
        ...emptyTechnologyUpdate(),
        id: trimmed,
        researchCost: '1',
        startYear: '1936',
        folders: folder ? [{ name: folder, x: '0', y: '0' }] : [],
        aiWillDo: 'factor = 1',
      });
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <Dialog open onOpenChange={onOpenChange}>
      <DialogContent
        title='Add a technology'
        description={
          folder
            ? `A stub is appended to the file and placed at 0, 0 on ${folder}; fill in the rest on the right.`
            : 'A stub is appended to the file; give it a folder on the right to see it on the tree.'
        }
      >
        <DialogBody className='flex flex-col gap-3'>
          <Field label='Technology id' hint='Letters, digits and underscores, e.g. infantry_weapons4'>
            <CodeInput
              value={id}
              autoFocus
              placeholder='infantry_weapons4'
              onChange={(event) => setId(event.target.value)}
            />
          </Field>
        </DialogBody>
        <DialogFooter>
          <DialogClose asChild>
            <Button>Cancel</Button>
          </DialogClose>
          <Button variant='primary' onClick={() => void create()} disabled={!id.trim() || isSaving}>
            {isSaving ? <Loader2 className='animate-spin' /> : <Plus />}
            Add technology
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
