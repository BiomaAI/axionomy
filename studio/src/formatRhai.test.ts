import { describe, expect, it } from "vitest";
import { formatRhai } from "./formatRhai";

describe("Rhai source layout", () => {
  it("opens compact maps, nested blocks and statements on indented lines", () => {
    const result = formatRhai('let m=#{name:"maze",version:#{major:1,minor:2}};if true{return m;}\n');
    expect(result).toBe('let m = #{\n    name: "maze",\n    version: #{\n        major: 1,\n        minor: 2\n    }\n};\nif true {\n    return m;\n}\n');
    expect(formatRhai(result)).toBe(result);
  });

  it("spaces arithmetic and keeps unary signs and range operators intact", () => {
    expect(formatRhai('fn add(x){return -9223372036854775808+x*2;}add(-1);0..=2'))
      .toBe('fn add(x) {\n    return -9223372036854775808 + x * 2;\n}\nadd(-1);\n0..=2');
  });

  it("treats nested comments, raw strings and template interpolations as source text", () => {
    const raw = '##"a " quote, #{; } and // hi"##';
    const template = '`path/${#{cell: `cell/${x}`}.cell}; // literal`';
    const comment = '/* ; { outer\n  /* nested */ end */';
    const result = formatRhai('let x=' + comment + raw + ';let y=' + template + '; // keep\n[x,y]');
    for (const literal of [raw, template, comment, '// keep\n']) expect(result).toContain(literal);
    expect(result).toContain('\nlet y = ');
    expect(formatRhai(result)).toBe(result);
  });

  it("keeps the opening column of a continued string and the file's line endings", () => {
    const source = 'let x=#{a:"first\\\r\n            second"};x.a';
    const formatted = formatRhai(source);
    expect(formatted.split('\r\n').find(l => l.includes('"first'))?.indexOf('"')).toBe(source.indexOf('"'));
    expect(formatted).toContain('"first\\\r\n            second"');
    expect(formatted.replaceAll('\r\n', '')).not.toContain('\n');
    expect(formatRhai(formatted)).toBe(formatted);
  });

  it("leaves an unfinished edit intact", () => {
    for (const source of ['let x=#{a:1', 'fn f(]', '`value/${', 'let x="unfinished', '/* open']) {
      expect(formatRhai(source)).toBe(source);
    }
  });
});
