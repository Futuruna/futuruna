// Exact, read-only primitives for the local Danish calculation-output viewers.
import { closeSync, constants, fstatSync, openSync, readFileSync } from 'node:fs';

const maxBytes = 16 * 1024 * 1024;
const number = new Intl.NumberFormat('da-DK', { maximumFractionDigits: 0 });
export function requireThat(condition, message) { if (!condition) throw new Error(message); }

// This output contract has only integer numbers. A small strict JSON reader
// preserves every i64 and rejects duplicate decoded keys (including escaped
// spellings), rather than losing evidence through JSON.parse's Number/last-key
// conversions. JSON.parse is used only for individual validated string tokens.
export function parseReportOutput(text) {
  requireThat(Buffer.byteLength(text, 'utf8') <= maxBytes, 'Resultatfilen overstiger 16 MiB.');
  let position = 0;
  const fail = () => { throw new Error(`Ugyldig eller ikke-understøttet resultat-JSON ved position ${position}.`); };
  const whitespace = () => { while (/[\x20\t\r\n]/.test(text[position] ?? '') && position < text.length) position++; };
  const string = () => {
    const token = /"(?:[^"\\\u0000-\u001f]|\\(?:["\\/bfnrt]|u[0-9a-fA-F]{4}))*"/y;
    token.lastIndex = position;
    const match = token.exec(text);
    if (!match) fail();
    position = token.lastIndex;
    return JSON.parse(match[0]);
  };
  const value = depth => {
    if (depth > 64) fail();
    whitespace();
    const first = text[position];
    if (first === '"') return string();
    if (first === '{' || first === '[') {
      const object = first === '{';
      const result = object ? Object.create(null) : [];
      const end = object ? '}' : ']';
      position++;
      whitespace();
      if (text[position] === end) { position++; return result; }
      while (true) {
        if (object) {
          const key = string();
          requireThat(!Object.hasOwn(result, key), `Gentaget JSON-feltnavn ved position ${position}.`);
          whitespace();
          if (text[position++] !== ':') fail();
          result[key] = value(depth + 1);
        } else result.push(value(depth + 1));
        whitespace();
        if (text[position] === end) { position++; return result; }
        if (text[position++] !== ',') fail();
        whitespace();
      }
    }
    for (const [literal, result] of [['true', true], ['false', false], ['null', null]]) {
      if (text.startsWith(literal, position)) { position += literal.length; return result; }
    }
    const token = /-?(?:0|[1-9][0-9]*)/y;
    token.lastIndex = position;
    const match = token.exec(text);
    if (!match) fail();
    position = token.lastIndex;
    requireThat(match[0].length <= 20, 'Et resultatbeløb ligger uden for i64.');
    const result = BigInt(match[0]);
    requireThat(result >= -(1n << 63n) && result < (1n << 63n), 'Et resultatbeløb ligger uden for i64.');
    return result;
  };
  const result = value(0);
  whitespace();
  if (position !== text.length) fail();
  return result;
}

export function record(value, fields, where) {
  requireThat(value !== null && typeof value === 'object' && !Array.isArray(value), `${where}: objekt forventet.`);
  const actual = Object.keys(value).sort();
  const expected = [...fields].sort();
  requireThat(actual.length === expected.length && actual.every((key, index) => key === expected[index]),
    `${where}: felterne svarer ikke til den understøttede resultatkontrakt.`);
}
export function textValue(value, where) { requireThat(typeof value === 'string' && value.trim().length > 0, `${where}: tekst mangler.`); }
export function list(value, where) { requireThat(Array.isArray(value), `${where}: liste forventet.`); }
export function integer(value, optional, where) {
  requireThat((optional && value === null) || (typeof value === 'bigint' && value >= -(1n << 63n) && value < (1n << 63n)),
    `${where}: eksakt heltal${optional ? ' eller null' : ''} forventet.`);
}
export function variant(value, choices, where) {
  record(value, ['$variant'], where);
  requireThat(typeof value.$variant === 'string' && Object.hasOwn(choices, value.$variant), `${where}: ukendt status.`);
  return value.$variant;
}
// Preserve text, but make control/bidi characters visible rather than allowing
// a case name or diagnostic to clear/rewrite terminal output or forge headings.
export function visible(text) {
  return text.replace(/[\u0000-\u001f\u007f-\u009f\u061c\u200e\u200f\u2028-\u202e\u2066-\u2069]/g,
    character => `\\u${character.codePointAt(0).toString(16).padStart(4, '0')}`);
}
export function amount(value, unit) {
  if (value === null) return 'ukendt';
  if (unit === 'DKK') return `${number.format(value)} DKK`;
  const magnitude = value < 0n ? -value : value;
  return `${value < 0n ? '-' : ''}${number.format(magnitude / 100n)},${String(magnitude % 100n).padStart(2, '0')} kr. (${number.format(value)} øre)`;
}
export function unit(value) { requireThat(value === 'DKK' || value === 'øre', 'Ukendt beløbsenhed; ingen beløb omregnet.'); }

// A case of `runa call`: its name, its result and, since Futuruna 0.4.1, the
// assumptions the result rests on (left out when there are none).
export function caseRecord(row) {
  record(row, Object.hasOwn(row ?? {}, 'assumptions') ? ['case_id', 'result', 'assumptions'] : ['case_id', 'result'], 'Sag');
  textValue(row.case_id, 'Sagsnavn');
}
// The assumptions a case's result rests on, each in full: a view that shows
// the result without them would claim more than the model does.
export function assumptionLines(row) {
  if (!Object.hasOwn(row, 'assumptions')) return ['Antagelser: modellen returnerede ingen for denne sag.'];
  list(row.assumptions, 'Antagelser');
  requireThat(row.assumptions.length > 0, 'Antagelser: en tom liste udelades i kontrakten.');
  const lines = ['Antagelser, som resultatet hviler på:'];
  for (const assumption of row.assumptions) {
    // `{section?, rule?, binding?, value}`: the law section the assumption
    // is written in and the rule it is written on, each left out when it
    // does not apply, but never both.
    const keys = ['section', 'rule', 'binding'].filter((key) => Object.hasOwn(assumption ?? {}, key));
    record(assumption, [...keys, 'value'], 'Antagelse');
    requireThat(keys.includes('section') || keys.includes('rule'), 'Antagelse: hverken afsnit eller regel er angivet.');
    if (keys.includes('section')) textValue(assumption.section, 'Antagelsens afsnit');
    if (keys.includes('rule')) textValue(assumption.rule, 'Antagelsens regel');
    if (keys.includes('binding')) textValue(assumption.binding, 'Antagelsens navn');
    const place = [
      ...(keys.includes('section') ? [`afsnit ${visible(assumption.section)}`] : []),
      ...(keys.includes('rule') ? [`regel ${visible(assumption.rule)}`] : []),
    ].join(', ');
    const name = keys.includes('binding') ? `, ${visible(assumption.binding)}` : '';
    lines.push(`  - ${place}${name}: ${visible(plain(assumption.value))}`);
  }
  return lines;
}
// A value from the strict reader as one line: text as written, integers
// exactly, records and lists with every member.
function plain(value) {
  if (typeof value === 'string') return value;
  if (typeof value === 'bigint' || typeof value === 'boolean' || value === null) return String(value);
  if (Array.isArray(value)) return `[${value.map(plain).join(', ')}]`;
  return `{${Object.entries(value).map(([key, member]) => `${key}: ${plain(member)}`).join(', ')}}`;
}

export function readSavedOutput(path) {
  let descriptor;
  try {
    descriptor = openSync(path, constants.O_RDONLY | (constants.O_NONBLOCK ?? 0));
    const stat = fstatSync(descriptor);
    requireThat(stat.isFile() && stat.size <= maxBytes, 'Vælg en almindelig JSON-resultatfil på højst 16 MiB.');
    return parseReportOutput(new TextDecoder('utf-8', { fatal: true }).decode(readFileSync(descriptor)));
  } finally { if (descriptor !== undefined) closeSync(descriptor); }
}
