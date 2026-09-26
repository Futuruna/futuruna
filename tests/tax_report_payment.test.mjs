// Fictional report arithmetic, not independent tax/interest/collection law.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { parseReportOutput, renderReportOutput } from '../examples/danish-income-tax/afstemning-resultat.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
const model = 'examples/danish-income-tax/aarsopgoerelse-afstemning.calculate.runa';
const enabled = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build.' };
const v = $variant => ({ $variant });
function run(args) {
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 60000,
    maxBuffer: 8 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
}
function base() {
  // Explicit invented observations: no inferred personal facts or defaults.
  return {
    skatteår: 2025, kommune: v('København'), betaler_kirkeskat: false,
    ægtefællens_kommune: null, gift_samlevende_ved_årets_udløb: false,
    indkomstbro: { personlig_indkomst_kroner: 100000, kapitalindkomst_kroner: 0,
      ligningsmæssige_fradrag_kroner: 0, skattepligtig_indkomst_kroner: 100000,
      øvrige_indkomstreguleringer_kroner: 0, oplyst_ægtefælleunderskud_kroner: 0 },
    ægtefællenedslag: { statsligt_personfradrag_øre: 0, kommunalt_personfradrag_øre: 0,
      kirkeligt_personfradrag_øre: 0, kommunalt_og_kirkeligt_personfradrag_øre: null,
      negativ_kapitalindkomst_øre: 0, eget_negativ_kapitalindkomstnedslag_øre: 0 },
    skat: { poster_uden_ægtefællenedslag: [{ navn: 'Fiktiv samlet skat, side 1', beløb_øre: 2000000 }],
      poster_uden_ægtefællenedslag_komplette: true, oplyst_beregnet_skat_øre: 2000000 },
    betaling: { oplyst_forskudsskat_øre: 2200000, oplyst_beregnet_skat_øre: 2000000,
      tillæg_til_slutskat: [], tillæg_til_slutskat_komplette: true,
      oplyst_overskydende_skat_øre: 200000, korrektioner_til_udbetaling: [
        { navn: 'Fiktiv tidligere udbetalt, side 2', beløb_øre: -500000 },
      ], korrektioner_komplette: true, oplyst_udbetaling_kroner: null, restskat: null },
  };
}
function call(cases) {
  const dir = mkdtempSync(join(tmpdir(), 'futuruna-report-payment-'));
  console.log(`Fictional payment-balance evidence: ${dir}`);
  const envelope = run(['template', model, '--format', 'json']); envelope.cases = cases;
  const save = (name, value) => {
    const path = join(dir, name);
    writeFileSync(path, JSON.stringify(value, null, 2) + '\n', { flag: 'wx', mode: 0o600 }); return path;
  };
  const path = save('cases.json', envelope), before = readFileSync(path);
  const out = run(['call', model, '--input', path]); save('results.json', out);
  assert.deepEqual(readFileSync(path), before, 'source packet unchanged');
  assert.deepEqual(out.diagnostics, []);
  assert.deepEqual(out.results.map(r => r.case_id), cases.map(c => c.case_id));
  return out;
}
function named(rows, name) {
  const matches = rows.filter(r => r.navn === name); assert.equal(matches.length, 1, name); return matches[0];
}

test('legacy refund route does not turn a negative corrected payout into a refund', enabled, () => {
  const input = base(); input.betaling.oplyst_udbetaling_kroner = -3000;
  const out = call([{ case_id: 'legacy-negative-payout', input }]);
  const r = out.results[0].result;
  assert.equal(r.status.$variant, 'Ufuldstændig');
  assert.equal(named(r.kontroller, 'Udbetaling efter oplyste korrektioner og hele kroner').forventet, null);
  assert.equal(r.uafhængig_skatteberegning_udført, false);
});

test('generated report contract exposes an explicit final-payment interview', enabled, () => {
  const schema = run(['schema', model, '--format', 'compact-json']);
  const fields = schema.field_metadata.filter(f => f.path === 'betaling.slutbetaling');
  assert.equal(fields.length, 1);
  const field = fields[0]; assert.ok(field.question?.length > 0);
  for (const phrase of ['TilBetaling', 'TilUdbetaling', 'oplyst_beløb_kroner', 'null',
    'ikke restskat før', 'opkrævningsgrænser', 'aarsopgoerelse-afstemning.md']) {
    assert.ok(field.help.includes(phrase), phrase);
  }
  const sources = schema.source_groups[field.source_group].map(id => schema.source_objects[id]);
  assert.ok(sources.some(s => s.role === 'source'
    && JSON.stringify(s).includes('https://www.retsinformation.dk/eli/lta/2024/460')));
  const meta = run(['meta', '--json', model]);
  assert.deepEqual(meta.diagnostics, []);
  const anchor = meta.anchors.find(a => a.label === 'rapport_korrigeret_betaling');
  assert.ok(anchor);
  const attachments = anchor.references.flatMap(r => r.attachments);
  assert.ok(attachments.some(a => a.role === 'source' && a.value.includes('KSL § 62 A')));
  assert.ok(attachments.some(a => a.role === 'warning' && a.value.includes('ikke retmæssigheden')));
  const span = meta.spans.find(s => s.label === anchor.label);
  assert.ok(span.code_start_line < span.code_end_line);
  for (const name of ['rapport_korrigeret_betalingssaldo_øre', 'rapport_slutbetaling_kroner', 'rapport_betalingskontroller']) {
    assert.ok(span.symbols.some(s => s.kind === 'rule' && s.name === name), name);
  }
});

test('revised report separates annual balance from final cash direction and preserves unknowns', enabled, () => {
  const cases = [], expected = new Map();
  function add(case_id, status, edit = () => {}, amount = 3000, ore = 300000) {
    const input = base();
    input.betaling.slutbetaling = { retning: v('TilBetaling'), oplyst_beløb_kroner: 3000 };
    edit(input.betaling);
    cases.push({ case_id, input }); expected.set(case_id, { status, amount, ore });
  }
  add('smaller-refund-requires-repayment', 'BetingetAfstemt');
  add('larger-refund-extra-payout', 'BetingetAfstemt', b => {
    b.korrektioner_til_udbetaling[0].beløb_øre = -100000;
    b.slutbetaling = { retning: v('TilUdbetaling'), oplyst_beløb_kroner: 1000 };
  }, 1000, 100000);
  function annualDebt(b) {
    b.oplyst_forskudsskat_øre = 1800000;
    b.oplyst_overskydende_skat_øre = null;
    b.restskat = { oplyst_restskat_øre: 200000 };
  }
  add('smaller-restskat-refunds-prior-payment', 'BetingetAfstemt', b => {
    annualDebt(b);
    b.korrektioner_til_udbetaling = [{ navn: 'Fiktiv tidligere betalt restskat', beløb_øre: 500000 }];
    b.slutbetaling = { retning: v('TilUdbetaling'), oplyst_beløb_kroner: 3000 };
  });
  add('new-restskat-and-prior-refund', 'BetingetAfstemt', b => {
    annualDebt(b); b.slutbetaling.oplyst_beløb_kroner = 7000;
  }, 7000, 700000);
  add('restskat-with-reported-interest-and-part-payment', 'BetingetAfstemt', b => {
    annualDebt(b);
    b.korrektioner_til_udbetaling = [
      { navn: 'Fiktiv særskilt indbetaling', beløb_øre: 100000 },
      { navn: 'Fiktivt skyldigt tillæg', beløb_øre: -10001 },
    ];
    b.slutbetaling.oplyst_beløb_kroner = 1100;
  }, 1100, 110001);
  add('addition-before-corrected-balance', 'BetingetAfstemt', b => {
    b.tillæg_til_slutskat = [{ navn: 'Fiktivt overført beløb', beløb_øre: 50000 }];
    b.oplyst_overskydende_skat_øre = 150000;
    b.slutbetaling.oplyst_beløb_kroner = 3500;
  }, 3500, 350000);
  for (const balance of [-101, -100, -99, -1, 0, 1, 99, 100, 101]) {
    const direction = balance < 0 ? 'TilBetaling' : 'TilUdbetaling';
    const magnitude = Math.abs(balance), kroner = Math.floor(magnitude / 100);
    add(`ore-boundary-${balance}`, 'BetingetAfstemt', b => {
      b.korrektioner_til_udbetaling[0].beløb_øre = balance - 200000;
      b.slutbetaling = { retning: v(direction), oplyst_beløb_kroner: kroner };
    }, kroner, magnitude);
  }
  for (const balance of [-1, 1]) {
    add(`wrong-direction-before-rounding-${balance}`, 'Modstrid', b => {
      b.korrektioner_til_udbetaling[0].beløb_øre = balance - 200000;
      b.slutbetaling = { retning: v(balance < 0 ? 'TilUdbetaling' : 'TilBetaling'), oplyst_beløb_kroner: 0 };
    }, null, -1);
  }
  add('wrong-direction', 'Modstrid', b => { b.slutbetaling.retning = v('TilUdbetaling'); }, null, -300000);
  add('wrong-final-amount', 'Modstrid', b => { b.slutbetaling.oplyst_beløb_kroner = 2999; });
  add('ore-in-kroner-field', 'Modstrid', b => { b.slutbetaling.oplyst_beløb_kroner = 300000; });
  add('missing-final-observation', 'Ufuldstændig', b => { b.slutbetaling.oplyst_beløb_kroner = null; });
  add('unknown-corrections', 'Ufuldstændig', b => { b.korrektioner_komplette = false; }, null, null);
  add('unknown-additions', 'Ufuldstændig', b => { b.tillæg_til_slutskat_komplette = false; }, null, null);
  add('unknown-prepaid', 'Ufuldstændig', b => { b.oplyst_forskudsskat_øre = null; }, null, null);
  add('unknown-assessed-tax', 'Ufuldstændig', b => { b.oplyst_beregnet_skat_øre = null; }, null, null);
  add('missing-annual-observation', 'Ufuldstændig', b => { b.oplyst_overskydende_skat_øre = null; });
  add('wrong-annual-direction', 'Modstrid', b => {
    b.oplyst_forskudsskat_øre = 1800000;
    b.oplyst_overskydende_skat_øre = -200000;
    b.slutbetaling.oplyst_beløb_kroner = 7000;
  }, 7000, 700000);
  add('negative-final-magnitude', 'UgyldigtRapportinput', b => { b.slutbetaling.oplyst_beløb_kroner = -3000; });
  add('overlapping-legacy-payout-even-zero', 'UgyldigtRapportinput', b => { b.oplyst_udbetaling_kroner = 0; });
  add('overlapping-annual-directions', 'UgyldigtRapportinput', b => { b.restskat = { oplyst_restskat_øre: 0 }; });
  add('duplicate-correction', 'UgyldigtRapportinput', b => { b.korrektioner_til_udbetaling.push({ ...b.korrektioner_til_udbetaling[0] }); });

  const out = call(cases);
  for (const [{ case_id, result: r }, { input }] of out.results.map((r, i) => [r, cases[i]])) {
    const e = expected.get(case_id);
    assert.equal(r.status.$variant, e.status, case_id);
    assert.equal(r.uafhængig_skatteberegning_udført, false);
    if (e.status === 'UgyldigtRapportinput') { assert.deepEqual(r.kontroller, []); continue; }
    const paying = input.betaling.slutbetaling.retning.$variant === 'TilBetaling';
    const control = named(r.kontroller,
      `${paying ? 'Til betaling' : 'Til udbetaling'} efter oplyste korrektioner og hele kroner`);
    assert.equal(control.forventet, e.amount, case_id);
    assert.equal(control.oplyst, input.betaling.slutbetaling.oplyst_beløb_kroner, case_id);
    assert.equal(control.difference, control.oplyst === null || e.amount === null ? null : control.oplyst - e.amount);
    assert.equal(r.kontroller.some(c => c.navn === 'Udbetaling efter oplyste korrektioner og hele kroner'), false);
    const direction = r.nødvendige_forudsætninger.filter(f => f.navn === 'Korrigeret saldo i den oplyste betalingsretning');
    if (e.ore === null) { assert.deepEqual(direction, [], case_id); }
    else {
      assert.equal(direction.length, 1);
      assert.equal(direction[0].nødvendigt_beløb, e.ore, case_id);
      assert.equal(direction[0].inden_for_kontrollerede_grænser, e.ore >= 0, case_id);
    }
    assert.ok(r.uafklaret.some(w => w.includes('KSL § 62 C') && w.includes('ikke et krav om betaling nu')));
  }
  const displayed = renderReportOutput(parseReportOutput(JSON.stringify(out)));
  assert.equal(displayed.exitCode, 2);
  for (const c of cases) assert.ok(displayed.text.includes(c.case_id));
  assert.ok(displayed.text.includes('Til betaling efter oplyste korrektioner'));
  assert.ok(displayed.text.includes('Til udbetaling efter oplyste korrektioner'));
  const first = { ...out, results: out.results.slice(0, 1) };
  const shown = renderReportOutput(parseReportOutput(JSON.stringify(first)));
  assert.equal(shown.exitCode, 0);
  assert.ok(shown.text.includes('3.000 DKK'));
});
