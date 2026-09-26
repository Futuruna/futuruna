// Final part-year assessment and real metadata through the public contract.
// Fictional inputs; recorded model controls, not an independent legal oracle.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { buildFictionalCases } from '../examples/danish-income-tax/bilag-demo.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
const model = 'examples/danish-income-tax/personskat-par14.calculate.runa';
const enabled = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no build, network or personal files.' };
const v = $variant => ({ $variant });
function run(args) {
  // Cold contract validation can exceed ten wall-clock minutes on the shared
  // 8 GB host. Assertions and the single calculation worker remain unchanged.
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 1200000,
    maxBuffer: 64 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr);
  return JSON.parse(p.stdout);
}
function save(directory, name, value) {
  const path = join(directory, name);
  writeFileSync(path, JSON.stringify(value, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
  return path;
}

test('part-year comparison uses the final assessment, never a nested ordinary amount or invalid zero', enabled, () => {
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-partyear-validity-'));
  console.log(`Fictional part-year evidence: ${evidence}`);
  const envelope = run(['template', model, '--entry', 'beregn_personskat_delår', '--format', 'json']);
  // Reuse construction only, not the original single-person source ledger.
  // New fiction: 2025 full liability ends June30; salary300000 during that
  // period, no other income/spouse/ATP/property/losses/relief/pension/payouts.
  const person = buildFictionalCases({ cases: [{ input: envelope.cases[0].input.personskat }] })
    .envelope.cases[0].input;
  person.lønmodtager.bruttoløn_kroner = 300000;
  person.lønmodtager.pension.pbl18_indbetalinger = [];
  const baseline = { personskat: person, skattepligtsændring: v('FuldSkattepligtOphører'),
    skattepligtsperiode: { fra_dato: { år: 2025, måned: 1, dag: 1 }, til_dato: { år: 2025, måned: 6, dag: 30 } },
    valg_afgivet_ved_oplysninger: false, omvalg_dato: null,
    kilder: [{ identifikation: 'fiktiv-loen', beregningsfelt: v('Par14Bruttoløn'), delårsbeløb_kroner: 300000,
      omregningsmetode: v('Par14ForholdsmæssigtLøbendeBeløb'), faktisk_helårsbeløb_kroner: null }],
    helårsgrundlag: v('AfledtFraIdentificeredeKilder') };
  const edits = [
    ['valid', () => {}],
    ['source-mismatch', x => { x.kilder[0].delårsbeløb_kroner = 299999; }],
    ['period-mismatch', x => { x.skattepligtsperiode.fra_dato.dag = 2; }],
    ['actual-annual-unknown', x => { x.valg_afgivet_ved_oplysninger = true; }],
    ['atp-unknown', x => { x.personskat.lønmodtager.pension.atp = v('AtpUoplyst'); }],
    ['church-unknown', x => { x.personskat.lønmodtager.kirkeskat = v('KirkeskatUoplyst'); }],
    ['church-part-year', x => { x.personskat.lønmodtager.kirkeskat = v('KirkeskatEnDelAfÅret'); }],
  ];
  envelope.cases = edits.map(([case_id, edit]) => { const input = structuredClone(baseline); edit(input); return { case_id, input }; });
  const output = run(['call', model, '--entry', 'beregn_personskat_delår', '--input', save(evidence, 'cases.json', envelope)]);
  save(evidence, 'results.json', output);
  assert.deepEqual(output.diagnostics, []);
  assert.deepEqual(output.results.map(row => row.case_id), edits.map(([id]) => id));
  for (const { case_id, result: r } of output.results) {
    const valid = case_id === 'valid', a = r.vurdering;
    assert.ok(a, `${case_id}: missing final part-year assessment`);
    assert.equal(r.input_gyldigt, valid, case_id);
    assert.equal(a.alle_kontroller_gyldige, r.input_gyldigt, case_id);
    assert.equal(a.status.$variant, valid ? 'BeregnetMedForbehold' : 'UgyldigtBeregningsgrundlag', case_id);
    assert.equal(a.samlet_modeldækning_bekræftet, false);
    assert.equal(a.slutskat_til_sammenligning_øre, valid ? 10383101 : null, case_id);
    assert.equal(a.kontrolgrundlag.beregning.length, 11);
    assert.equal(a.kontrolgrundlag.afregning.length, 1);
    assert.equal(a.kontroller.length, 12);
    assert.deepEqual(a.fejl, a.kontroller.filter(c => !c.gyldig));
    assert.equal(a.kontroller.every(c => c.gyldig), valid);
    for (const warning of r.delårsresultat.vurdering.forbehold) assert.ok(a.forbehold.includes(warning), warning);
    assert.ok(a.forbehold.some(w => w.includes('delårsresultat.vurdering') && w.includes('mellemtrin')));
    assert.ok(a.forbehold.some(w => w.includes('ikke restskat')));
    if (valid) {
      assert.equal(r.slutskat_efter_par14_øre, 10383101);
      assert.equal(r.delårsresultat.vurdering.slutskat_til_sammenligning_øre, 9433144);
      assert.notEqual(a.slutskat_til_sammenligning_øre, r.delårsresultat.vurdering.slutskat_til_sammenligning_øre);
      assert.equal(r.skattepligtsdage, 181);
    } else {
      assert.equal(r.slutskat_efter_par14_øre, 0, 'retain raw diagnostic convention, not a usable zero-tax conclusion');
      const path = case_id === 'source-mismatch' || case_id === 'actual-annual-unknown' ? 'kilder'
        : case_id === 'period-mismatch' ? 'skattepligtsperiode' : 'personskat';
      assert.ok(a.fejl.some(f => f.sti === path), case_id);
    }
    if (case_id === 'source-mismatch') {
      assert.equal(r.delårsresultat.vurdering.alle_kontroller_gyldige, true,
        'a valid intermediate result must not override failed part-year reconciliation');
    }
    if (case_id.startsWith('church-')) {
      assert.ok(r.delårsresultat.vurdering.fejl.some(f => f.sti === 'lønmodtager.kirkeskat'));
      assert.equal(r.delårsresultat.vurdering.slutskat_til_sammenligning_øre, null);
    }
  }
});

test('generated part-year intake distinguishes liability from employment and links the final assessment warning', enabled, () => {
  const schema = run(['schema', model, '--entry', 'beregn_personskat_delår', '--format', 'compact-json']);
  // A nullary enum is one canonical field, not a payload discriminator column.
  const field = schema.field_metadata.find(f => f.path === 'skattepligtsændring');
  assert.ok(field?.question.includes('skattepligt'));
  assert.ok(field.help.includes('Færre lønmåneder betyder ikke i sig selv delårsskattepligt'));
  const sources = schema.source_groups[field.source_group]?.map(id => schema.source_objects[id]);
  assert.ok(sources?.some(s => s.role === 'source'));
  assert.ok(sources.some(s => s.role === 'guidance'));
  assert.ok(sources.some(s => s.role === 'warning' && s.data.value.includes('delårsresultat.vurdering')));
});
