"use client";

import * as React from "react";
import {
  defaultKeymap,
  history,
  historyKeymap,
  indentWithTab,
} from "@codemirror/commands";
import {
  bracketMatching,
  foldGutter,
  foldKeymap,
  HighlightStyle,
  indentUnit,
  StreamLanguage,
  syntaxHighlighting,
} from "@codemirror/language";
import { highlightSelectionMatches, search, searchKeymap } from "@codemirror/search";
import { EditorState, type Extension } from "@codemirror/state";
import {
  drawSelection,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import { tags } from "@lezer/highlight";

/**
 * A small stream parser for Clausewitz script. It is deliberately shallow —
 * enough to tell keys, values, numbers and comments apart, which is what makes
 * a focus or event file readable.
 */
const paradoxLanguage = StreamLanguage.define<{ afterOperator: boolean }>({
  name: "paradox",

  startState: () => ({ afterOperator: false }),

  token(stream, state) {
    if (stream.eatSpace()) return null;

    if (stream.peek() === "#") {
      stream.skipToEnd();
      return "comment";
    }

    if (stream.match(/^"(?:[^"\\]|\\.)*"?/)) {
      state.afterOperator = false;
      return "string";
    }

    if (stream.match(/^[{}]/)) {
      state.afterOperator = false;
      return "brace";
    }

    if (stream.match(/^(?:[<>!?]=|==|=|<|>)/)) {
      state.afterOperator = true;
      return "operator";
    }

    // Dates such as 1936.1.1 and plain numbers.
    if (stream.match(/^-?\d+(?:\.\d+)*/)) {
      state.afterOperator = false;
      return "number";
    }

    if (stream.match(/^(?:yes|no)\b/)) {
      state.afterOperator = false;
      return "bool";
    }

    if (stream.match(/^[^\s{}=<>!?#"]+/)) {
      const wasValue = state.afterOperator;
      state.afterOperator = false;
      return wasValue ? "value" : "key";
    }

    stream.next();
    return null;
  },

  tokenTable: {
    brace: tags.brace,
    bool: tags.bool,
    key: tags.propertyName,
    value: tags.variableName,
  },
});

const highlightStyle = HighlightStyle.define([
  { tag: tags.comment, color: "var(--muted)", fontStyle: "italic" },
  { tag: tags.propertyName, color: "var(--accent)" },
  { tag: tags.variableName, color: "var(--foreground)" },
  { tag: tags.string, color: "var(--success)" },
  { tag: tags.number, color: "var(--warning)" },
  { tag: tags.bool, color: "var(--warning)" },
  { tag: tags.operator, color: "var(--muted)" },
  { tag: tags.brace, color: "var(--border-strong)" },
]);

const editorTheme = EditorView.theme({
  "&": {
    height: "100%",
    fontSize: "13px",
    backgroundColor: "var(--surface-sunken)",
    color: "var(--foreground)",
  },
  ".cm-scroller": {
    fontFamily: "var(--font-mono)",
    lineHeight: "1.65",
  },
  ".cm-content": { caretColor: "var(--accent)" },
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--accent)" },
  "&.cm-focused": { outline: "none" },
  ".cm-gutters": {
    backgroundColor: "var(--surface-sunken)",
    color: "var(--border-strong)",
    border: "none",
  },
  ".cm-activeLine": { backgroundColor: "color-mix(in oklab, var(--accent) 7%, transparent)" },
  ".cm-activeLineGutter": {
    backgroundColor: "transparent",
    color: "var(--muted)",
  },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground, ::selection": {
    backgroundColor: "color-mix(in oklab, var(--accent) 25%, transparent)",
  },
  ".cm-searchMatch": {
    backgroundColor: "color-mix(in oklab, var(--accent) 25%, transparent)",
  },
  ".cm-panels": {
    backgroundColor: "var(--surface-raised)",
    color: "var(--foreground)",
    borderColor: "var(--border)",
  },
  ".cm-panel input, .cm-panel button": {
    backgroundColor: "var(--surface)",
    color: "var(--foreground)",
    border: "1px solid var(--border)",
    borderRadius: "4px",
  },
});

function extensions(readOnly: boolean): Extension[] {
  return [
    lineNumbers(),
    foldGutter(),
    history(),
    drawSelection(),
    bracketMatching(),
    highlightActiveLine(),
    highlightActiveLineGutter(),
    highlightSelectionMatches(),
    search({ top: true }),
    indentUnit.of("\t"),
    paradoxLanguage,
    syntaxHighlighting(highlightStyle),
    editorTheme,
    EditorView.lineWrapping,
    EditorState.readOnly.of(readOnly),
    keymap.of([
      ...defaultKeymap,
      ...historyKeymap,
      ...searchKeymap,
      ...foldKeymap,
      indentWithTab,
    ]),
  ];
}

export function ScriptEditor({
  value,
  onChange,
  onSave,
  readOnly = false,
  className,
}: {
  value: string;
  onChange?: (value: string) => void;
  onSave?: () => void;
  readOnly?: boolean;
  className?: string;
}) {
  const host = React.useRef<HTMLDivElement>(null);
  const view = React.useRef<EditorView | null>(null);

  // Kept in refs so the editor is never torn down just to see a new callback.
  const changeHandler = React.useRef(onChange);
  const saveHandler = React.useRef(onSave);

  React.useEffect(() => {
    changeHandler.current = onChange;
    saveHandler.current = onSave;
  });

  React.useEffect(() => {
    if (!host.current) return;

    const instance = new EditorView({
      parent: host.current,
      state: EditorState.create({
        doc: value,
        extensions: [
          ...extensions(readOnly),
          keymap.of([
            {
              key: "Mod-s",
              preventDefault: true,
              run: () => {
                saveHandler.current?.();
                return true;
              },
            },
          ]),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) {
              changeHandler.current?.(update.state.doc.toString());
            }
          }),
        ],
      }),
    });

    view.current = instance;
    return () => {
      instance.destroy();
      view.current = null;
    };
    // `value` is applied through the effect below; re-creating the editor on
    // every keystroke would lose the cursor.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [readOnly]);

  React.useEffect(() => {
    const instance = view.current;
    if (!instance) return;

    const current = instance.state.doc.toString();
    if (current === value) return;

    instance.dispatch({
      changes: { from: 0, to: current.length, insert: value },
    });
  }, [value]);

  return <div ref={host} className={className} data-selectable />;
}
