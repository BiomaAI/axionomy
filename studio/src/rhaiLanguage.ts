import { StreamLanguage, StringStream } from "@codemirror/language";

// The stack allows interpolations to contain maps, comments and further templates.
type Context =
  | { kind: "interpolation" | "comment"; depth: number }
  | { kind: "string"; end: string; raw: boolean }
  | { kind: "template" };

const keywords = new Set("as break catch const continue do else export fn for if import in let loop private return switch throw try until while".split(" "));

function blockComment(stream: StringStream, state: Context[], comment: { depth: number }) {
  while (!stream.eol()) {
    if (stream.match("/*")) comment.depth++;
    else if (stream.match("*/")) {
      if (--comment.depth === 0) { state.pop(); break; }
    } else stream.next();
  }
  return "comment";
}

function quoted(stream: StringStream, state: Context[], quote: { end: string; raw: boolean }) {
  while (!stream.eol()) {
    if (stream.match(quote.end)) {
      if (!quote.raw && stream.match(quote.end)) continue;
      state.pop();
      return "string";
    }
    if (!quote.raw && stream.eat("\\")) {
      if (stream.eol()) return "string"; // A backslash continues a quoted string onto the next line.
    }
    stream.next();
  }
  if (quote.raw) return "string";
  state.pop();
  return "invalid";
}

function token(stream: StringStream, state: Context[]): string | null {
  const frame = state.at(-1);
  if (frame?.kind === "comment") return blockComment(stream, state, frame);
  if (frame?.kind === "string") return quoted(stream, state, frame);
  if (frame?.kind === "template") {
    if (stream.match("${")) {
      state.push({ kind: "interpolation", depth: 1 });
      return "string.special";
    }
    while (!stream.eol() && !stream.match("${", false)) {
      if (stream.match("\\${") || stream.match("``")) continue;
      if (stream.next() === "`") { state.pop(); break; }
    }
    return "string";
  }

  if (stream.eatSpace()) return null;
  if (stream.match("//")) { stream.skipToEnd(); return "comment"; }
  if (stream.match("/*")) {
    const comment: Context = { kind: "comment", depth: 1 };
    state.push(comment);
    return blockComment(stream, state, comment);
  }
  const raw = stream.match(/^(#+)"/);
  const delimiter = raw ? '"' + stream.current().slice(0, -1) : stream.eat(/["']/);
  if (delimiter) {
    const quote: Context = { kind: "string", end: delimiter, raw: Boolean(raw) };
    state.push(quote);
    return quoted(stream, state, quote);
  }
  if (stream.eat("`")) { state.push({ kind: "template" }); return "string"; }
  if (stream.match(/^#?\{/)) {
    if (frame?.kind === "interpolation") frame.depth++;
    return "brace";
  }
  if (stream.eat("}")) {
    if (frame?.kind === "interpolation" && --frame.depth === 0) { state.pop(); return "string.special"; }
    return "brace";
  }
  if (stream.match(/^(?:0x[\da-f_]+|0b[01_]+|0o[0-7_]+|\d[\d_]*(?:\.\d[\d_]*)?(?:e[+-]?\d[\d_]*)?)/i)) return "number";
  if (stream.match(/^[\p{ID_Start}_][\p{ID_Continue}]*/u)) {
    const word = stream.current();
    if (stream.match(/^\s*:(?!:)/, false)) return "propertyName";
    if (keywords.has(word)) return "keyword";
    if (word === "true" || word === "false") return "bool";
    return stream.match(/^\s*\(/, false) ? "variableName.function" : null;
  }
  if (stream.match(/^(?:\.\.[=]?|::|=>|(?:<<|>>|\*\*|&&|\|\||\?\?|[+*/%<>=!&|^~-])=?|\?)/)) return "operator";
  stream.next();
  return null;
}

export const rhaiLanguage = StreamLanguage.define<Context[]>({
  name: "rhai",
  startState: () => [],
  copyState: state => state.map(frame => ({ ...frame })),
  token,
});

export type RhaiToken = { text: string; kind: string | null; from: number; to: number };

/** Keep entire literals and comments together, including their original line endings. */
export function sourceTokens(source: string): RhaiToken[] | undefined {
  const state: Context[] = [];
  const result: RhaiToken[] = [];
  for (const line of source.matchAll(/[^\r\n]*(?:\r\n|\n|\r|$)/g)) {
    const stream = new StringStream(line[0].replace(/[\r\n]+$/, ""), 4, 4);
    while (!stream.eol()) {
      stream.start = stream.pos;
      const continuation = state.length > 0;
      const kind = token(stream, state);
      if (kind === "invalid") return undefined;
      const from = line.index + stream.start;
      const to = line.index + stream.pos;
      if (continuation) {
        result[result.length - 1].to = to;
      } else if (stream.current().trim()) {
        result.push({ from, to, text: stream.current(), kind });
      }
    }
  }
  if (state.length) return undefined;
  for (const item of result) item.text = source.slice(item.from, item.to);
  return result;
}
