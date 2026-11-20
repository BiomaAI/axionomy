import { useId, useLayoutEffect, useMemo, useRef } from "react";
import { defaultKeymap, history, historyKeymap, isolateHistory } from "@codemirror/commands";
import { syntaxHighlighting } from "@codemirror/language";
import { Annotation, Compartment, EditorState, Transaction } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { classHighlighter } from "@lezer/highlight";
import { formatRhai } from "./formatRhai";
import { rhaiLanguage } from "./rhaiLanguage";

const externalValue = Annotation.define<boolean>();

type Props = {
  label: string;
  value: string;
  onChange: (source: string) => void;
  disabled?: boolean;
  compact?: boolean;
  formatOnOpen?: boolean;
};

export default function RhaiEditor({ label, value, onChange, disabled = false, compact, formatOnOpen }: Props) {
  const id = useId();
  const parent = useRef<HTMLDivElement>(null);
  const editor = useRef<EditorView | null>(null);
  const shown = useMemo(() => formatOnOpen ? formatRhai(value) : value, [formatOnOpen, value]);
  const latest = useRef({ shown, onChange });
  useLayoutEffect(() => { latest.current = { shown, onChange }; }, [shown, onChange]);

  const readOnly = useRef(new Compartment());
  useLayoutEffect(() => {
    const initial = latest.current.shown;
    const view = new EditorView({
      parent: parent.current!,
      doc: initial,
      extensions: [
        EditorState.lineSeparator.of(initial.match(/\r\n|\n|\r/)?.[0] ?? "\n"),
        rhaiLanguage, syntaxHighlighting(classHighlighter), history(), keymap.of([...defaultKeymap, ...historyKeymap]),
        EditorView.lineWrapping, readOnly.current.of([]),
        EditorView.contentAttributes.of({ id, "aria-labelledby": id + "-label", "aria-multiline": "true", spellcheck: "false" }),
        EditorView.updateListener.of(update => {
          if (update.docChanged && !update.transactions.some(tr => tr.annotation(externalValue))) {
            latest.current.onChange(update.state.sliceDoc());
          }
        }),
      ],
    });
    editor.current = view;
    return () => { view.destroy(); editor.current = null; };
  }, [id]);
  useLayoutEffect(() => {
    editor.current?.dispatch({ effects: readOnly.current.reconfigure([
      EditorState.readOnly.of(disabled), EditorView.editable.of(!disabled),
      EditorView.contentAttributes.of({ "aria-readonly": String(disabled) }),
    ]) });
  }, [disabled]);
  useLayoutEffect(() => {
    const view = editor.current;
    if (view && view.state.sliceDoc() !== shown) {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: shown },
        annotations: [externalValue.of(true), Transaction.addToHistory.of(false)] });
    }
  }, [shown]);

  function format() {
    const view = editor.current;
    if (!view || disabled) return;
    const source = view.state.sliceDoc();
    const formatted = formatRhai(source);
    if (formatted !== source) {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: formatted }, annotations: isolateHistory.of("full") });
    } else if (formatted !== value) {
      onChange(formatted);
    }
    view.focus();
  }
  return <div className={"rhai-field" + (compact ? " compact" : "")}>
    <div className="rhai-heading">
      <label id={id + "-label"} htmlFor={id} onClick={() => editor.current?.focus()}>{label}</label>
      <button type="button" disabled={disabled || !value.trim()} onClick={format}>Format</button>
    </div>
    <div ref={parent} className="rhai-editor" />
  </div>;
}
