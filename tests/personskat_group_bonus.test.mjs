// Fictional source facts; independent 2025 public-form observations are
// comparison expectations only. No private reports, network or compiler build.
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
const path = 'lønmodtager.personlig_indkomst.ordinære_forhold.forenings_og_arbejdsløshedsydelser';
const branch = 'PersonskatGruppelivFraPensionsbonus';
const funding = 'GruppelivBetaltAfBonusPåFradragsberettigetPension';
const v = ($variant, fields = {}) => ({ $variant, ...fields });
const taxFields = (r, spouse) => spouse
  ? { ...r.ægtefælle.skat.indkomstgrundlag, ...r.ægtefælle.skat.ligningsfradrag } : r.skat;

test('pension-bonus group life adds income without work deductions and explains unresolved facts', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build or network.',
}, () => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-group-bonus-'));
  console.log(`Fictional bonus-premium evidence: ${directory}`);
  const save = (name, data) => {
    const file = join(directory, name);
    writeFileSync(file, JSON.stringify(data) + '\n', { flag: 'wx', mode: 0o600 }); return file;
  };
  const run = args => {
    const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 600000,
      maxBuffer: 32 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
    assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
  };
  const envelope = run(['template', model, '--format', 'json']);
  const base = buildFictionalCases(envelope).envelope.cases.find(c => c.case_id === 'arbejdsgiver-atp').input;
  // Explicit fictional changes to that baseline: no new pension deposits/ATP,
  // only a premium funded from an existing deductible pension's bonus.
  base.lønmodtager.pension.pbl18_indbetalinger = [];
  base.lønmodtager.pension.atp = v('IngenAtpIndbetalinger');
  const source = { document: 'Fiktiv pensionsmeddelelse', lines: {
    1: { year: 2025 },
    2: { text: 'Gruppeliv betalt af bonus på en fradragsberettiget pension, ikke løntræk.', financing: funding },
    3: { premium: 920, unit: 'kr./år' },
    4: { ordinaryDanishPremiumWithoutCorrections: true },
  } };
  save('fictional-source.json', source);
  const specs = [
    { id: 'bonus-low', salary: 100000, valid: true, total: 1978223, employment: 12300, job: 0 },
    { id: 'bonus-job', salary: 230000, valid: true, total: 6883626, employment: 28290, job: 248 },
    { id: 'bonus-spouse', salary: 100000, valid: true, spouse: true, employment: 12300, job: 0 },
    { id: 'unknown-funding', salary: 100000, valid: false, financing: 'GruppelivsfinansieringUoplystEllerUdenForModellen' },
    { id: 'payroll-is-not-bonus', salary: 100000, valid: false, financing: 'GruppelivBetaltAfLøntræk' },
    { id: 'insurance-bonus-is-distinct', salary: 100000, valid: false, financing: 'GruppelivBetaltAfBonusFraSelveForsikringen', spouse: true },
    { id: 'duplicate-premium', salary: 100000, valid: false, duplicate: true },
    { id: 'wrong-income-year', salary: 100000, valid: false, year: 2024 },
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
    const row = v(branch, { fakta: {
      identifikation: 'fictional-bonus-premium', indkomstår: c.year ?? source.lines[1].year,
      kildereference: 'Fiktiv pensionsmeddelelse:1–4',
      finansiering: v(c.financing ?? source.lines[2].financing),
      præmie_kroner: source.lines[3].premium,
      ordinær_dansk_præmie_uden_korrektioner: source.lines[4].ordinaryDanishPremiumWithoutCorrections,
    } });
    person.lønmodtager.personlig_indkomst.ordinære_forhold.forenings_og_arbejdsløshedsydelser = c.duplicate
      ? [row, structuredClone(row)] : [row];
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
    const control = r.vurdering.kontroller.find(k => k.sti === `${prefix}${path}.${branch}`);
    assert.equal(control?.gyldig, c.valid, case_id);
    assert.ok(control.forklaring.includes('personskat-gruppeliv.md'), case_id);
    assert.equal(r.vurdering.alle_kontroller_gyldige, c.valid, case_id);
    if (!c.valid) {
      assert.equal(r.vurdering.slutskat_til_sammenligning_øre, null, case_id);
      assert.ok(r.vurdering.fejl.some(k => k.sti === control.sti), case_id);
      continue;
    }
    const p = c.spouse ? r.ægtefælle.grundlag : r, s = taxFields(r, c.spouse);
    const row = p.personlig_indkomst.ordinære_forhold.forenings_og_arbejdsløshedsydelser[0];
    assert.equal(row.$variant, 'BeregnetGruppelivFraPensionsbonus');
    assert.equal(row.resultat.fakta.kildereference, 'Fiktiv pensionsmeddelelse:1–4');
    assert.equal(row.resultat.personlig_indkomst_uden_am_kroner, 920);
    assert.equal(row.resultat.ll9c_aftrapningsindkomst_kroner, 0);
    assert.equal(p.ligningsfradrag.befordring.aftrapningsindkomst_afklaret, true);
    assert.equal(p.ligningsfradrag.befordring.aftrapningsindkomst_kroner, c.salary);
    assert.equal(p.personlig_indkomst.arbejdsmarkedsbidragsgrundlag_med_indeholdt_bidrag_kroner, 0);
    assert.equal(p.personlig_indkomst.arbejdsmarkedsbidragsgrundlag_tillæg_kroner, 0);
    assert.equal(s.øvrig_personlig_indkomst_kroner, 920);
    assert.equal(s.arbejdsmarkedsbidrag_kroner, c.salary === 230000 ? 18400 : 8000);
    assert.equal(s.beskæftigelsesfradrag_kroner, c.employment);
    assert.equal(s.jobfradrag_kroner, c.job);
    assert.equal(s.ekstra_pensionsfradrag_kroner, 0);
    if (c.total !== undefined) assert.equal(r.vurdering.slutskat_til_sammenligning_øre, c.total, case_id);
  }
  const schema = run(['schema', model, '--format', 'compact-json']);
  const guidance = [];
  for (const prefix of ['', 'ægtefælle.MedÆgtefælle.fakta.']) {
    // A nullary enum is one canonical field, unlike a payload sum's $variant.
    for (const suffix of ['identifikation', 'indkomstår', 'kildereference', 'finansiering',
      'præmie_kroner', 'ordinær_dansk_præmie_uden_korrektioner']) {
      const target = `${prefix}${path}.${branch}.fakta.${suffix}`;
      const fields = schema.field_metadata.filter(f => f.path === target);
      assert.equal(fields.length, 1, target);
      const field = fields[0];
      assert.ok(field.question?.length > 0 && field.help?.length > 0, target);
      const sources = schema.source_groups[field.source_group].map(id => schema.source_objects[id]);
      assert.ok(JSON.stringify(sources).includes('https://info.skat.dk/data.aspx?oid=2045911'), target);
      if (suffix === 'finansiering') {
        for (const phrase of ['felt 38', 'indtægtsart 37', 'felt 91', 'selve forsikringen', 'Uoplyst']) {
          assert.ok(field.help.includes(phrase), `${target}: ${phrase}`);
        }
      }
      guidance.push({ ...field, resolved_sources: sources });
    }
    const collection = schema.field_metadata.find(f => f.path === `${prefix}${path}`);
    assert.ok(collection.help.includes(branch), 'the new route must be discoverable');
  }
  save('guidance.json', guidance);
});
