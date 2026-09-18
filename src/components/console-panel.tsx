'use client';

import * as React from 'react';
import { ChevronDown, ChevronRight, Copy, Trash2, X } from 'lucide-react';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import { Badge, EmptyState } from '@/components/ui/panel';
import { useConsoleStore, visibleEntries } from '@/lib/console-store';
import type { ConsoleEntry, ConsoleLevel, ConsoleSource } from '@/lib/types';
import { cn } from '@/lib/utils';

const SOURCES: Array<{ value: ConsoleSource; label: string }> = [
  { value: 'app', label: 'App' },
  { value: 'files', label: 'Files' },
  { value: 'mcp', label: 'MCP' },
];

const LEVELS: Array<{ value: ConsoleLevel; label: string }> = [
  { value: 'info', label: 'Info' },
  { value: 'warn', label: 'Warnings' },
  { value: 'error', label: 'Errors' },
];

const LEVEL_DOT: Record<ConsoleLevel, string> = {
  info: 'bg-border-strong',
  warn: 'bg-warning',
  error: 'bg-danger',
};

/**
 * The activity log, docked under the workspace.
 *
 * Everything the backend did on the user's behalf lands here: their own
 * saves, cache rebuilds, and every tool call an MCP client made, with its
 * arguments and result a click away. It is the answer to "what did the
 * assistant just do to my mod?".
 */
export function ConsolePanel() {
  const state = useConsoleStore();
  const entries = React.useMemo(() => visibleEntries(state), [state]);
  const list = React.useRef<HTMLDivElement>(null);
  // Follows new entries only while the user is already at the bottom, so
  // reading an old one is not yanked away by the next tool call.
  const stick = React.useRef(true);

  React.useEffect(() => {
    const element = list.current;
    if (element && stick.current) element.scrollTop = element.scrollHeight;
  }, [entries.length]);

  function onScroll(event: React.UIEvent<HTMLDivElement>) {
    const element = event.currentTarget;
    stick.current = element.scrollHeight - element.scrollTop - element.clientHeight < 8;
  }

  return (
    <section aria-label='Console' className='flex h-60 shrink-0 flex-col border-t border-border bg-surface'>
      <div className='flex h-9 shrink-0 items-center gap-2 border-b border-border bg-surface-raised px-3 text-xs'>
        <span className='font-medium'>Console</span>
        <span className='text-muted'>
          {entries.length === state.entries.length
            ? `${state.entries.length} entries`
            : `${entries.length} of ${state.entries.length}`}
        </span>

        <div className='ml-3 flex items-center gap-1'>
          {SOURCES.map((source) => (
            <FilterChip
              key={source.value}
              label={source.label}
              on={state.sources[source.value]}
              onToggle={(on) => state.setSource(source.value, on)}
            />
          ))}
          <span className='mx-1 h-4 w-px bg-border' />
          {LEVELS.map((level) => (
            <FilterChip
              key={level.value}
              label={level.label}
              on={state.levels[level.value]}
              dot={LEVEL_DOT[level.value]}
              onToggle={(on) => state.setLevel(level.value, on)}
            />
          ))}
        </div>

        <div className='ml-auto flex items-center gap-1'>
          <Button variant='ghost' size='icon-sm' title='Clear the console' onClick={() => void state.clear()}>
            <Trash2 />
          </Button>
          <Button variant='ghost' size='icon-sm' title='Close the console' onClick={state.toggle}>
            <X />
          </Button>
        </div>
      </div>

      <div ref={list} onScroll={onScroll} className='min-h-0 flex-1 overflow-auto font-mono text-xs'>
        {entries.length === 0 ? (
          <EmptyState
            title='Nothing logged yet'
            description='Saves, cache rebuilds and MCP tool calls will appear here as they happen.'
          />
        ) : (
          <ul>
            {entries.map((entry) => (
              <ConsoleRow
                key={entry.id}
                entry={entry}
                expanded={state.expandedId === entry.id}
                onToggle={() => state.expand(state.expandedId === entry.id ? null : entry.id)}
              />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

function FilterChip({
  label,
  on,
  dot,
  onToggle,
}: {
  label: string;
  on: boolean;
  dot?: string;
  onToggle: (on: boolean) => void;
}) {
  return (
    <button
      type='button'
      aria-pressed={on}
      onClick={() => onToggle(!on)}
      className={cn(
        'flex items-center gap-1.5 rounded-full border px-2 py-0.5 text-[0.6875rem] transition-colors',
        on ? 'border-border-strong bg-surface text-foreground' : 'border-transparent text-muted hover:text-foreground',
      )}
    >
      {dot ? <span className={cn('size-1.5 rounded-full', dot, !on && 'opacity-40')} /> : null}
      {label}
    </button>
  );
}

function ConsoleRow({ entry, expanded, onToggle }: { entry: ConsoleEntry; expanded: boolean; onToggle: () => void }) {
  const hasDetail = Boolean(entry.detail);
  const Chevron = expanded ? ChevronDown : ChevronRight;
  const detail = React.useRef<HTMLPreElement>(null);

  // The detail of the last entry opens below the fold of a panel that was
  // scrolled to its bottom; bring it up rather than leave a chevron pointing
  // at nothing.
  React.useEffect(() => {
    if (expanded) detail.current?.scrollIntoView({ block: 'nearest' });
  }, [expanded]);

  async function copy() {
    const text = [formatTime(entry.atMs), entry.source, entry.level, entry.message, entry.detail ?? '']
      .filter(Boolean)
      .join('\n');
    try {
      await navigator.clipboard.writeText(text);
      toast.success('Copied');
    } catch {
      toast.error('The clipboard is not available');
    }
  }

  return (
    <li className='group border-b border-border/60'>
      <div
        role={hasDetail ? 'button' : undefined}
        tabIndex={hasDetail ? 0 : undefined}
        onClick={hasDetail ? onToggle : undefined}
        onKeyDown={(event) => {
          if (hasDetail && (event.key === 'Enter' || event.key === ' ')) {
            event.preventDefault();
            onToggle();
          }
        }}
        className={cn(
          'flex items-start gap-2 px-3 py-1 leading-5',
          hasDetail && 'cursor-pointer hover:bg-surface-raised',
          entry.level === 'error' && 'text-danger',
          entry.level === 'warn' && 'text-warning',
        )}
      >
        <span className='w-3 shrink-0 pt-1.5 text-muted'>{hasDetail ? <Chevron className='size-3' /> : null}</span>
        <span className='shrink-0 tabular-nums text-muted'>{formatTime(entry.atMs)}</span>
        <span className={cn('mt-2 size-1.5 shrink-0 rounded-full', LEVEL_DOT[entry.level])} />
        <Badge tone='outline' className='shrink-0 font-sans uppercase'>
          {entry.source}
        </Badge>
        <span className='min-w-0 flex-1 truncate' title={entry.message}>
          {entry.message}
        </span>
        <button
          type='button'
          title='Copy this entry'
          onClick={(event) => {
            event.stopPropagation();
            void copy();
          }}
          className='shrink-0 rounded p-0.5 text-muted opacity-0 transition-opacity group-hover:opacity-100 hover:text-foreground focus-visible:opacity-100'
        >
          <Copy className='size-3.5' />
        </button>
      </div>
      {expanded && entry.detail ? (
        <pre
          ref={detail}
          data-selectable
          className='max-h-48 overflow-auto whitespace-pre-wrap break-all border-t border-border/60 bg-surface-sunken px-9 py-2 text-[0.6875rem] leading-relaxed text-foreground'
        >
          {entry.detail}
        </pre>
      ) : null}
    </li>
  );
}

function formatTime(atMs: number): string {
  const date = new Date(atMs);
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}
