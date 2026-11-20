import { sourceTokens, type RhaiToken } from "./rhaiLanguage";

const closeFor = new Map([["#{", "}"], ["{", "}"], ["[", "]"], ["(", ")"]]);
const closing = new Set(closeFor.values());
const binaryOperators = new Set([
  "=", "==", "!=", "=>", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>=", "**=",
  "&&", "||", "??", "*", "**", "/", "%", "<", ">", "<=", ">=", "<<", ">>", "&", "^",
]);
const continuesBlock = new Set(["else", "catch", "while", "until", "as", "in"]);
const indent = "    ";

/** Choose line breaks for complete groups before printing, so formatting twice gives the same layout. */
function expandedGroups(tokens: RhaiToken[], source: string) {
  const widths = [0];
  for (const { text } of tokens) widths.push(widths[widths.length - 1] + text.length + 1);
  const stack: number[] = [];
  const expanded = new Set<number>();
  for (const [i, { text, to }] of tokens.entries()) {
    if (closeFor.has(text)) stack.push(i);
    else if (closing.has(text)) {
      const start = stack.pop();
      if (start === undefined || closeFor.get(tokens[start].text) !== text) return;
      const from = tokens[start].from;
      if (i > start + 1 && (text === "}" || expanded.has(start) || widths[i + 1] - widths[start] > 88 || /[\n\r]/.test(source.slice(from, to)))) {
        expanded.add(start);
        expanded.add(i);
        const outer = stack.at(-1);
        if (outer !== undefined) expanded.add(outer);
      }
    }
  }
  return stack.length ? undefined : expanded;
}

/** Only whitespace between tokens is rewritten. An unfinished group stays editable as entered. */
export function formatRhai(source: string): string {
  const tokens = sourceTokens(source);
  if (!tokens?.length) return source;
  const expanded = expandedGroups(tokens, source);
  if (!expanded) return source;
  const newline = source.match(/\r\n|\n|\r/)?.[0] ?? "\n";
  const binary = tokens.map(({ text }, i) => {
    if (text !== "+" && text !== "-") return binaryOperators.has(text);
    // Leave a unary sign attached to its operand, including the signed integer minimum.
    const before = tokens[i - 1];
    return before && before.kind !== "keyword" && (closing.has(before.text) ||
      /^(?:[\p{ID_Start}_\d'"`]|#+")/u.test(before.text));
  });
  const groups: boolean[] = [];
  let depth = 0;
  let result = "";
  for (const [i, { text, from }] of tokens.entries()) {
    const previous = tokens[i - 1];
    if (closing.has(text) && groups.pop()) depth--;
    if (previous) {
      const gap = source.slice(previous.to, from);
      const oldLines = gap.match(/\r\n|\n|\r/g)?.length ?? 0;
      const inlineComment = !oldLines && (text.startsWith("//") || text.startsWith("/*"));
      const split = (closeFor.has(previous.text) && expanded.has(i - 1)) ||
        (closing.has(text) && expanded.has(i)) || previous.text === ";" ||
        (previous.text === "," && groups.at(-1)) ||
        (previous.text === "}" && /^[\p{ID_Start}_]/u.test(text) && !continuesBlock.has(text));
      if (oldLines || (split && !inlineComment)) {
        result += newline.repeat(oldLines > 1 ? 2 : 1) + indent.repeat(depth);
      } else if (inlineComment || previous.text === "," || previous.text === ":" ||
        binary[i] || binary[i - 1] ||
        (text === "{" && !closeFor.has(previous.text)) || (previous.text === "}" && continuesBlock.has(text))) {
        result += " ";
      } else {
        result += gap;
      }
    }
    // Rhai strips continuation indentation up to the opening quote's column.
    if ((text.startsWith('"') || text.startsWith("`")) && /\\\r?\n/.test(text)) {
      const originalColumn = columnAt(source, from);
      const newColumn = columnAt(result, result.length);
      if (newColumn > originalColumn) result += newline + " ".repeat(originalColumn);
      else result += " ".repeat(originalColumn - newColumn);
    }
    result += text;
    if (closeFor.has(text)) {
      groups.push(expanded.has(i));
      if (expanded.has(i)) depth++;
    }
  }
  return result + (/[\r\n]\s*$/.test(source) ? newline : "");
}

function columnAt(source: string, offset: number): number {
  const lineStart = Math.max(source.lastIndexOf("\n", offset - 1), source.lastIndexOf("\r", offset - 1)) + 1;
  return Array.from(source.slice(lineStart, offset)).length;
}

export function formatCompactRhai(source: string): string {
  return /[\r\n]/.test(source.trim()) ? source : formatRhai(source);
}
