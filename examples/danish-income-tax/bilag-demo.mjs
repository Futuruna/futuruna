// A fixed, fictional source-to-input rehearsal, NOT a document importer.
// Source classification is authored explicitly below; only Futuruna computes tax.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderPersonskatOutput } from './personskat-resultat.mjs';
import { readSavedOutput } from './resultat-visning.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const v = ($variant, fields = {}) => ({ $variant, ...fields });
export const sources = JSON.parse(readFileSync(new URL('./bilag-demo-kilder.json', import.meta.url), 'utf8'));

function source(ref, documents) {
  const [id, line] = ref.split(':');
  const found = documents.dokumenter[id]?.linjer[line];
  assert.ok(found, `Ukendt fiktiv kildelinje: ${ref}`);
  return found;
}

// Pure construction is also tested without executing any tax rules.
export function buildFictionalCases(template, documents = sources) {
  assert.equal(documents.fiktivt, true, 'Dette eksempel modtager ikke personlige data.');
  const envelope = structuredClone(template);
  const value = (ref) => source(ref, documents).værdi;
  const kroner = (ref) => {
    const row = source(ref, documents);
    assert.equal(row.enhed, 'kr./år', ref);
    assert.ok(Number.isSafeInteger(row.værdi) && row.værdi >= 0, `${ref}: hele, ikke-negative kroner`);
    return row.værdi;
  };
  const confirmed = (ref) => assert.equal(value(ref), true, `${ref}: fiktiv bekræftelse mangler`);
  for (const ref of ['F:4', 'F:5', 'F:6', 'L:2', 'P:2', 'B:4', 'EP:4']) confirmed(ref);
  assert.equal(value('F:1'), 2025);
  assert.equal(value('F:3'), 101);
  const year = value('F:1');
  const ledger = [];
  function caseFromTemplate(case_id) {
    const input = structuredClone(template.cases[0].input);
    const mappings = [];
    function put(path, data, refs, note) {
      refs.forEach(ref => source(ref, documents));
      const parts = path.split('.'); const name = parts.pop();
      const parent = parts.reduce((record, key) => record[key], input);
      assert.ok(parent && Object.hasOwn(parent, name), `Skabelonen har ændret sig: ${path}`);
      parent[name] = structuredClone(data);
      mappings.push({ path, value: structuredClone(data), sources: refs, explanation: note });
    }
    put('lønmodtager.skatteår', year, ['F:1'], 'Indkomstår, ikke dokumentets udskriftsår.');
    put('lønmodtager.kommune', v('København'), ['F:3'], 'Afklaret skattekommune, ikke gættet nuværende bopæl.');
    put('lønmodtager.kirkeskat', v('IngenKirkeskatHeleÅret'), ['F:3'], 'Udtrykkeligt afklaret fravær af kirkeskat hele indkomståret.');
    put('lønmodtager.pension.fødselsdato', value('F:2'), ['F:2'], 'Personens dato, også relevant uden pension.');
    put('lønmodtager.pension.udbetalingsoplysninger', {
      for_året_komplette: true, for_foregående_år_komplette: true,
    }, ['F:6'], 'Tom historik er her udtrykkeligt bekræftet for begge år.');
    put('lønmodtager.ligningsfradrag.enlig_forsørger', v('IntetEkstraBørnetilskud'), ['F:4'], 'Ikke udledt af civilstand.');
    put('lønmodtager.ligningsfradrag.boligjob', v('IngenBoligjobudgifter'), ['F:4'], 'Bekræftet ingen relevante udgifter.');
    put('lønmodtager.ligningsfradrag.arbejdsfradrag_udland', v('IngenUdlandsudelukkelseIFællesForhold', {
      dbo_hjemmehørende_udland_i_nogen_periode: false,
      noget_arbejde_udført_udland: null, nogen_udenlandsk_arbejdsgiver: null,
      kildereference: 'F:5 — fiktivt afklaret DBO-hjemsted',
    }), ['F:5'], 'Dokumenteret nej til DBO-udlandsbetingelsen; de to øvrige tests behøves ikke.');
    assert.deepEqual(input.ægtefælle, v('UdenÆgtefælle'));
    ledger.push({ case_id, baseline_confirmation: 'F:4', mappings, excluded_sources: [] });
    return { input, put, record: ledger.at(-1) };
  }
  function payment(id, amount, am, employer) {
    return {
      identifikation: id, ordning: v('Pbl18Rateopsparing'),
      indbetalingskilde: v(employer ? 'Pbl18Arbejdsgiverindbetaling' : 'Pbl18EgenIndbetaling'),
      fradragsretshaver: v('Pbl18OrdningensEjer'),
      betaling: {
        beløb_kroner: amount, arbejdsmarkedsbidrag_kroner: am,
        forfaldsår: year, betalingsår: year, betalt_senest_bankjusteret_1_april_efter_forfald: true,
        hidrører_fra_par22e_tilbagebetaling: false, par15a_fradragsplacering: v('Pbl18IkkePar15APlacering'),
      },
      fordelingsforløb: v('Pbl18IngenTiårsfordeling'),
      indeksvalg: { fradragsvalgte_kontraktbidrag_kroner: [] },
      indeksordningsgrundlag: v('Pbl18IkkeIndeksordning'),
      forfaldne_ikke_tidligere_fratrukket_kroner: 0,
      særligt_ordningsgrundlag: v('Pbl18IntetSærligtOrdningsgrundlag'),
      begrænsninger: {
        pbl54_personkreds_opfyldt: true, afgiftspligt_for_hele_ordningen_indtrådt: false,
        udenlandsk_overførsel_med_tidligere_fradrag_uden_skatte_eller_afgiftskonsekvens: false,
      },
    };
  }
  const cases = [];
  for (const case_id of ['privat-rate', 'egne-renter', 'arbejdsgiver-atp', 'atp-uoplyst']) {
    const { input, put, record } = caseFromTemplate(case_id);
    if (case_id === 'privat-rate' || case_id === 'egne-renter') {
      put('lønmodtager.bruttoløn_kroner', kroner('L:1'), ['L:1', 'L:2'], 'Før AM og skat; privat pension trækkes ikke fra dette input.');
      put('lønmodtager.pension.atp', v('IngenAtpIndbetalinger'), ['L:2'], 'Fiktivt bekræftet fravær, ikke et standardvalg.');
      if (case_id === 'privat-rate') {
        put('lønmodtager.pension.pbl18_indbetalinger', [payment('P:1', kroner('P:1'), 0, false)],
          ['P:1', 'P:2', 'F:1'], 'Privat betaling én gang; modellen beregner fradragene.');
      } else {
        const share = value('A:1');
        assert.equal(source('A:1', documents).enhed, '%');
        assert.ok(Number.isSafeInteger(share) && share >= 0 && share <= 100, 'A:1: andel skal være afklaret');
        const numerator = kroner('B:1') * share;
        assert.ok(Number.isSafeInteger(numerator) && numerator % 100 === 0, 'Andelen kan ikke repræsenteres i hele kroner.');
        const ownInterest = numerator / 100;
        assert.equal(ownInterest, kroner('R:1'), 'Kilderne stemmer ikke; ret ikke fakta for at få et match.');
        put('kapitalindkomst.renter.renteudgifter_kroner', ownInterest, ['B:1', 'B:4', 'A:1'],
          'Lånets renter × dokumenteret andel; ikke nettobeløb eller hele lånets ydelse.');
        put('kapitalindkomst.renter.renteindtægter_kroner', kroner('B:3'), ['B:3'], 'Indtægt separat; ingen dobbelt modregning.');
        record.excluded_sources.push(
          { source: 'B:2', reason: 'Afdrag er ikke renter.' },
          { source: 'R:1', reason: 'Samme allerede medtagne udgift, kun krydskontrol.' },
        );
      }
    } else {
      assert.equal(kroner('E:1') - kroner('E:2') - kroner('E:3'), kroner('E:4'), 'Lønoplysningerne er modstridende.');
      assert.equal(kroner('E:2') + kroner('E:5'), kroner('EP:1'), 'Pensionens opdeling er modstridende.');
      assert.equal(kroner('EP:1') - kroner('EP:2'), kroner('EP:3'), 'Pension før/efter AM er modstridende.');
      put('lønmodtager.bruttoløn_kroner', kroner('E:4'), ['E:1', 'E:2', 'E:3', 'E:4'],
        'E:4 er allerede efter eget pensionsbidrag og ATP. Træk ikke disse fra igen.');
      put('lønmodtager.pension.pbl18_indbetalinger', [payment('EP:1', kroner('EP:1'), kroner('EP:2'), true)],
        ['E:2', 'E:5', 'EP:1', 'EP:2', 'EP:3', 'EP:4', 'F:1'],
        'Hele arbejdsgiverordningen én gang, inklusive medarbejderens andel. Ikke også privat pension eller arbejdsgiverydelse.');
      if (case_id === 'atp-uoplyst') {
        assert.equal(value('U:1'), null);
        put('lønmodtager.pension.atp', v('AtpUoplyst'), ['U:1', 'E:3'],
          'Eget løntræk oplyser ikke hele ATP-grundlaget. Ingen opskalering eller gættet netto.');
        record.excluded_sources.push({ source: 'T', reason: 'Ikke til rådighed i denne selvstændige variant.' });
      } else {
        confirmed('T:3');
        put('lønmodtager.pension.atp', v('OplysteAtpIndbetalinger', {
          oplysninger_for_året_komplette: true,
          poster: [{ identifikation: 'T:1', indkomstår: year, kildereference: 'T:1–3 — fiktiv ATP-oversigt',
            grundlag: v('AtpArbejdsgiverPar19Stk1'),
            indberettet_før_am_kroner: kroner('T:1'), indberettet_efter_am_kroner: kroner('T:2') }],
        }), ['T:1', 'T:2', 'T:3', 'F:1'], 'Dokumenteret samlet bidrag og netto; ikke beregnet fra E:3.');
      }
    }
    cases.push({ case_id, input });
  }
  assert.equal(value('U:2'), null);
  const held = [{
    case_id: 'renteandel-uoplyst', path: 'kapitalindkomst.renter.renteudgifter_kroner',
    sources: ['B:1', 'B:4', 'U:2'], unavailable_documents: ['A', 'R'],
    status: 'IkkeIndsendt',
    question: 'Hvad er personens dokumenterede fradragsberettigede andel af lånets renter?',
    explanation: 'Beløbsfeltet kan ikke repræsentere ukendt. Interviewet stopper denne sag før call; modellen har ikke afvist et indsendt beløb. Nul og 50 % er ikke tilladte gæt.',
  }];
  envelope.cases = cases;
  return { envelope, ledger, held };
}

// These expected totals come from recorded independent 2025 observations, not
// from the source documents or from this script's construction of the input.
export function verifyResults(output) {
  assert.deepEqual(output.diagnostics, []);
  assert.deepEqual(output.results.map(row => row.case_id), ['privat-rate', 'egne-renter', 'arbejdsgiver-atp', 'atp-uoplyst']);
  for (const { case_id, result: r } of output.results) {
    const invalid = case_id === 'atp-uoplyst'; const a = r.vurdering;
    assert.equal(a.status.$variant, invalid ? 'UgyldigtBeregningsgrundlag' : 'BeregnetMedForbehold', case_id);
    assert.equal(a.alle_kontroller_gyldige, !invalid, case_id);
    assert.equal(a.samlet_modeldækning_bekræftet, false);
    assert.ok(a.forbehold.length > 0);
    if (invalid) {
      assert.equal(a.slutskat_til_sammenligning_øre, null);
      assert.ok(a.fejl.some(f => f.sti === 'lønmodtager.pension.atp'));
      continue;
    }
    assert.deepEqual(a.fejl, []);
    assert.equal(r.skat.bruttoløn_kroner, 600000);
    assert.equal(r.skat.arbejdsmarkedsbidrag_kroner, 48000);
    if (case_id === 'privat-rate') {
      assert.equal(a.slutskat_til_sammenligning_øre, 19661194, 'skatdk-pensionsfradrag-ekstern.md, 2025/1990/40001');
      assert.equal(r.skat.personlig_indkomst_efter_am_kroner, 511999);
      assert.equal(r.skat.ekstra_pensionsfradrag_kroner, 4801);
    } else if (case_id === 'egne-renter') {
      assert.equal(a.slutskat_til_sammenligning_øre, 19619430, 'skatdk-rentefradrag-ekstern.md, 600000/51001/1000');
      assert.equal(r.skat.nettokapitalindkomst_kroner, -50001);
    } else {
      // Source-model checks only. The literal 2025 employer-rate form inputs
      // disagree; see skatdk-arbejdsgiverpension-ekstern.md. Do not fit inputs.
      assert.ok(Number.isSafeInteger(a.slutskat_til_sammenligning_øre));
      assert.equal(r.skat.personlig_indkomst_efter_am_kroner, 552000);
      assert.equal(r.arbejdsfradrag_udland.grundlag.grundlag_før_udlandsafgrænsning_kroner, 653000);
      assert.equal(r.pension.arbejdsgiver_rate_resultat.indbetaling_før_am_kroner, 50000);
      assert.equal(r.pension.arbejdsgiver_rate_resultat.indeholdt_am_kroner, 4000);
      assert.equal(r.pension.arbejdsgiver_rate_resultat.bortseelsesberettiget_efter_am_kroner, 46000);
      assert.equal(r.pension.pbl18_årsresultat.rate_og_ophørende_fradrag_kroner, 0);
      assert.equal(r.pension.atp_resultat.arbejdsfradragsgrundlag_kroner, 3000);
      assert.equal(r.pension.atp_resultat.ekstra_pensionsfradragsgrundlag_kroner, 2760);
      assert.equal(r.skat.ekstra_pensionsfradrag_kroner, 5852);
    }
  }
}

function saveEvidence(directory, name, value) {
  const path = join(directory, name);
  writeFileSync(path, typeof value === 'string' ? value : JSON.stringify(value, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
  return path;
}

// Separate display from computation so a display repair can reuse saved
// fictional output without spending another canonical calculation run.
export function finishFictionalDemo(evidence, held) {
  const resultPath = join(evidence, 'results.json');
  const output = JSON.parse(readFileSync(resultPath, 'utf8'), (_key, value) => {
    // This demonstration checks small fixed fictional amounts. Reject lossy
    // Number conversion; the separate viewer below uses exact BigInt values.
    if (typeof value === 'number') assert.ok(Number.isSafeInteger(value), 'Eksemplets resultat kræver eksakte sikre heltal.');
    return value;
  });
  verifyResults(output);
  // The viewer uses BigInt throughout, including for values beyond JS Number.
  // Read the original bytes through its strict reader, never a JSON roundtrip.
  const rendered = renderPersonskatOutput(readSavedOutput(resultPath));
  assert.equal(rendered.exitCode, 2, 'ATP-ukendt skal kræve opmærksomhed i visningen.');
  saveEvidence(evidence, 'resultat.txt', rendered.text);
  const rows = output.results.map(({ case_id, result }) => ({
    case_id, status: result.vurdering.status.$variant,
    slutskat_til_sammenligning_øre: result.vurdering.slutskat_til_sammenligning_øre,
    evidence: case_id === 'privat-rate' || case_id === 'egne-renter'
      ? 'Match med tidligere registreret offentlig 2025-beregner; ikke en ny ekstern observation.'
      : case_id === 'arbejdsgiver-atp'
        ? 'Uafklaret afvigelse fra den offentlige 2025-formular for arbejdsgiverrate; se skatdk-arbejdsgiverpension-ekstern.md.'
        : 'Modelregression af ukendte fakta; ikke ekstern konformitet.',
  }));
  saveEvidence(evidence, 'summary.json', { fictional: true, cases: rows, held_before_call: held });
  console.log('Fiktiv bilagsgennemgang, 2025. Beregnet med forbehold — ikke en godkendt årsopgørelse.');
  const money = new Intl.NumberFormat('da-DK', { style: 'currency', currency: 'DKK' });
  for (const row of rows) console.log(`${row.case_id}: ${row.slutskat_til_sammenligning_øre === null
    ? 'Intet sammenligningsbeløb: ATP-oplysninger mangler.'
    : `${money.format(row.slutskat_til_sammenligning_øre / 100)} modelleret skat.`}`);
  console.log('renteandel-uoplyst: Ikke indsendt; egen andel skal afklares.');
  console.log('arbejdsgiver-atp: Uafklaret ekstern afvigelse; ret ikke kildefakta for at få et match. Se skatdk-arbejdsgiverpension-ekstern.md.');
  console.log(`Fire beregnede sager og én tilbageholdt sag kontrolleret. Fuld visning med alle forbehold: ${join(evidence, 'resultat.txt')}`);
  return output;
}

export function runDemo(binaryArgument) {
  const binary = /[/\\]/.test(binaryArgument) ? resolve(binaryArgument) : binaryArgument;
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-bilag-demo-'));
  console.error(`Kun fiktive data. Input, kildelog og resultater: ${evidence}`);
  const save = (name, value) => saveEvidence(evidence, name, value);
  const run = (args, raw = false) => {
    const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024,
      env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
    if (p.stderr) process.stderr.write(p.stderr);
    assert.ifError(p.error); assert.equal(p.status, 0, `${args[0]} fejlede:\n${p.stdout}`);
    return raw ? p.stdout : JSON.parse(p.stdout);
  };
  const schema = run(['schema', model, '--entry', 'beregn_personskat', '--format', 'compact-json']);
  const template = run(['template', model, '--entry', 'beregn_personskat', '--format', 'json']);
  assert.equal(schema.schema_hash, template.$futuruna.schema_hash);
  const rehearsal = buildFictionalCases(template);
  // Preserve the actual generated questions/help/source traces beside the
  // authored classifications. Merely embedding their text would not test them.
  const paths = new Set(rehearsal.ledger.flatMap(row => row.mappings.map(m => m.path)));
  const guidance = schema.field_metadata.filter(f => [...paths].some(p => f.path === p || f.path.startsWith(`${p}.`)));
  for (const path of ['lønmodtager.bruttoløn_kroner', 'kapitalindkomst.renter.renteudgifter_kroner',
    'kapitalindkomst.renter.renteindtægter_kroner', 'lønmodtager.pension.atp.$variant']) {
    const field = guidance.find(f => f.path === path);
    assert.ok(field?.question && field.help && field.source_group, `Genereret vejledning mangler: ${path}`);
  }
  save('sources.json', sources);
  save('guidance.json', { schema_hash: schema.schema_hash, field_metadata: guidance,
    source_groups: schema.source_groups, source_objects: schema.source_objects });
  save('mapping.json', { fictional: true, contract: template.$futuruna, cases: rehearsal.ledger,
    held_before_call: rehearsal.held });
  const input = save('cases.json', rehearsal.envelope);
  console.error('Beregner fire fiktive sager med én worker; den femte er ikke indsendt.');
  save('results.json', run(['call', model, '--entry', 'beregn_personskat', '--input', input], true));
  const output = finishFictionalDemo(evidence, rehearsal.held);
  return { evidence, output, rehearsal };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  if (args.length !== 1 || args[0] === '--help') {
    console.log('Usage: node examples/danish-income-tax/bilag-demo.mjs PATH_TO_VERIFIED_RUNA');
    console.log('Kun de medfølgende fiktive kilder; ingen PDF-import, netværk eller installation.');
    process.exitCode = args[0] === '--help' ? 0 : 1;
  } else {
    runDemo(args[0]);
  }
}
