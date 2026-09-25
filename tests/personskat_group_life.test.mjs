// Fictional payroll premiums; two independently observed 2025 public-form
// expectations. No private reports, live network, compiler build or JS tax.
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
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const path = 'lønmodtager.personlig_indkomst.ordinære_forhold.arbejdsgiverydelser';
const group = 'GruppelivSomUadskiltDelAfPbl19Ordning';
const aggregate = 'ArbejdsgiveradministreretSamlepostEfterPbl19EllerPbl56';
const gross = 'præmie_før_indeholdt_arbejdsmarkedsbidrag_kroner';
const aggregateGross = 'indberettet_beløb_før_indeholdt_arbejdsmarkedsbidrag_kroner';
const v = ($variant, fields = {}) => ({ $variant, ...fields });
// Main-person output is flat; the spouse retains the nested wage calculation.
const taxFields = (result, spouse) => spouse
  ? { ...result.ægtefælle.skat.indkomstgrundlag, ...result.ægtefælle.skat.ligningsfradrag }
  : result.skat;

test('payroll group life retains gross work deductions, net income and unknown-fact guards', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build or network.',
}, () => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-group-life-'));
  console.log(`Fictional group-life evidence: ${directory}`);
  const save = (name, value) => {
    const file = join(directory, name);
    writeFileSync(file, JSON.stringify(value) + '\n', { flag: 'wx', mode: 0o600 }); return file;
  };
  const run = args => {
    const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 600000,
      maxBuffer: 32 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
    assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
  };
  const envelope = run(['template', model, '--format', 'json']);
  // Reuse explicit fictional absences, not template zeros as personal facts.
  // Here the only pension-related payment is a non-deductible insurance premium.
  const base = buildFictionalCases(envelope).envelope.cases.find(c => c.case_id === 'arbejdsgiver-atp').input;
  base.lønmodtager.pension.pbl18_indbetalinger = [];
  base.lønmodtager.pension.atp = v('IngenAtpIndbetalinger');
  const specs = [
    { id: 'payroll-low', salary: 100000, gross: 1000, valid: true, total: 1975332 },
    { id: 'payroll-job', salary: 230000, gross: 1000, valid: true, total: 6879678 },
    { id: 'aggregate-known', salary: 100000, gross: 1000, valid: true, aggregate: true, total: 1975332 },
    { id: 'gross-unknown', salary: 100000, gross: null, valid: false },
    { id: 'gross-below-net', salary: 100000, gross: 919, valid: false },
    { id: 'aggregate-unknown', salary: 100000, gross: null, valid: false, aggregate: true },
    { id: 'spouse-payroll', salary: 100000, gross: 1000, valid: true, spouse: true },
    { id: 'spouse-gross-unknown', salary: 100000, gross: null, valid: false, spouse: true },
  ];
  envelope.cases = specs.map(c => {
    const input = structuredClone(base);
    input.lønmodtager.bruttoløn_kroner = c.salary;
    let person = input;
    if (c.spouse) {
      const names = ['lønmodtager', 'kapitalindkomst', 'aktieavance', 'udenlandske_sociale_bidrag',
        'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter'];
      person = Object.fromEntries(names.map(name => [name, structuredClone(input[name])]));
      input.ægtefælle = v('MedÆgtefælle', { fakta: person,
        samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] });
    }
    person.lønmodtager.personlig_indkomst.ordinære_forhold.arbejdsgiverydelser = [{
      identifikation: 'fictional-payroll-premium', indkomstår: 2025,
      ydelse: c.aggregate ? v(aggregate, { [aggregateGross]: c.gross,
        indberettet_beløb_efter_indeholdt_arbejdsmarkedsbidrag_kroner: 920 }) : v(group, {
        [gross]: c.gross, personlig_indkomst_efter_indeholdt_arbejdsmarkedsbidrag_kroner: 920 }),
    }];
    return { case_id: c.id, input };
  });
  const output = run(['call', model, '--input', save('cases.json', envelope)]);
  save('results.json', output);
  assert.deepEqual(output.diagnostics, []);
  assert.deepEqual(output.results.map(c => c.case_id), specs.map(c => c.id));
  const summary = output.results.map(({ case_id, result: r }, i) => {
    const s = taxFields(r, specs[i].spouse);
    return { case_id, valid: r.vurdering.alle_kontroller_gyldige,
      employment: s.beskæftigelsesfradrag_kroner, job: s.jobfradrag_kroner,
      extra: s.ekstra_pensionsfradrag_kroner, comparison_ore: r.vurdering.slutskat_til_sammenligning_øre };
  });
  save('summary.json', summary); console.log(JSON.stringify(summary));
  for (const [i, { case_id, result: r }] of output.results.entries()) {
    const c = specs[i], prefix = c.spouse ? 'ægtefælle.MedÆgtefælle.fakta.' : '';
    assert.equal(r.vurdering.alle_kontroller_gyldige, c.valid, case_id);
    const control = r.vurdering.kontroller.find(k => k.sti === prefix + path);
    assert.equal(control?.gyldig, c.valid, case_id);
    assert.ok(control.forklaring.includes('personskat-gruppeliv.md'), case_id);
    if (!c.valid) {
      assert.equal(r.vurdering.slutskat_til_sammenligning_øre, null, case_id);
      assert.ok(r.vurdering.fejl.some(k => k.sti === prefix + path), case_id);
      continue;
    }
    const p = c.spouse ? r.ægtefælle.grundlag : r;
    const s = taxFields(r, c.spouse);
    assert.equal(p.personlig_indkomst.arbejdsmarkedsbidragsgrundlag_med_indeholdt_bidrag_kroner, 1000, case_id);
    assert.equal(s.øvrig_personlig_indkomst_kroner, 920, case_id);
    assert.equal(s.arbejdsmarkedsbidrag_kroner, c.salary === 230000 ? 18400 : 8000, case_id);
    assert.equal(s.beskæftigelsesfradrag_kroner, c.salary === 230000 ? 28413 : 12423, case_id);
    assert.equal(s.jobfradrag_kroner, c.salary === 230000 ? 293 : 0, case_id);
    assert.equal(s.ekstra_pensionsfradrag_kroner, 0, 'non-deductible insurance is not a LL9L payment');
    if (c.total !== undefined) assert.equal(r.vurdering.slutskat_til_sammenligning_øre, c.total, case_id);
  }
  const schema = run(['schema', model, '--format', 'compact-json']);
  for (const prefix of ['', 'ægtefælle.MedÆgtefælle.fakta.']) {
    for (const suffix of [`${group}.${gross}`, `${aggregate}.${aggregateGross}`]) {
      const target = `${prefix}${path}.ydelse.${suffix}`;
      const matches = schema.field_metadata.filter(f => f.path === target);
      assert.equal(matches.length, 1, target);
      const field = matches[0];
      assert.ok(field.question.length > 0, target);
      for (const phrase of ['null', 'netto/0,92', 'personskat-gruppeliv.md']) {
        assert.ok(field.help.includes(phrase), `${target}: ${phrase}`);
      }
      const sources = schema.source_groups[field.source_group].map(id => schema.source_objects[id]);
      assert.ok(JSON.stringify(sources).includes('https://info.skat.dk/data.aspx?oid=2045911'), target);
    }
    const kind = schema.field_metadata.find(f => f.path === `${prefix}${path}.ydelse.$variant`);
    for (const phrase of ['felt 91', 'pensionsbonus', 'indtægtsart 37', 'rateoverskud']) {
      assert.ok(kind.help.includes(phrase), phrase);
    }
  }
});
