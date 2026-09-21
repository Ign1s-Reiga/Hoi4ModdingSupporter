'use client';

import * as React from 'react';
import { ImageOff } from 'lucide-react';

import type { TextMap } from '@/lib/use-localised-texts';
import type { EventUpdate, SpriteIcon } from '@/lib/types';
import { cn } from '@/lib/utils';

/**
 * The localisation key a title or description entry names: the entry itself
 * when it is a plain key, or the `text = key` inside a triggered block.
 */
export function textKeyOf(entry: string): string {
  const trimmed = entry.trim();
  if (!/[{=\s]/.test(trimmed)) return trimmed;
  return /\btext\s*=\s*"?([^\s"}]+)/.exec(trimmed)?.[1] ?? '';
}

/** Every key the preview of `event` will want resolved. */
export function previewKeys(event: EventUpdate): string[] {
  return [
    ...event.titles.map(textKeyOf),
    ...event.descs.map(textKeyOf),
    ...event.options.map((option) => option.name.trim()),
  ].filter(Boolean);
}

/**
 * The event window the game would draw, as near as a web page gets: the
 * picture, the title, the description and one button per option, with the
 * game's text codes rendered. A key no file defines is shown as itself,
 * marked, which is the point of previewing.
 */
export function EventPreview({
  event,
  texts,
  picture,
  className,
}: {
  event: EventUpdate;
  texts: TextMap;
  picture: SpriteIcon | undefined;
  className?: string;
}) {
  const news = event.kind === 'news_event';
  const title = event.titles[0] ?? '';
  const desc = event.descs[0] ?? '';

  return (
    <div
      className={cn(
        'mx-auto w-full overflow-hidden rounded-sm border border-[#8b7a4a] bg-[#1b1814] font-serif text-[#d8d2c2] shadow-[0_12px_40px_rgba(0,0,0,0.6)]',
        news ? 'max-w-[40rem]' : 'max-w-[27rem]',
        className,
      )}
    >
      <div className='border-b border-[#8b7a4a]/60 bg-[linear-gradient(180deg,#2b2519,#1b1814)] px-4 py-2.5 text-center'>
        <div className={cn('text-[#e5c15c]', news ? 'text-lg uppercase tracking-[0.12em]' : 'text-base tracking-wide')}>
          <GameText text={title} texts={texts} fallback='(no title)' />
        </div>
        {event.titles.length > 1 ? <Note>1 of {event.titles.length} titles, picked by trigger</Note> : null}
      </div>

      <div className='p-3'>
        <div
          className={cn(
            'flex items-center justify-center overflow-hidden border border-[#5c4f30] bg-[#0f0d0b]',
            news ? 'aspect-[16/7]' : 'aspect-[400/175]',
          )}
        >
          {picture?.url ? (
            // Data URL produced by the backend, so next/image cannot help here.
            // oxlint-disable-next-line next/no-img-element
            <img src={picture.url} alt='' className='size-full object-cover' />
          ) : (
            <div className='flex flex-col items-center gap-1 text-xs text-[#6f6552]'>
              <ImageOff className='size-5' />
              {event.picture.trim() ? `${event.picture.trim()} not found` : 'no picture'}
            </div>
          )}
        </div>

        <div className='mt-3 min-h-16 whitespace-pre-wrap px-1 text-[0.8125rem] leading-relaxed'>
          <GameText text={desc} texts={texts} fallback='(no description)' />
        </div>
        {event.descs.length > 1 ? (
          <Note className='px-1'>1 of {event.descs.length} descriptions, picked by trigger</Note>
        ) : null}

        <div className='mt-3 flex flex-col gap-1.5'>
          {event.options.length === 0 ? <Note>No option: the window could not be closed.</Note> : null}
          {event.options.map((option, index) => (
            <div
              key={index}
              className='rounded-sm border border-[#8b7a4a] bg-[linear-gradient(180deg,#332b1c,#221d14)] px-3 py-1.5 text-center text-[0.8125rem] text-[#e8dfc6] shadow-[inset_0_1px_0_rgba(255,255,255,0.06)]'
            >
              <GameText text={option.name} texts={texts} fallback='(unnamed option)' />
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function Note({ children, className }: { children: React.ReactNode; className?: string }) {
  return <p className={cn('mt-1 text-[0.6875rem] italic text-[#8a7f66]', className)}>{children}</p>;
}

/**
 * A localisation key rendered as the text behind it, or as the key itself
 * when no file defines it, or nothing yet while the lookup is out.
 */
function GameText({ text, texts, fallback }: { text: string; texts: TextMap; fallback: string }) {
  const key = textKeyOf(text);
  if (!key) return <span className='italic text-[#8a7f66]'>{fallback}</span>;

  const resolved = texts.get(key);
  if (resolved === undefined) return <span className='text-[#8a7f66]'>…</span>;
  if (resolved === null) {
    return (
      <span className='font-mono text-xs text-[#c9705a]' title='No localisation file of this language defines the key'>
        {key}
      </span>
    );
  }

  return <>{renderGameText(resolved)}</>;
}

/** The colours the game's `§` codes stand for, near enough. */
const COLOURS: Record<string, string> = {
  Y: '#e5c15c',
  H: '#f5d77a',
  R: '#d9534f',
  G: '#7bbf6a',
  B: '#6da2e0',
  C: '#6fd0d5',
  O: '#e9963e',
  W: '#ffffff',
  P: '#e08bd0',
  L: '#c9a26b',
  T: '#f0e6c8',
  M: '#b48ead',
};

/** The newer `#name … #!` formatting the game also accepts. */
const STYLES: Record<string, React.CSSProperties> = {
  bold: { fontWeight: 600 },
  italic: { fontStyle: 'italic' },
  underline: { textDecoration: 'underline' },
  red: { color: COLOURS.R },
  green: { color: COLOURS.G },
  blue: { color: COLOURS.B },
  yellow: { color: COLOURS.Y },
  gold: { color: COLOURS.H },
  grey: { color: '#9a9284' },
  white: { color: COLOURS.W },
  light_grey: { color: '#b5ad9d' },
  header: { fontWeight: 600, color: COLOURS.Y },
  title: { fontWeight: 600, color: COLOURS.Y },
};

/**
 * Turns the game's text into spans: `§Y…§!` and `#bold …#!` colour and
 * style runs, `\n` line breaks, and `[Root.GetName]`, `$VARIABLE$`, `£icon`
 * and `@TAG` placeholders the game would fill in at run time, shown as what
 * they are.
 */
function renderGameText(text: string): React.ReactNode[] {
  const out: React.ReactNode[] = [];
  const styles: React.CSSProperties[] = [];
  let buffer = '';
  let index = 0;
  let key = 0;

  const flush = () => {
    if (!buffer) return;
    const style = Object.assign({}, ...styles) as React.CSSProperties;
    out.push(
      <span key={key++} style={style}>
        {buffer}
      </span>,
    );
    buffer = '';
  };
  const placeholder = (label: string, hint: string) => {
    flush();
    out.push(
      <span key={key++} className='rounded-sm bg-white/10 px-1 font-mono text-[0.7em] text-[#b5ad9d]' title={hint}>
        {label}
      </span>,
    );
  };

  while (index < text.length) {
    const rest = text.slice(index);

    if (rest.startsWith('\\n')) {
      flush();
      out.push(<br key={key++} />);
      index += 2;
      continue;
    }
    if (rest.startsWith('§')) {
      const code = rest[1] ?? '';
      flush();
      if (code === '!') styles.pop();
      else styles.push(COLOURS[code] ? { color: COLOURS[code] } : {});
      index += 1 + code.length;
      continue;
    }
    const styled = /^#([a-z_]+)(?=\s)/.exec(rest);
    if (styled && STYLES[styled[1]]) {
      flush();
      styles.push(STYLES[styled[1]]);
      index += styled[0].length + 1;
      continue;
    }
    if (rest.startsWith('#!')) {
      flush();
      styles.pop();
      index += 2;
      continue;
    }
    const scripted = /^\[[^\]]*\]/.exec(rest);
    if (scripted) {
      placeholder(scripted[0], 'Scripted localisation the game fills in');
      index += scripted[0].length;
      continue;
    }
    const variable = /^\$[A-Za-z0-9_.|%]+\$/.exec(rest);
    if (variable) {
      placeholder(variable[0], 'A variable the game fills in');
      index += variable[0].length;
      continue;
    }
    const icon = /^[£@][A-Za-z0-9_]+/.exec(rest);
    if (icon) {
      placeholder(icon[0], icon[0].startsWith('£') ? 'An inline icon' : 'A country flag');
      index += icon[0].length;
      continue;
    }

    buffer += rest[0];
    index += 1;
  }
  flush();

  return out;
}
