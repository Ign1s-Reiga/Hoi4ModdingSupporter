'use client';

import * as React from 'react';
import { Code2, Columns2, TriangleAlert } from 'lucide-react';

import { ScriptEditor, type Reveal } from '@/components/script-editor';
import { cn } from '@/lib/utils';

/** How an editor pairs its own view with the file's text. */
export type View = 'visual' | 'split' | 'code';

const VIEW_VALUES: readonly View[] = ['visual', 'split', 'code'];

type Icon = React.ComponentType<{ className?: string }>;

/** The chosen view, remembered per machine under `key`. */
export function useStoredView(key: string): [View, (next: View) => void] {
  const [view, setView] = React.useState<View>(() => storedView(key));

  const choose = React.useCallback(
    (next: View) => {
      setView(next);
      try {
        window.localStorage.setItem(key, next);
      } catch {
        // A private window forgets; the choice still holds for this session.
      }
    },
    [key],
  );

  return [view, choose];
}

function storedView(key: string): View {
  try {
    const stored = window.localStorage.getItem(key);
    return VIEW_VALUES.some((view) => view === stored) ? (stored as View) : 'visual';
  } catch {
    return 'visual';
  }
}

/** The three-way switch between an editor's own view, both, and the text. */
export function ViewToggle({
  value,
  onChange,
  visual,
}: {
  value: View;
  onChange: (view: View) => void;
  /** What the editor's own view is called: a tree, a form. */
  visual: { label: string; icon: Icon };
}) {
  const options: Array<{ value: View; label: string; icon: Icon }> = [
    { value: 'visual', label: visual.label, icon: visual.icon },
    { value: 'split', label: 'Split', icon: Columns2 },
    { value: 'code', label: 'Code', icon: Code2 },
  ];

  return (
    <div role='radiogroup' aria-label='Layout' className='flex rounded-md border border-border p-0.5'>
      {options.map((option) => {
        const OptionIcon = option.icon;
        const active = option.value === value;
        return (
          <button
            key={option.value}
            type='button'
            role='radio'
            aria-checked={active}
            title={option.label}
            onClick={() => onChange(option.value)}
            className={cn(
              'flex size-7 items-center justify-center rounded transition-colors',
              active ? 'bg-surface-sunken text-foreground' : 'text-muted hover:text-foreground',
            )}
          >
            <OptionIcon className='size-4' />
          </button>
        );
      })}
    </div>
  );
}

/**
 * The code half of a split editor: the script editor with the parse error,
 * if any, in a banner above it.
 */
export function SourcePane({
  source,
  parseError,
  reveal,
  onChange,
  onSave,
  className,
}: {
  source: string;
  parseError: string | null;
  reveal: Reveal | undefined;
  onChange: (text: string) => void;
  onSave: () => void;
  className?: string;
}) {
  return (
    <div className={cn('flex min-h-0 flex-col', className)}>
      {parseError ? (
        <div className='flex shrink-0 items-start gap-2 border-b border-border bg-danger-soft px-3 py-1.5 text-xs text-danger'>
          <TriangleAlert className='mt-0.5 size-3.5 shrink-0' />
          <span className='min-w-0 break-words'>{parseError}</span>
        </div>
      ) : null}
      <ScriptEditor
        value={source}
        onChange={onChange}
        onSave={onSave}
        reveal={reveal}
        className='min-h-0 flex-1 overflow-hidden'
      />
    </div>
  );
}
