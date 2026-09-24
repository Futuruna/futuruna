// Read-only presentation of saved conditional-model output. No tax evaluation,
// PDF import, network, source-fact inference, or modification of evidence.
import { closeSync, constants, fstatSync, openSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const maxBytes = 16 * 1024 * 1024;
const statuses = {
  BetingetAfstemt: 'Betinget afstemt — de viste kontroller stemmer; skatteforholdene er ikke godkendt.',
  Modstrid: 'Modstrid — mindst én kontrol eller nødvendig betingelse er ikke opfyldt.',
  Ufuldstændig: 'Ufuldstændig — oplysninger mangler; tilgængelige nødvendige beløb vises stadig.',
  UgyldigtRapportinput: 'Ugyldigt rapportinput — ingen afstemning udført.',
  IkkeUnderstøttetÅrEllerKommune: 'År eller kommune ikke understøttet — ingen afstemning udført.',
};
const checkStatuses = { Stemmer: 'Stemmer', Afviger: 'Afviger', IkkeOplyst: 'Ikke oplyst' };
const number = new Intl.NumberFormat('da-DK', { maximumFractionDigits: 0 });
function requireThat(condition, message) { if (!condition) throw new Error(message); }

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

function record(value, fields, where) {
  requireThat(value !== null && typeof value === 'object' && !Array.isArray(value), `${where}: objekt forventet.`);
  const actual = Object.keys(value).sort();
  const expected = [...fields].sort();
  requireThat(actual.length === expected.length && actual.every((key, index) => key === expected[index]),
    `${where}: felterne svarer ikke til den understøttede resultatkontrakt.`);
}
function textValue(value, where) { requireThat(typeof value === 'string' && value.trim().length > 0, `${where}: tekst mangler.`); }
function list(value, where) { requireThat(Array.isArray(value), `${where}: liste forventet.`); }
function integer(value, optional, where) {
  requireThat((optional && value === null) || (typeof value === 'bigint' && value >= -(1n << 63n) && value < (1n << 63n)),
    `${where}: eksakt heltal${optional ? ' eller null' : ''} forventet.`);
}
function variant(value, choices, where) {
  record(value, ['$variant'], where);
  requireThat(typeof value.$variant === 'string' && Object.hasOwn(choices, value.$variant), `${where}: ukendt status.`);
  return value.$variant;
}
// Preserve text, but make control/bidi characters visible rather than allowing
// a case name or diagnostic to clear/rewrite terminal output or forge headings.
function visible(text) {
  return text.replace(/[\u0000-\u001f\u007f-\u009f\u061c\u200e\u200f\u2028-\u202e\u2066-\u2069]/g,
    character => `\\u${character.codePointAt(0).toString(16).padStart(4, '0')}`);
}
function amount(value, unit) {
  if (value === null) return 'ukendt';
  if (unit === 'DKK') return `${number.format(value)} DKK`;
  const magnitude = value < 0n ? -value : value;
  return `${value < 0n ? '-' : ''}${number.format(magnitude / 100n)},${String(magnitude % 100n).padStart(2, '0')} kr. (${number.format(value)} øre)`;
}
function unit(value) { requireThat(value === 'DKK' || value === 'øre', 'Ukendt beløbsenhed; ingen beløb omregnet.'); }

export function renderReportOutput(envelope) {
  record(envelope, ['$futuruna', 'results', 'diagnostics'], 'Resultat');
  const metadata = envelope.$futuruna;
  record(metadata, ['schema', 'schema_hash', 'entry'], 'Kontrakt');
  requireThat(metadata.schema === 'futuruna.calculate.output.v1' && metadata.entry === 'afstem_årsopgørelse',
    'Brug JSON-output fra afstem_årsopgørelse, ikke input eller en anden beregning.');
  requireThat(typeof metadata.schema_hash === 'string' && /^[0-9a-f]{64}$/.test(metadata.schema_hash), 'Kontraktens fingeraftryk mangler.');
  list(envelope.results, 'Resultater');
  list(envelope.diagnostics, 'Diagnostik');
  requireThat(envelope.results.length + envelope.diagnostics.length > 0, 'Filen indeholder ingen sager eller diagnostik.');
  const ids = new Set();
  const overview = [];
  let needsAttention = envelope.diagnostics.length > 0;
  const lines = [
    'BETINGET RAPPORTAFSTEMNING — ikke en uafhængig skatteberegning eller skatterådgivning.',
    'Viser gemt modeloutput. Ingen genberegning, dokumentkontrol eller godkendelse af skatteforhold.',
    'Nødvendige beløb er betingelser, ikke dokumenterede fakta om ægtefællen. Ukendt er ikke nul.',
    `Kontrakt fra filen: ${metadata.schema_hash} (ikke kontrolleret mod den aktuelle model).`,
  ];
  for (const row of envelope.results) {
    record(row, ['case_id', 'result'], 'Sag');
    textValue(row.case_id, 'Sagsnavn');
    requireThat(!ids.has(row.case_id), 'Gentaget sagsnavn i resultater.');
    ids.add(row.case_id);
    const result = row.result;
    record(result, ['status', 'uafhængig_skatteberegning_udført', 'kontroller', 'nødvendige_forudsætninger', 'uafklaret'], 'Afstemning');
    const status = variant(result.status, statuses, 'Afstemning');
    overview.push(`  ${visible(row.case_id)}: ${statuses[status]}`);
    requireThat(result.uafhængig_skatteberegning_udført === false, 'Output må ikke påstå en uafhængig skatteberegning i denne visning.');
    list(result.kontroller, 'Kontroller');
    list(result.nødvendige_forudsætninger, 'Forudsætninger');
    list(result.uafklaret, 'Forbehold');
    requireThat(result.uafklaret.length > 0, 'Resultatets forbehold mangler.');
    if (status === 'BetingetAfstemt') requireThat(result.kontroller.length > 0, 'Betinget status uden kontroller afvises.');
    needsAttention ||= status !== 'BetingetAfstemt';
    lines.push('', `Sag: ${visible(row.case_id)}`, statuses[status], 'Kontroller (difference = oplyst − forventet):');
    if (result.kontroller.length === 0) lines.push('  Ingen kontroller returneret.');
    for (const check of result.kontroller) {
      record(check, ['navn', 'enhed', 'forventet', 'oplyst', 'difference', 'status'], 'Kontrol');
      textValue(check.navn, 'Kontrolnavn'); unit(check.enhed);
      for (const field of ['forventet', 'oplyst', 'difference']) integer(check[field], true, 'Kontrolbeløb');
      const state = variant(check.status, checkStatuses, 'Kontrol');
      const known = check.forventet !== null && check.oplyst !== null;
      requireThat(known
        ? check.difference === check.oplyst - check.forventet && state === (check.difference === 0n ? 'Stemmer' : 'Afviger')
        : check.difference === null && state === 'IkkeOplyst', 'Kontrolstatus og returnerede beløb er indbyrdes modstridende.');
      if (status === 'BetingetAfstemt') requireThat(state === 'Stemmer', 'Betinget status strider mod en kontrolstatus.');
      lines.push(`  ${visible(check.navn)}: ${checkStatuses[state]}`,
        `    Forventet: ${amount(check.forventet, check.enhed)}; oplyst: ${amount(check.oplyst, check.enhed)}; difference: ${amount(check.difference, check.enhed)}.`);
    }
    lines.push('Nødvendige betingelser — ikke bekræftede kildefakta:');
    if (result.nødvendige_forudsætninger.length === 0) lines.push('  Ingen nødvendige beløb returneret; det betyder ikke nul.');
    for (const condition of result.nødvendige_forudsætninger) {
      record(condition, ['navn', 'enhed', 'nødvendigt_beløb', 'mindst', 'højst', 'inden_for_kontrollerede_grænser', 'forklaring'], 'Betingelse');
      textValue(condition.navn, 'Betingelsesnavn'); textValue(condition.forklaring, 'Forklaring'); unit(condition.enhed);
      for (const field of ['nødvendigt_beløb', 'mindst']) integer(condition[field], false, 'Betingelsesbeløb');
      integer(condition.højst, true, 'Øvre grænse');
      requireThat(typeof condition.inden_for_kontrollerede_grænser === 'boolean', 'Betingelsens udfald mangler.');
      if (status === 'BetingetAfstemt') requireThat(condition.inden_for_kontrollerede_grænser, 'Betinget status strider mod en nødvendig betingelse.');
      const ceiling = condition.højst === null ? 'ukendt — ikke ubegrænset ret' : amount(condition.højst, condition.enhed);
      lines.push(`  ${visible(condition.navn)}: ${amount(condition.nødvendigt_beløb, condition.enhed)}`,
        `    Mindst: ${amount(condition.mindst, condition.enhed)}; højst: ${ceiling}. Inden for kontrollerede grænser: ${condition.inden_for_kontrollerede_grænser ? 'ja' : 'NEJ'}.`,
        `    ${visible(condition.forklaring)}`);
    }
    lines.push('Forbehold og uafklarede forhold:');
    for (const caveat of result.uafklaret) { textValue(caveat, 'Forbehold'); lines.push(`  - ${visible(caveat)}`); }
  }
  if (envelope.diagnostics.length > 0) lines.push('', 'SAGER UDEN BEREGNINGSRESULTAT:');
  for (const diagnostic of envelope.diagnostics) {
    record(diagnostic, ['case_id', 'path', 'message'], 'Diagnostik');
    for (const field of ['case_id', 'path', 'message']) textValue(diagnostic[field], 'Diagnostiktekst');
    requireThat(!ids.has(diagnostic.case_id), 'Samme sag har både resultat og diagnostik; visningen afvises.');
    lines.push(`  ${visible(diagnostic.case_id)} — ${visible(diagnostic.path)}: ${visible(diagnostic.message)}`);
  }
  const conclusion = needsAttention
    ? 'Kræver gennemgang: mindst én sag er ufuldstændig, modstridende, ugyldig eller uden resultat.'
    : 'Kun betinget afstemning. Resultaterne beviser ikke, at hele årsopgørelsen er korrekt.';
  lines.splice(4, 0, '', conclusion,
    `Overblik: ${envelope.results.length} sager med modeloutput; ${envelope.diagnostics.length} diagnostiklinjer.`, ...overview);
  lines.push('', conclusion);
  return { text: lines.join('\n') + '\n', exitCode: needsAttention ? 2 : 0 };
}

function main(args) {
  if (args.length === 1 && args[0] === '--help') {
    console.log('Brug: node examples/danish-income-tax/afstemning-resultat.mjs RESULTATER.json');
    console.log('Viser gemt afstem_årsopgørelse-output lokalt. Skriver ingen filer og sender ingen data.');
    console.log('Exit: 0 = kun betingede match; 2 = sager kræver gennemgang; 1 = fil/format kan ikke vises.');
    return 0;
  }
  requireThat(args.length === 1 && !args[0].startsWith('--'), 'Angiv én resultatfil, eller brug --help.');
  let descriptor;
  let content;
  try {
    descriptor = openSync(args[0], constants.O_RDONLY | (constants.O_NONBLOCK ?? 0));
    const stat = fstatSync(descriptor);
    requireThat(stat.isFile() && stat.size <= maxBytes, 'Vælg en almindelig JSON-resultatfil på højst 16 MiB.');
    content = new TextDecoder('utf-8', { fatal: true }).decode(readFileSync(descriptor));
  } finally { if (descriptor !== undefined) closeSync(descriptor); }
  const rendered = renderReportOutput(parseReportOutput(content));
  // Validate the entire document before printing any success-looking case.
  process.stdout.write(rendered.text);
  return rendered.exitCode;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { process.exitCode = main(process.argv.slice(2)); }
  catch (error) {
    // Do not echo raw JSON, personal values or filenames in parse/IO errors.
    const message = error.code ? `Filen kunne ikke læses (${error.code}).` : error.message;
    console.error(`Visning afvist: ${visible(message)} Ingen afstemning godkendt.`);
    process.exitCode = 1;
  }
}
