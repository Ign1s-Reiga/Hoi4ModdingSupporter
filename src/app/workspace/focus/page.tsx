"use client";

import * as React from "react";
import {
  FileText,
  FolderTree,
  Loader2,
  Plus,
  Save,
  Trash2,
  X,
} from "lucide-react";
import { toast } from "sonner";

import { FocusCanvas } from "@/components/focus-canvas";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogClose,
  DialogContent,
  DialogFooter,
} from "@/components/ui/dialog";
import { CodeInput, Field, Label, Textarea } from "@/components/ui/form";
import {
  Badge,
  EmptyState,
  ListRow,
  Panel,
  PanelBody,
  PanelHeader,
} from "@/components/ui/panel";
import { confirmDelete, confirmDiscard } from "@/lib/dialogs";
import { api, describeError } from "@/lib/ipc";
import { useAppStore } from "@/lib/store";
import {
  emptyFocusUpdate,
  toFocusUpdate,
  type FocusFile,
  type FocusUpdate,
  type ProjectFile,
} from "@/lib/types";
import { isInFolder } from "@/lib/utils";

const FOCUS_FOLDER = "common/national_focus";

export default function FocusPage() {
  const { scan, setError } = useAppStore();

  const [selectedFile, setSelectedFile] = React.useState<ProjectFile | null>(null);
  const [focusFile, setFocusFile] = React.useState<FocusFile | null>(null);
  const [selectedId, setSelectedId] = React.useState<string | null>(null);
  const [draft, setDraft] = React.useState<FocusUpdate | null>(null);
  const [isDirty, setIsDirty] = React.useState(false);
  const [isLoading, setIsLoading] = React.useState(false);
  const [isSaving, setIsSaving] = React.useState(false);
  const [isAdding, setIsAdding] = React.useState(false);

  const files = React.useMemo(
    () =>
      (scan?.files ?? []).filter(
        (file) => isInFolder(file.relativePath, FOCUS_FOLDER) && file.extension === "txt",
      ),
    [scan],
  );

  const focuses = focusFile?.focuses ?? [];
  const selected = focuses.find((focus) => focus.id === selectedId) ?? null;

  async function openFile(file: ProjectFile) {
    if (isDirty && !(await confirmDiscard("Discard unsaved focus changes?"))) return;

    setSelectedFile(file);
    setIsLoading(true);
    try {
      const loaded = await api.readFocusFile(file.fullPath);
      applyFile(loaded, loaded.focuses[0]?.id ?? null);
    } catch (error) {
      const message = describeError(error);
      setError(message);
      setFocusFile(null);
      setSelectedId(null);
      setDraft(null);
    } finally {
      setIsLoading(false);
    }
  }

  function applyFile(loaded: FocusFile, focusId: string | null) {
    setFocusFile(loaded);
    setSelectedId(focusId);
    const focus = loaded.focuses.find((entry) => entry.id === focusId);
    setDraft(focus ? toFocusUpdate(focus) : null);
    setIsDirty(false);
  }

  async function selectFocus(id: string) {
    if (id === selectedId) return;
    if (isDirty && !(await confirmDiscard("Discard unsaved focus changes?"))) return;

    const focus = focuses.find((entry) => entry.id === id);
    setSelectedId(id);
    setDraft(focus ? toFocusUpdate(focus) : null);
    setIsDirty(false);
  }

  function patch(changes: Partial<FocusUpdate>) {
    setDraft((current) => (current ? { ...current, ...changes } : current));
    setIsDirty(true);
  }

  /** Dragging on the canvas edits the focus that was dragged. */
  async function moveFocus(id: string, x: number, y: number) {
    if (id !== selectedId) {
      if (isDirty && !(await confirmDiscard("Discard unsaved focus changes?"))) return;
      const focus = focuses.find((entry) => entry.id === id);
      if (!focus) return;
      setSelectedId(id);
      setDraft({ ...toFocusUpdate(focus), x: String(x), y: String(y) });
      setIsDirty(true);
      return;
    }

    patch({ x: String(x), y: String(y) });
  }

  async function save() {
    if (!selectedFile || !draft || !selectedId) return;

    setIsSaving(true);
    try {
      const updated = await api.updateFocus(selectedFile.fullPath, selectedId, draft);
      // The id itself may have changed, so follow the draft rather than the old id.
      applyFile(updated, draft.id || selectedId);
      toast.success(`Saved ${draft.id || selectedId}`);
    } catch (error) {
      const message = describeError(error);
      setError(message);
      toast.error(message);
    } finally {
      setIsSaving(false);
    }
  }

  async function remove() {
    if (!selectedFile || !selectedId) return;
    if (!(await confirmDelete(`Delete focus "${selectedId}" from ${selectedFile.name}?`))) {
      return;
    }

    try {
      const updated = await api.deleteFocus(selectedFile.fullPath, selectedId);
      applyFile(updated, updated.focuses[0]?.id ?? null);
      toast.success(`Deleted ${selectedId}`);
    } catch (error) {
      const message = describeError(error);
      setError(message);
      toast.error(message);
    }
  }

  return (
    <div className="grid h-full grid-cols-[15rem_minmax(0,1fr)_20rem] gap-3 p-3">
      <div className="grid min-h-0 grid-rows-2 gap-3">
        <Panel>
          <PanelHeader title="Focus files" subtitle={`${files.length} in ${FOCUS_FOLDER}`} />
          <PanelBody>
            {files.length === 0 ? (
              <EmptyState
                title="No focus files"
                description={`This mod has no .txt files under ${FOCUS_FOLDER}.`}
              />
            ) : (
              <ul className="py-1">
                {files.map((file) => (
                  <li key={file.fullPath}>
                    <ListRow
                      active={selectedFile?.fullPath === file.fullPath}
                      onClick={() => void openFile(file)}
                      title={file.relativePath}
                    >
                      <FileText className="size-3.5 shrink-0 opacity-70" />
                      <span className="truncate">{file.name}</span>
                    </ListRow>
                  </li>
                ))}
              </ul>
            )}
          </PanelBody>
        </Panel>

        <Panel>
          <PanelHeader
            title="Focuses"
            subtitle={focusFile ? `${focuses.length} in this file` : "No file open"}
            actions={
              focusFile ? (
                <Button
                  variant="ghost"
                  size="icon-sm"
                  title="Add focus"
                  onClick={() => setIsAdding(true)}
                >
                  <Plus />
                </Button>
              ) : null
            }
          />
          <PanelBody>
            {focuses.length === 0 ? (
              <EmptyState title="Nothing to show" description="Open a focus file first." />
            ) : (
              <ul className="py-1">
                {focuses.map((focus) => (
                  <li key={`${focus.id}-${focus.line}`}>
                    <ListRow
                      active={focus.id === selectedId}
                      onClick={() => void selectFocus(focus.id)}
                      title={`line ${focus.line}`}
                    >
                      <span className="truncate font-mono text-xs">{focus.id || "(no id)"}</span>
                      {focus.shared ? (
                        <Badge tone="outline" className="ml-auto shrink-0">
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
          title={focusFile ? selectedFile?.name : "Focus tree"}
          subtitle={
            focusFile?.trees.length
              ? focusFile.trees.map((tree) => tree.id || "(unnamed tree)").join(", ")
              : "Open a file to see its tree"
          }
        />
        <PanelBody className="overflow-hidden p-0">
          {isLoading ? (
            <div className="flex h-full items-center justify-center gap-2 text-sm text-muted">
              <Loader2 className="size-4 animate-spin" />
              Reading focuses…
            </div>
          ) : focuses.length === 0 ? (
            <EmptyState
              icon={<FolderTree />}
              title="No focus tree loaded"
              description="Pick a file from common/national_focus to see its focuses laid out on the game grid."
            />
          ) : (
            <FocusCanvas
              focuses={focuses.map((focus) =>
                focus.id === selectedId && draft
                  ? { ...focus, ...draft }
                  : focus,
              )}
              selectedId={selectedId}
              onSelect={(id) => void selectFocus(id)}
              onMove={(id, x, y) => void moveFocus(id, x, y)}
            />
          )}
        </PanelBody>
      </Panel>

      <Panel>
        <PanelHeader
          title={selected ? "Focus properties" : "Nothing selected"}
          subtitle={selected ? `line ${selected.line}` : undefined}
          actions={
            selected ? (
              <>
                {isDirty ? <Badge tone="accent">Unsaved</Badge> : null}
                <Button variant="ghost" size="icon-sm" title="Delete focus" onClick={() => void remove()}>
                  <Trash2 />
                </Button>
                <Button
                  variant="primary"
                  size="sm"
                  onClick={() => void save()}
                  disabled={!isDirty || isSaving}
                >
                  {isSaving ? <Loader2 className="animate-spin" /> : <Save />}
                  Save
                </Button>
              </>
            ) : null
          }
        />
        <PanelBody className="p-3">
          {!draft ? (
            <EmptyState
              title="Select a focus"
              description="Pick one from the list or the tree to edit its fields."
            />
          ) : (
            <FocusForm
              draft={draft}
              focusIds={focuses.map((focus) => focus.id).filter(Boolean)}
              onChange={patch}
            />
          )}
        </PanelBody>
      </Panel>

      <AddFocusDialog
        open={isAdding}
        onOpenChange={setIsAdding}
        file={focusFile}
        onCreated={(updated, id) => applyFile(updated, id)}
        onError={(message) => {
          setError(message);
          toast.error(message);
        }}
      />
    </div>
  );
}

function FocusForm({
  draft,
  focusIds,
  onChange,
}: {
  draft: FocusUpdate;
  focusIds: string[];
  onChange: (changes: Partial<FocusUpdate>) => void;
}) {
  const listId = React.useId();

  return (
    <div className="flex flex-col gap-3">
      <datalist id={listId}>
        {focusIds.map((id) => (
          <option key={id} value={id} />
        ))}
      </datalist>

      <Field label="Id">
        <CodeInput value={draft.id} onChange={(event) => onChange({ id: event.target.value })} />
      </Field>

      <Field label="Icon" hint="Sprite name, e.g. GFX_goal_generic_army_doctrines">
        <CodeInput value={draft.icon} onChange={(event) => onChange({ icon: event.target.value })} />
      </Field>

      <div className="grid grid-cols-3 gap-2">
        <Field label="X">
          <CodeInput value={draft.x} onChange={(event) => onChange({ x: event.target.value })} />
        </Field>
        <Field label="Y">
          <CodeInput value={draft.y} onChange={(event) => onChange({ y: event.target.value })} />
        </Field>
        <Field label="Cost">
          <CodeInput value={draft.cost} onChange={(event) => onChange({ cost: event.target.value })} />
        </Field>
      </div>

      <Field label="Relative to" hint="Leave empty for absolute coordinates">
        <CodeInput
          list={listId}
          value={draft.relativePositionId}
          onChange={(event) => onChange({ relativePositionId: event.target.value })}
        />
      </Field>

      <div className="flex flex-col gap-1.5">
        <div className="flex items-center justify-between">
          <Label>Prerequisites</Label>
          <Button
            variant="ghost"
            size="icon-sm"
            title="Add prerequisite group"
            onClick={() => onChange({ prerequisites: [...draft.prerequisites, [""]] })}
          >
            <Plus />
          </Button>
        </div>
        {draft.prerequisites.length === 0 ? (
          <p className="text-xs text-muted">None. The focus is available from the start.</p>
        ) : (
          draft.prerequisites.map((group, index) => (
            <div key={index} className="flex items-center gap-1.5">
              <CodeInput
                list={listId}
                value={group.join(" ")}
                placeholder="focus_a focus_b"
                onChange={(event) => {
                  const next = [...draft.prerequisites];
                  next[index] = event.target.value.split(/\s+/).filter(Boolean);
                  onChange({ prerequisites: next });
                }}
              />
              <Button
                variant="ghost"
                size="icon-sm"
                title="Remove group"
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
        <p className="text-xs text-muted">
          Ids in one row satisfy each other; each row must be satisfied.
        </p>
      </div>

      <Field label="Mutually exclusive">
        <CodeInput
          list={listId}
          value={draft.mutuallyExclusive.join(" ")}
          placeholder="focus_c focus_d"
          onChange={(event) =>
            onChange({ mutuallyExclusive: event.target.value.split(/\s+/).filter(Boolean) })
          }
        />
      </Field>

      <BlockField
        label="Completion reward"
        value={draft.completionReward}
        onChange={(value) => onChange({ completionReward: value })}
        open
      />
      <BlockField
        label="Available"
        value={draft.available}
        onChange={(value) => onChange({ available: value })}
      />
      <BlockField
        label="Bypass"
        value={draft.bypass}
        onChange={(value) => onChange({ bypass: value })}
      />
      <BlockField
        label="Allow branch"
        value={draft.allowBranch}
        onChange={(value) => onChange({ allowBranch: value })}
      />
      <BlockField
        label="AI will do"
        value={draft.aiWillDo}
        onChange={(value) => onChange({ aiWillDo: value })}
      />
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
    <details open={open || value.trim().length > 0} className="group">
      <summary className="cursor-pointer list-none text-xs font-medium uppercase tracking-wide text-muted marker:content-none hover:text-foreground">
        {label}
        {value.trim() ? "" : " · empty"}
      </summary>
      <Textarea
        className="mt-1.5"
        rows={4}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder="add_political_power = 120"
      />
    </details>
  );
}

function AddFocusDialog({
  open,
  onOpenChange,
  file,
  onCreated,
  onError,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  file: FocusFile | null;
  onCreated: (updated: FocusFile, focusId: string) => void;
  onError: (message: string) => void;
}) {
  const [id, setId] = React.useState("");
  const [treeId, setTreeId] = React.useState("");
  const [isSaving, setIsSaving] = React.useState(false);

  React.useEffect(() => {
    if (open) {
      setId("");
      setTreeId(file?.trees[0]?.id ?? "");
    }
  }, [file, open]);

  async function create() {
    if (!file || !id.trim()) return;

    setIsSaving(true);
    try {
      const focus: FocusUpdate = {
        ...emptyFocusUpdate(),
        id: id.trim(),
        icon: "GFX_goal_unknown",
        x: "0",
        y: "0",
        cost: "10",
      };
      const updated = await api.addFocus(file.path, treeId, focus);
      onCreated(updated, focus.id);
      onOpenChange(false);
    } catch (error) {
      onError(describeError(error));
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        title="Add a focus"
        description="A stub focus is appended to the tree; fill in the rest on the right."
      >
        <DialogBody className="flex flex-col gap-3">
          <Field label="Focus id">
            <CodeInput
              value={id}
              autoFocus
              placeholder="my_country_rearmament"
              onChange={(event) => setId(event.target.value)}
            />
          </Field>
          {file && file.trees.length > 1 ? (
            <Field label="Focus tree">
              <CodeInput value={treeId} onChange={(event) => setTreeId(event.target.value)} />
            </Field>
          ) : null}
        </DialogBody>
        <DialogFooter>
          <DialogClose asChild>
            <Button>Cancel</Button>
          </DialogClose>
          <Button variant="primary" onClick={() => void create()} disabled={!id.trim() || isSaving}>
            {isSaving ? <Loader2 className="animate-spin" /> : <Plus />}
            Add focus
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
