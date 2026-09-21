'use client';

import * as React from 'react';
import { listen } from '@tauri-apps/api/event';
import { ClipboardList, FileText, ImageOff, Loader2, Plus, Save, Trash2, UsersRound, X } from 'lucide-react';
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
import { useSourceDocument } from '@/lib/use-source-document';
import { useSpriteIcons, type SpriteMap } from '@/lib/use-sprite-icons';
import { useUnsavedIn } from '@/lib/use-unsaved';
import {
  emptyAdvisor,
  emptyCharacterUpdate,
  emptyCommander,
  emptyCountryLeader,
  toCharacterUpdate,
  type Advisor,
  type Character,
  type CharacterFile,
  type CharacterUpdate,
  type Commander,
  type CommanderKind,
  type CountryLeader,
  type Portraits,
  type ProjectFile,
  type SpriteIcon,
} from '@/lib/types';
import { cn, isInFolder } from '@/lib/utils';

const CHARACTER_FOLDER = 'common/characters';

/** Sent by the backend when an MCP client writes a file. */
const FILE_CHANGED_EVENT = 'project://file-changed';

/** Where the panel's layout choice is remembered. */
const VIEW_KEY = 'hoi4ms.characters-view';

const COMMANDER_KINDS: Array<{ value: CommanderKind; label: string }> = [
  { value: 'corps_commander', label: 'General' },
  { value: 'field_marshal', label: 'Field marshal' },
  { value: 'navy_leader', label: 'Admiral' },
];

/**
 * The form's copy of a character. Word lists are edited as one text, so the
 * space being typed between two traits is not swallowed by a split.
 */
type LeaderDraft = Omit<CountryLeader, 'traits'> & { traits: string };
type AdvisorDraft = Omit<Advisor, 'traits'> & { traits: string };
type CommanderDraft = Omit<Commander, 'traits'> & { traits: string };
interface CharacterDraft extends Omit<CharacterUpdate, 'leaders' | 'advisors' | 'commanders'> {
  leaders: LeaderDraft[];
  advisors: AdvisorDraft[];
  commanders: CommanderDraft[];
}

function toDraft(update: CharacterUpdate): CharacterDraft {
  return {
    ...update,
    leaders: update.leaders.map((role) => ({ ...role, traits: role.traits.join(' ') })),
    advisors: update.advisors.map((role) => ({ ...role, traits: role.traits.join(' ') })),
    commanders: update.commanders.map((role) => ({ ...role, traits: role.traits.join(' ') })),
  };
}

function toUpdate(draft: CharacterDraft): CharacterUpdate {
  const words = (text: string) => text.split(/\s+/).filter(Boolean);
  return {
    ...draft,
    leaders: draft.leaders.map((role) => ({ ...role, traits: words(role.traits) })),
    advisors: draft.advisors.map((role) => ({ ...role, traits: words(role.traits) })),
    commanders: draft.commanders.map((role) => ({ ...role, traits: words(role.traits) })),
  };
}

/** The picture that best stands for a character in a list. */
function thumbnailOf(portraits: Portraits): string {
  return (
    portraits.civilianSmall ||
    portraits.armySmall ||
    portraits.navySmall ||
    portraits.civilianLarge ||
    portraits.armyLarge ||
    portraits.navyLarge
  );
}

function rolesOf(character: Character): string {
  const roles: string[] = [];
  if (character.leaders.length) roles.push('leader');
  if (character.advisors.length)
    roles.push(character.advisors.length > 1 ? 'advisor ×' + character.advisors.length : 'advisor');
  for (const commander of character.commanders) {
    roles.push(COMMANDER_KINDS.find((kind) => kind.value === commander.kind)?.label.toLowerCase() ?? commander.kind);
  }
  return roles.join(' · ');
}

/**
 * The character editor: the people a country recruits as leaders, advisors
 * and commanders, each edited in a form or in the file's text, like the
 * focus editor.
 */
export default function CharactersPage() {
  const { project, scan, setError } = useAppStore();

  const [selectedFile, setSelectedFile] = React.useState<ProjectFile | null>(null);
  const [file, setFile] = React.useState<CharacterFile | null>(null);
  const [selectedId, setSelectedId] = React.useState<string | null>(null);
  const [draft, setDraft] = React.useState<CharacterDraft | null>(null);
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
      (scan?.files ?? []).filter(
        (entry) => isInFolder(entry.relativePath, CHARACTER_FOLDER) && entry.extension === 'txt',
      ),
    [scan],
  );

  const characters = React.useMemo(() => file?.characters ?? [], [file]);
  const selected = characters.find((character) => character.id === selectedId) ?? null;

  const latest = React.useRef({ isDirty, selectedId });
  React.useEffect(() => {
    latest.current = { isDirty, selectedId };
  });

  const openPath = selectedFile?.fullPath;
  const doc = useSourceDocument<CharacterFile>({
    path: openPath,
    parse: api.parseCharacterSource,
    onParsed: (parsed) => {
      const current = latest.current.selectedId;
      const stillThere = current !== null && parsed.characters.some((character) => character.id === current);
      const nextId = stillThere ? current : (parsed.characters[0]?.id ?? null);

      setFile(parsed);
      setSelectedId(nextId);
      // A form the user is mid-way through editing keeps its draft; an
      // untouched one follows the new text.
      if (!latest.current.isDirty) {
        const character = parsed.characters.find((entry) => entry.id === nextId);
        setDraft(character ? toDraft(toCharacterUpdate(character)) : null);
      }
    },
  });
  const { source, setSource, isDirty: sourceDirty, parseError } = doc;

  // With the code hidden and the text untouched, the form's Save writes the
  // file at once. Once the text has edits of its own the form can only
  // apply: writing would take those along uninvited.
  const formCommits = view === 'visual' && !sourceDirty;

  useUnsavedIn('character editor', isDirty || sourceDirty);

  // The list shows one picture per character; the form shows all six of the
  // selected one, as they are being typed.
  const spriteNames = React.useMemo(() => {
    const names = characters.map((character) => thumbnailOf(character.portraits));
    if (draft) names.push(...Object.values(draft.portraits));
    return names;
  }, [characters, draft]);
  const icons = useSpriteIcons(project?.folderPath, spriteNames);

  const { adopt: adoptFile, show } = doc;

  /** Shows a file the backend handed back, selecting `characterId`. */
  const applyFile = React.useCallback(
    (loaded: CharacterFile, characterId: string | null) => {
      setFile(loaded);
      show(loaded);
      setSelectedId(characterId);
      const character = loaded.characters.find((entry) => entry.id === characterId);
      setDraft(character ? toDraft(toCharacterUpdate(character)) : null);
      setIsDirty(false);
    },
    [show],
  );

  async function confirmLeavingEdits(): Promise<boolean> {
    if (isDirty && !(await confirmDiscard('Discard unsaved character changes?'))) return false;
    if (sourceDirty && !(await confirmDiscard('Discard unsaved changes to the file text?'))) return false;
    return true;
  }

  async function openFile(entry: ProjectFile) {
    if (!(await confirmLeavingEdits())) return;

    const request = ++openRequest.current;
    setSelectedFile(entry);
    setIsLoading(true);
    try {
      const loaded = await api.readCharacterFile(entry.fullPath);
      if (openRequest.current !== request) return;

      adoptFile(loaded);
      applyFile(loaded, loaded.characters[0]?.id ?? null);
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
      void api.readCharacterFile(openPath).then((loaded) => {
        if (cancelled || openRequest.current !== request) return;
        adoptFile(loaded);
        applyFile(
          loaded,
          loaded.characters.some((character) => character.id === selectedId)
            ? selectedId
            : (loaded.characters[0]?.id ?? null),
        );
        toast.message('Reloaded: changed by an MCP client');
      });
    });

    return () => {
      cancelled = true;
      void subscription.then((unlisten) => unlisten());
    };
  }, [adoptFile, applyFile, openPath, isDirty, sourceDirty, selectedId]);

  async function selectCharacter(id: string) {
    if (id === selectedId) return;
    if (isDirty && !(await confirmDiscard('Discard unsaved character changes?'))) return;

    const character = characters.find((entry) => entry.id === id);
    setSelectedId(id);
    setDraft(character ? toDraft(toCharacterUpdate(character)) : null);
    setIsDirty(false);
    // The code view jumps to the character, so the two stay in step.
    if (character) setReveal((current) => ({ line: character.line, key: (current?.key ?? 0) + 1 }));
  }

  function patch(changes: Partial<CharacterDraft>) {
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
    operation: () => Promise<CharacterFile>,
    characterId: (updated: CharacterFile) => string | null,
    done: string,
  ) {
    if (!selectedFile) return;

    setIsSaving(true);
    try {
      const updated = await operation();
      const persisted = formCommits;
      if (persisted) await writeFile(updated.source);
      applyFile(updated, characterId(updated));
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
      () => api.updateCharacterSource(path, source, doc.hasBom, doc.encoding, id, change),
      // The id itself may have changed, so follow the draft rather than the old id.
      () => change.id.trim() || id,
      change.id.trim() || id,
    );
  }

  async function remove() {
    if (!selectedFile || !selectedId) return;
    if (!(await confirmDelete(`Delete character "${selectedId}" from ${selectedFile.name}?`))) {
      return;
    }
    const path = selectedFile.fullPath;
    const id = selectedId;

    await commit(
      () => api.deleteCharacterSource(path, source, doc.hasBom, doc.encoding, id),
      (updated) => updated.characters[0]?.id ?? null,
      id,
    );
  }

  async function addCharacter(character: CharacterUpdate) {
    if (!selectedFile) return;
    const path = selectedFile.fullPath;

    await commit(
      () => api.addCharacterSource(path, source, doc.hasBom, doc.encoding, character),
      () => character.id,
      character.id,
    );
  }

  const form =
    !draft || !selected ? (
      <EmptyState
        icon={<UsersRound />}
        title='Select a character'
        description='Pick one from the list to edit their name, portraits and roles.'
      />
    ) : selected.hasInstances ? (
      <EmptyState
        icon={<ClipboardList />}
        title='Defined through instance blocks'
        description={`${selected.id} has one definition per DLC set-up. The form cannot tell which one you mean, so edit it in the code view.`}
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
            <Button variant='ghost' size='icon-sm' title='Delete character' onClick={() => void remove()}>
              <Trash2 />
            </Button>
            <Button
              variant='primary'
              size='sm'
              onClick={() => void save()}
              disabled={!isDirty || isSaving}
              title={
                formCommits
                  ? 'Write this character to the file'
                  : 'Put this character into the file text; Save file writes it'
              }
            >
              {isSaving ? <Loader2 className='animate-spin' /> : <Save />}
              {formCommits ? 'Save' : 'Apply'}
            </Button>
          </div>
        </div>
        <div className='min-h-0 flex-1 overflow-auto p-3'>
          <CharacterForm draft={draft} icons={icons} onChange={patch} />
        </div>
      </div>
    );

  return (
    <div className='grid h-full grid-cols-[16rem_minmax(0,1fr)] gap-3 p-3'>
      <div className='grid min-h-0 grid-rows-[2fr_3fr] gap-3'>
        <Panel>
          <PanelHeader title='Character files' subtitle={`${files.length} in ${CHARACTER_FOLDER}`} />
          <PanelBody>
            {files.length === 0 ? (
              <EmptyState
                title='No character files'
                description={`This mod has no .txt files under ${CHARACTER_FOLDER}.`}
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
            title='Characters'
            subtitle={file ? `${characters.length} in this file` : 'No file open'}
            actions={
              file ? (
                <Button variant='ghost' size='icon-sm' title='Add character' onClick={() => setIsAdding(true)}>
                  <Plus />
                </Button>
              ) : null
            }
          />
          <PanelBody>
            {characters.length === 0 ? (
              <EmptyState title='Nothing to show' description='Open a character file first.' />
            ) : (
              <ul className='py-1'>
                {characters.map((character) => (
                  <li key={`${character.id}-${character.line}`}>
                    <ListRow
                      active={character.id === selectedId}
                      onClick={() => void selectCharacter(character.id)}
                      title={`line ${character.line}`}
                      className='py-1'
                    >
                      <Thumbnail icon={icons.get(thumbnailOf(character.portraits))} />
                      <span className='flex min-w-0 flex-col'>
                        <span className='truncate font-mono text-xs'>{character.id}</span>
                        <span className='truncate text-[0.6875rem] text-muted'>
                          {character.hasInstances ? 'instances' : rolesOf(character) || 'no roles'}
                        </span>
                      </span>
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
          title={file ? selectedFile?.name : 'Characters'}
          subtitle={file ? `${characters.length} characters` : 'Open a file to edit its characters'}
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
              Reading characters…
            </div>
          ) : !file || !selectedFile ? (
            <EmptyState
              icon={<UsersRound />}
              title='No character file open'
              description='Pick a file from common/characters to see the people in it.'
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

      {/* Mounted only while open so its fields start empty every time. */}
      {isAdding && file ? (
        <AddCharacterDialog
          onOpenChange={setIsAdding}
          onAdd={async (character) => {
            await addCharacter(character);
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

function Thumbnail({ icon }: { icon: SpriteIcon | undefined }) {
  return (
    <span className='flex size-7 shrink-0 items-center justify-center overflow-hidden rounded border border-border bg-surface-sunken'>
      {icon?.url ? (
        // Data URL produced by the backend, so next/image cannot help here.
        // oxlint-disable-next-line next/no-img-element
        <img src={icon.url} alt='' className='size-full object-cover object-top' />
      ) : (
        <ImageOff className='size-3 text-border-strong' />
      )}
    </span>
  );
}

function CharacterForm({
  draft,
  icons,
  onChange,
}: {
  draft: CharacterDraft;
  icons: SpriteMap;
  onChange: (changes: Partial<CharacterDraft>) => void;
}) {
  return (
    <div className='@container flex flex-col gap-4'>
      <div className='grid gap-3 @xl:grid-cols-3'>
        <Field label='Id' hint='The key of the block; what recruit_character names'>
          <CodeInput value={draft.id} onChange={(event) => onChange({ id: event.target.value })} />
        </Field>
        <Field label='Name' hint='Localisation key'>
          <CodeInput value={draft.name} onChange={(event) => onChange({ name: event.target.value })} />
        </Field>
        <Field label='Gender'>
          <Select
            value={draft.gender || 'default'}
            onValueChange={(value) => onChange({ gender: value === 'default' ? '' : value })}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value='default'>Not set (male)</SelectItem>
              <SelectItem value='female'>female</SelectItem>
              <SelectItem value='male'>male</SelectItem>
            </SelectContent>
          </Select>
        </Field>
      </div>

      <PortraitFields portraits={draft.portraits} icons={icons} onChange={(portraits) => onChange({ portraits })} />

      <RoleList
        title='Country leader'
        addLabel='Add leader role'
        roles={draft.leaders}
        onChange={(leaders) => onChange({ leaders })}
        create={() => ({ ...emptyCountryLeader(), traits: '' })}
        render={(role, update) => (
          <>
            <div className='grid gap-3 @lg:grid-cols-2'>
              <Field label='Ideology'>
                <CodeInput value={role.ideology} onChange={(event) => update({ ideology: event.target.value })} />
              </Field>
              <Field label='Expire' hint='Date, e.g. 1965.1.1.1'>
                <CodeInput value={role.expire} onChange={(event) => update({ expire: event.target.value })} />
              </Field>
              <Field label='Description' hint='Localisation key, optional'>
                <CodeInput value={role.desc} onChange={(event) => update({ desc: event.target.value })} />
              </Field>
              <Field label='Traits' hint='Separated by spaces'>
                <CodeInput value={role.traits} onChange={(event) => update({ traits: event.target.value })} />
              </Field>
            </div>
          </>
        )}
      />

      <RoleList
        title='Advisor'
        addLabel='Add advisor role'
        roles={draft.advisors}
        onChange={(advisors) => onChange({ advisors })}
        create={() => ({ ...emptyAdvisor(), traits: '' })}
        render={(role, update) => (
          <>
            <div className='grid gap-3 @lg:grid-cols-3'>
              <Field label='Slot' hint='political_advisor, theorist, high_command …'>
                <CodeInput value={role.slot} onChange={(event) => update({ slot: event.target.value })} />
              </Field>
              <Field label='Idea token' hint='Unique across the mod'>
                <CodeInput value={role.ideaToken} onChange={(event) => update({ ideaToken: event.target.value })} />
              </Field>
              <Field label='Ledger' hint='army, navy, air, civilian, hidden'>
                <CodeInput value={role.ledger} onChange={(event) => update({ ledger: event.target.value })} />
              </Field>
              <Field label='Cost'>
                <CodeInput value={role.cost} onChange={(event) => update({ cost: event.target.value })} />
              </Field>
              <Field label='Removal cost'>
                <CodeInput value={role.removalCost} onChange={(event) => update({ removalCost: event.target.value })} />
              </Field>
              <Field label='Can be fired' hint='yes or no'>
                <CodeInput value={role.canBeFired} onChange={(event) => update({ canBeFired: event.target.value })} />
              </Field>
            </div>
            <Field label='Traits' hint='Separated by spaces'>
              <CodeInput value={role.traits} onChange={(event) => update({ traits: event.target.value })} />
            </Field>
            <BlockField
              label='Allowed'
              value={role.allowed}
              onChange={(allowed) => update({ allowed })}
              placeholder='original_tag = GER'
            />
            <BlockField
              label='Available'
              value={role.available}
              onChange={(available) => update({ available })}
              placeholder='has_completed_focus = GER_rhineland'
            />
            <BlockField
              label='Visible'
              value={role.visible}
              onChange={(visible) => update({ visible })}
              placeholder='has_government = fascism'
            />
            <BlockField
              label='AI will do'
              value={role.aiWillDo}
              onChange={(aiWillDo) => update({ aiWillDo })}
              placeholder='factor = 1'
            />
          </>
        )}
      />

      <RoleList
        title='Commander'
        addLabel='Add commander role'
        roles={draft.commanders}
        onChange={(commanders) => onChange({ commanders })}
        create={() => ({ ...emptyCommander('corps_commander'), traits: '' })}
        render={(role, update) => {
          const navy = role.kind === 'navy_leader';
          return (
            <>
              <div className='grid gap-3 @md:grid-cols-2 @2xl:grid-cols-4'>
                <Field label='Role'>
                  <Select value={role.kind} onValueChange={(kind) => update({ kind: kind as CommanderKind })}>
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {COMMANDER_KINDS.map((kind) => (
                        <SelectItem key={kind.value} value={kind.value}>
                          {kind.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </Field>
                <Field label='Skill'>
                  <CodeInput value={role.skill} onChange={(event) => update({ skill: event.target.value })} />
                </Field>
                <Field label='Attack'>
                  <CodeInput
                    value={role.attackSkill}
                    onChange={(event) => update({ attackSkill: event.target.value })}
                  />
                </Field>
                <Field label='Defense'>
                  <CodeInput
                    value={role.defenseSkill}
                    onChange={(event) => update({ defenseSkill: event.target.value })}
                  />
                </Field>
                {navy ? (
                  <>
                    <Field label='Maneuvering'>
                      <CodeInput
                        value={role.maneuveringSkill}
                        onChange={(event) => update({ maneuveringSkill: event.target.value })}
                      />
                    </Field>
                    <Field label='Coordination'>
                      <CodeInput
                        value={role.coordinationSkill}
                        onChange={(event) => update({ coordinationSkill: event.target.value })}
                      />
                    </Field>
                  </>
                ) : (
                  <>
                    <Field label='Planning'>
                      <CodeInput
                        value={role.planningSkill}
                        onChange={(event) => update({ planningSkill: event.target.value })}
                      />
                    </Field>
                    <Field label='Logistics'>
                      <CodeInput
                        value={role.logisticsSkill}
                        onChange={(event) => update({ logisticsSkill: event.target.value })}
                      />
                    </Field>
                  </>
                )}
                <Field label='Legacy id' hint='Optional'>
                  <CodeInput value={role.legacyId} onChange={(event) => update({ legacyId: event.target.value })} />
                </Field>
              </div>
              <Field label='Traits' hint='Separated by spaces'>
                <CodeInput value={role.traits} onChange={(event) => update({ traits: event.target.value })} />
              </Field>
            </>
          );
        }}
      />

      <BlockField
        label='Allowed civil war'
        value={draft.allowedCivilWar}
        onChange={(allowedCivilWar) => onChange({ allowedCivilWar })}
        placeholder='has_government = fascism'
      />
    </div>
  );
}

const PORTRAIT_GROUPS: Array<{ label: string; large: keyof Portraits; small: keyof Portraits }> = [
  { label: 'Civilian', large: 'civilianLarge', small: 'civilianSmall' },
  { label: 'Army', large: 'armyLarge', small: 'armySmall' },
  { label: 'Navy', large: 'navyLarge', small: 'navySmall' },
];

function PortraitFields({
  portraits,
  icons,
  onChange,
}: {
  portraits: Portraits;
  icons: SpriteMap;
  onChange: (portraits: Portraits) => void;
}) {
  return (
    <div className='space-y-2 rounded-lg border border-border p-3'>
      <Label>Portraits</Label>
      <div className='grid gap-3 @lg:grid-cols-3'>
        {PORTRAIT_GROUPS.map((group) => (
          <div key={group.label} className='flex flex-col gap-2'>
            <span className='text-xs font-medium text-muted'>{group.label}</span>
            <PortraitInput
              label='Large'
              value={portraits[group.large]}
              icon={icons.get(portraits[group.large])}
              onChange={(value) => onChange({ ...portraits, [group.large]: value })}
            />
            <PortraitInput
              label='Small'
              value={portraits[group.small]}
              icon={icons.get(portraits[group.small])}
              onChange={(value) => onChange({ ...portraits, [group.small]: value })}
            />
          </div>
        ))}
      </div>
      <p className='text-xs text-muted'>
        Sprite names from interface/*.gfx. Leaders and advisors use the civilian pictures, commanders the army or navy
        ones; large is the leader portrait, small the advisor and commander icon.
      </p>
    </div>
  );
}

function PortraitInput({
  label,
  value,
  icon,
  onChange,
}: {
  label: string;
  value: string;
  icon: SpriteIcon | undefined;
  onChange: (value: string) => void;
}) {
  return (
    <div className='flex items-start gap-2'>
      <span
        className='flex h-12 w-9 shrink-0 items-center justify-center overflow-hidden rounded-md border border-border bg-surface-sunken'
        title={value ? (icon === undefined ? '' : icon.path || 'No interface/*.gfx file defines this sprite') : ''}
      >
        {icon?.url ? (
          // Data URL produced by the backend, so next/image cannot help here.
          // oxlint-disable-next-line next/no-img-element
          <img src={icon.url} alt='' className='size-full object-cover object-top' />
        ) : (
          <ImageOff className='size-3.5 text-border-strong' />
        )}
      </span>
      <Field label={label} className='min-w-0 flex-1'>
        <CodeInput value={value} placeholder='GFX_portrait_…' onChange={(event) => onChange(event.target.value)} />
      </Field>
    </div>
  );
}

/** A repeated role: one card per block in the file, in the file's order. */
function RoleList<T>({
  title,
  addLabel,
  roles,
  onChange,
  create,
  render,
}: {
  title: string;
  addLabel: string;
  roles: T[];
  onChange: (roles: T[]) => void;
  create: () => T;
  render: (role: T, update: (changes: Partial<T>) => void) => React.ReactNode;
}) {
  return (
    <div className='space-y-2 rounded-lg border border-border p-3'>
      <div className='flex items-center justify-between'>
        <Label>
          {title}
          {roles.length > 1 ? ` · ${roles.length}` : ''}
        </Label>
        <Button size='sm' onClick={() => onChange([...roles, create()])}>
          <Plus />
          {addLabel}
        </Button>
      </div>
      {roles.length === 0 ? <p className='text-xs text-muted'>None.</p> : null}
      {roles.map((role, index) => (
        <div
          key={index}
          className='relative flex flex-col gap-3 rounded-md border border-border bg-surface-raised/40 p-3'
        >
          <Button
            variant='ghost'
            size='icon-sm'
            title={`Remove this ${title.toLowerCase()} role`}
            className='absolute right-1.5 top-1.5'
            onClick={() => onChange(roles.filter((_, at) => at !== index))}
          >
            <X />
          </Button>
          {render(role, (changes) => {
            const next = [...roles];
            next[index] = { ...role, ...changes };
            onChange(next);
          })}
        </div>
      ))}
    </div>
  );
}

function AddCharacterDialog({
  onOpenChange,
  onAdd,
}: {
  onOpenChange: (open: boolean) => void;
  /** Adds the character; the page decides whether that also writes the file. */
  onAdd: (character: CharacterUpdate) => Promise<void>;
}) {
  const [id, setId] = React.useState('');
  const [isSaving, setIsSaving] = React.useState(false);

  async function create() {
    if (!id.trim()) return;

    setIsSaving(true);
    try {
      await onAdd({ ...emptyCharacterUpdate(), id: id.trim(), name: id.trim() });
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <Dialog open onOpenChange={onOpenChange}>
      <DialogContent
        title='Add a character'
        description='A stub with just a name is appended to the file; give it portraits and roles in the form. The country still has to recruit_character it in its history file.'
      >
        <DialogBody className='flex flex-col gap-3'>
          <Field label='Character id' hint='By convention the tag and a name: GER_erwin_rommel'>
            <CodeInput
              value={id}
              autoFocus
              placeholder='GER_erwin_rommel'
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
            Add character
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
