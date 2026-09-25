// Actual compact contract and fictional report rows; no PDF import, network,
// compiler build or independent tax calculation.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { parseReportOutput, renderReportOutput } from '../examples/danish-income-tax/afstemning-resultat.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
const model = 'examples/danish-income-tax/aarsopgoerelse-afstemning.calculate.runa';
const enabled = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build.' };
function run(args, status = 0) {
  const result = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 60000,
    maxBuffer: 8 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(result.error); assert.equal(result.status, status, result.stderr);
  return JSON.parse(result.stdout);
}
function save(directory, name, value) {
  const path = join(directory, name);
  writeFileSync(path, JSON.stringify(value, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
  return path;
}

test('report church-tax intake uses the report year and labels the loose membership-period bound', enabled, () => {
  const schema = run(['schema', model, '--format', 'compact-json']);
  const fields = schema.field_metadata.filter(f => f.path === 'betaler_kirkeskat');
  assert.equal(fields.length, 1);
  const field = fields[0];
  assert.match(field.question, /rapportens indkomstår/);
  for (const phrase of ['hele eller en del af året', 'Ukendt status må ikke blive false',
    'nulbeløb', 'helårs-overgrænse', 'ikke medlemskab i dag']) assert.ok(field.help.includes(phrase), phrase);
  const sources = schema.source_groups[field.source_group].map(id => schema.source_objects[id]);
  for (const url of ['https://www.borger.dk/kultur-og-fritid/medlemskab-af-folkekirken',
    'https://www.dst.dk/da/Statistik/dokumentation/Times/personindkomst/kiskat']) {
    assert.ok(sources.some(s => s.role === 'guidance' && JSON.stringify(s).includes(url)), url);
  }
});

test('report row fields retain context-specific signs, units, questions and source warnings', enabled, () => {
  const schema = run(['schema', model, '--format', 'compact-json']);
  const contexts = [
    ['skat.poster_uden_ægtefællenedslag', ['Skatter positive', 'negative', 'Ingen subtotaler', 'ægtefællenedslag']],
    ['betaling.tillæg_til_slutskat', ['Ikke-negative', 'før overskydende skat eller restskat', 'ikke allerede', 'Ikke renter']],
    ['betaling.korrektioner_til_udbetaling', ['positive', 'tidligere udbetalinger og modregninger negative', 'ikke slutskatstillæg']],
  ];
  const amounts = [];
  for (const [parent, phrases] of contexts) {
    for (const member of ['navn', 'beløb_øre']) {
      const path = `${parent}.${member}`;
      const matches = schema.field_metadata.filter(field => field.path === path);
      assert.equal(matches.length, 1, `missing row metadata: ${path}`);
      const field = matches[0];
      assert.ok(field.label.length > 0 && field.question?.length > 0, path);
      const sources = schema.source_groups[field.source_group]?.map(id => schema.source_objects[id]);
      assert.ok(sources?.some(source => source.role === 'source'), path);
      assert.ok(sources?.some(source => source.role === 'warning'
        && source.data.value.includes('ikke uafhængig skatteberegning')), path);
      if (member === 'navn') {
        assert.ok(field.help.includes('side/linje') && field.help.includes('CPR'), path);
      } else {
        assert.equal(field.unit, 'øre', path);
        for (const phrase of [...phrases, '12,34 kr. = 1234 øre', 'Ukendt er ikke 0']) {
          assert.ok(field.help.includes(phrase), `${path}: ${phrase}`);
        }
        amounts.push(field.help);
      }
    }
  }
  assert.equal(new Set(amounts).size, 3, 'one shared amount prompt cannot describe all three signs/roles');
});

test('fictional signed rows reach reconciliation and keep mistakes or unknowns visible', enabled, () => {
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-report-row-input-'));
  console.log(`Fictional report-row evidence: ${evidence}`);
  // Authored observations, not legal tax expectations or template defaults.
  const baseline = {
    skatteår: 2025, kommune: { $variant: 'København' }, betaler_kirkeskat: false,
    ægtefællens_kommune: null, gift_samlevende_ved_årets_udløb: false,
    indkomstbro: { personlig_indkomst_kroner: 100000, kapitalindkomst_kroner: 0,
      ligningsmæssige_fradrag_kroner: 10000, skattepligtig_indkomst_kroner: 90000,
      øvrige_indkomstreguleringer_kroner: 0, oplyst_ægtefælleunderskud_kroner: 0 },
    ægtefællenedslag: { statsligt_personfradrag_øre: 0, kommunalt_personfradrag_øre: 0,
      kirkeligt_personfradrag_øre: 0, kommunalt_og_kirkeligt_personfradrag_øre: null,
      negativ_kapitalindkomst_øre: 0, eget_negativ_kapitalindkomstnedslag_øre: 0 },
    skat: { poster_uden_ægtefællenedslag: [
      { navn: 'Fiktiv skat, side 1 linje 1', beløb_øre: 2000000 },
      { navn: 'Fiktiv egen fradragsværdi, side 1 linje 2', beløb_øre: -100000 },
    ], poster_uden_ægtefællenedslag_komplette: true, oplyst_beregnet_skat_øre: 1900000 },
    betaling: { oplyst_forskudsskat_øre: 2000000, oplyst_beregnet_skat_øre: 1900000,
      tillæg_til_slutskat: [{ navn: 'Fiktiv overført restskat, side 2 linje 1', beløb_øre: 50000 }],
      tillæg_til_slutskat_komplette: true, oplyst_overskydende_skat_øre: 50000,
      korrektioner_til_udbetaling: [
        { navn: 'Fiktiv godtgørelse, side 2 linje 2', beløb_øre: 1234 },
        { navn: 'Fiktiv tidligere udbetaling, side 2 linje 3', beløb_øre: -10001 },
      ], korrektioner_komplette: true, oplyst_udbetaling_kroner: 412, restskat: null },
  };
  const envelope = run(['template', model, '--format', 'json']);
  const expected = new Map();
  envelope.cases = [];
  function add(case_id, status, edit = () => {}) {
    const input = structuredClone(baseline); edit(input);
    envelope.cases.push({ case_id, input });
    if (status) expected.set(case_id, status);
  }
  add('signed-refund', 'BetingetAfstemt');
  add('addition-before-restskat', 'BetingetAfstemt', ({ betaling: b }) => {
    b.oplyst_forskudsskat_øre = 1900000; b.oplyst_overskydende_skat_øre = null;
    b.oplyst_udbetaling_kroner = null; b.korrektioner_til_udbetaling = [];
    b.restskat = { oplyst_restskat_øre: 50000 };
  });
  add('tax-credit-sign', 'Modstrid', x => { x.skat.poster_uden_ægtefællenedslag[1].beløb_øre = 100000; });
  add('previous-refund-sign', 'Modstrid', x => { x.betaling.korrektioner_til_udbetaling[1].beløb_øre = 10001; });
  add('kroner-in-ore-field', 'Modstrid', x => { x.betaling.korrektioner_til_udbetaling[0].beløb_øre = 12; });
  add('negative-addition', 'UgyldigtRapportinput', x => { x.betaling.tillæg_til_slutskat[0].beløb_øre = -50000; });
  add('unreviewed-tax-rows', 'Ufuldstændig', x => { x.skat.poster_uden_ægtefællenedslag_komplette = false; });
  add('unreviewed-additions', 'Ufuldstændig', x => { x.betaling.tillæg_til_slutskat_komplette = false; });
  add('duplicate-row', 'UgyldigtRapportinput', x => {
    x.betaling.korrektioner_til_udbetaling.push({ ...x.betaling.korrektioner_til_udbetaling[0] });
  });
  add('printed-danish-amount', null, x => { x.betaling.korrektioner_til_udbetaling[0].beløb_øre = '12,34'; });
  const output = run(['call', model, '--input', save(evidence, 'cases.json', envelope)], 1);
  save(evidence, 'results.json', output);
  assert.equal(output.results.length, expected.size);
  for (const { case_id, result } of output.results) {
    assert.equal(result.status.$variant, expected.get(case_id), case_id);
    assert.equal(result.uafhængig_skatteberegning_udført, false);
    if (result.status.$variant !== 'UgyldigtRapportinput') {
      assert.ok(result.uafklaret.some(w => w.includes('helårs-overgrænse')
        && w.includes('ikke medlemskab i dag') && w.includes('beviser ikke ret til nedslaget')));
    }
    expected.delete(case_id);
  }
  assert.equal(expected.size, 0);
  assert.ok(output.diagnostics.length > 0);
  assert.deepEqual(new Set(output.diagnostics.map(d => d.case_id)), new Set(['printed-danish-amount']));
  assert.ok(output.diagnostics.some(d => d.path.includes('beløb_øre')));
  const refund = output.results.find(row => row.case_id === 'signed-refund').result;
  const payout = refund.kontroller.find(row => row.navn.startsWith('Udbetaling efter'));
  assert.equal(payout.forventet, 412); assert.equal(payout.oplyst, 412); assert.equal(payout.difference, 0);
  const shown = renderReportOutput(parseReportOutput(JSON.stringify(output)));
  assert.equal(shown.exitCode, 2, 'the successful case must not hide the rejected/contradictory cases');
  for (const { case_id } of envelope.cases) assert.ok(shown.text.includes(case_id));
});
