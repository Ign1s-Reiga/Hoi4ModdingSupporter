'use client';

import * as React from 'react';

import { api, describeError } from '@/lib/ipc';
import type { Encoding } from '@/lib/types';

/** What every file the backend hands back along with its text carries. */
export interface SourceFile {
  source: string;
  hasBom: boolean;
  encoding: Encoding;
}

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

export interface SourceDocument {
  /** The buffer: what the code view shows and every edit goes through. */
  source: string;
  setSource: (text: string) => void;
  /** Whether the buffer differs from what the disk last held. */
  isDirty: boolean;
  encoding: Encoding;
  hasBom: boolean;
  /** Why the buffer does not parse, or null while it does. */
  parseError: string | null;
  /**
   * Takes on a file as it was just read from disk: its text is what is
   * saved, and how it was written is how it will be written again. A file
   * handed back by a buffer operation goes through `show` alone, since its
   * text describes the encoding it was asked for, not what the disk holds.
   */
  adopt: (file: SourceFile) => void;
  /** Shows a file the backend handed back: the buffer follows its text. */
  show: (file: SourceFile) => void;
  /**
   * Writes `text` as the file, with the line endings it had. Returns the
   * encoding used, which differs from `encoding` when the text outgrew
   * Windows-1252 and the backend fell back to UTF-8.
   */
  write: (text: string) => Promise<Encoding>;
}

/**
 * A file's text as the document an editor works on, the way a Markdown
 * editor pairs source with preview.
 *
 * The page keeps the parsed view (a focus tree, a list of characters) and
 * hands back `onParsed`; the hook keeps the text, what the disk last held,
 * how the file was encoded, and re-parses the buffer once it sits still. A
 * text that does not parse leaves the last parsed view up with the error in
 * `parseError`, so a half-typed block does not blank the page.
 */
export function useSourceDocument<T extends SourceFile>(options: {
  /** Absolute path of the open file, undefined while nothing is open. */
  path: string | undefined;
  parse: (path: string, source: string, hasBom: boolean, encoding: Encoding) => Promise<T>;
  onParsed: (file: T) => void;
}): SourceDocument {
  const { path } = options;
  const [source, setSource] = React.useState('');
  // What the file on disk last held, so the two can be compared.
  const [savedSource, setSavedSource] = React.useState('');
  // The text the parsed view was read from, so an unchanged buffer is not
  // parsed again.
  const [parsedSource, setParsedSource] = React.useState('');
  const [encoding, setEncoding] = React.useState<Encoding>('utf8');
  const [hasBom, setHasBom] = React.useState(false);
  const [newline, setNewline] = React.useState<Newline>('\n');
  const [parseError, setParseError] = React.useState<string | null>(null);

  // What the parse callback needs at the moment it lands, not at the moment
  // the timer was set.
  const latest = React.useRef(options);
  React.useEffect(() => {
    latest.current = options;
  });

  React.useEffect(() => {
    if (!path || source === parsedSource) return;

    let cancelled = false;
    const timer = window.setTimeout(() => {
      void latest.current
        .parse(path, source, hasBom, encoding)
        .then((parsed) => {
          if (cancelled) return;
          setParsedSource(source);
          setParseError(null);
          latest.current.onParsed(parsed);
        })
        .catch((error: unknown) => {
          if (!cancelled) setParseError(describeError(error));
        });
    }, PARSE_DELAY_MS);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [encoding, hasBom, parsedSource, path, source]);

  const adopt = React.useCallback((file: SourceFile) => {
    setSavedSource(toBuffer(file.source));
    setEncoding(file.encoding);
    setHasBom(file.hasBom);
    setNewline(newlineOf(file.source));
  }, []);

  const show = React.useCallback((file: SourceFile) => {
    const text = toBuffer(file.source);
    setSource(text);
    setParsedSource(text);
    setParseError(null);
  }, []);

  const write = React.useCallback(
    async (text: string) => {
      if (!path) throw new Error('No file is open.');

      const used = await api.writeTextFile(path, text.replaceAll('\n', newline), encoding, hasBom);
      setSavedSource(text);
      if (used !== encoding) setEncoding(used);
      return used;
    },
    [encoding, hasBom, newline, path],
  );

  return {
    source,
    setSource,
    isDirty: source !== savedSource,
    encoding,
    hasBom,
    // A buffer put back to the text that last parsed is not parsed again,
    // so the error from the text in between has to clear on its own.
    parseError: source === parsedSource ? null : parseError,
    adopt,
    show,
    write,
  };
}
