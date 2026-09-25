// Fictional property facts: exclusion from one provision is not tax exemption.
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
const enabled = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no build or private documents.' };
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const path = 'kapitalindkomst.ejendomsdrift';
const v = $variant => ({ $variant });
function run(args) {
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 1200000,
    maxBuffer: 64 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
}
function save(dir, name, value) {
  const file = join(dir, name);
  writeFileSync(file, JSON.stringify(value), { flag: 'wx', mode: 0o600 }); return file;
}
function baseline(envelope) {
  const input = buildFictionalCases(envelope).envelope.cases[0].input;
  // Independent fiction: remove the source demo's private pension.
  input.lønmodtager.pension.pbl18_indbetalinger = [];
  return input;
}
function property(amount, commercial = true, category = 'EjskEnBoligenhed') {
  return { $variant: 'MedEjendomsdriftEfterPar4Nr6', fakta: {
    kategori: v(category), beliggenhed: v('EjskDanmark'), erhvervsmæssigt_udlejet: commercial,
    særlige_betingelser_for_nr6_til_nr8_opfyldt: true, overskud_eller_underskud_kroner: amount,
  } };
}
function addSpouse(input) {
  const fakta = Object.fromEntries(['lønmodtager', 'kapitalindkomst', 'aktieavance',
    'udenlandske_sociale_bidrag', 'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter']
    .map(key => [key, structuredClone(input[key])]));
  input.ægtefælle = { $variant: 'MedÆgtefælle', fakta,
    samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] };
  return fakta;
}

test('nonzero commercial property result cannot disappear from a valid annual comparison', enabled, () => {
  const dir = mkdtempSync(join(tmpdir(), 'futuruna-property-route-'));
  console.log(`Fictional property-route evidence: ${dir}`);
  const envelope = run(['template', model, '--format', 'json']);
  const base = baseline(envelope), misplaced = structuredClone(base);
  misplaced.kapitalindkomst.ejendomsdrift = property(20000);
  envelope.cases = [{ case_id: 'baseline', input: base }, { case_id: 'commercial', input: misplaced }];
  const out = run(['call', model, '--input', save(dir, 'input.json', envelope)]);
  save(dir, 'results.json', out);
  assert.deepEqual(out.diagnostics, []);
  assert.deepEqual(out.results.map(r => r.case_id), ['baseline', 'commercial']);
  const [known, result] = out.results.map(r => r.result);
  const leaf = result.kapitalindkomst.ejendomsdrift_resultat;
  assert.deepEqual(leaf.fakta, misplaced.kapitalindkomst.ejendomsdrift.fakta);
  assert.equal(leaf.par4_resultat.omfattet_af_kapitalindkomst, false);
  assert.equal(leaf.par4_resultat.ikke_omfattet_af_par4_nr6_kroner, 20000);
  assert.equal(known.vurdering.slutskat_til_sammenligning_øre, 21194454);
  console.log(JSON.stringify({ omitted_kroner: 20000,
    baseline_øre: known.vurdering.slutskat_til_sammenligning_øre,
    commercial_valid: result.vurdering.alle_kontroller_gyldige,
    commercial_øre: result.vurdering.slutskat_til_sammenligning_øre }));
  assert.equal(result.vurdering.alle_kontroller_gyldige, false);
  assert.equal(result.vurdering.slutskat_til_sammenligning_øre, null);
  assert.equal(result.kapitalindkomst.alle_input_gyldige, false);
  assert.ok(result.vurdering.fejl.some(f => f.sti === path));
});

test('covered gains and losses survive while excluded nonzero main and spouse amounts are withheld', enabled, () => {
  const dir = mkdtempSync(join(tmpdir(), 'futuruna-property-boundaries-'));
  console.log(`Fictional property boundaries: ${dir}`);
  const envelope = run(['template', model, '--format', 'json']), base = baseline(envelope);
  const specs = [
    ['covered-profit', 25000, false, 'EjskLandzoneOver5000M2', false, true],
    ['covered-loss', -6000, false, 'EjskLandzoneOver5000M2', false, true],
    ['commercial-loss', -6000, true, 'EjskEnBoligenhed', false, false],
    ['excluded-category', 16000, false, 'EjskEjerlejlighedTreTilSeks', false, false],
    ['excluded-zero', 0, true, 'EjskEnBoligenhed', false, true],
    ['spouse-commercial', 20000, true, 'EjskEnBoligenhed', true, false],
    ['spouse-covered', 25000, false, 'EjskLandzoneOver5000M2', true, true],
  ];
  envelope.cases = specs.map(([case_id, amount, commercial, category, spouse]) => {
    const input = structuredClone(base), person = spouse ? addSpouse(input) : input;
    person.kapitalindkomst.ejendomsdrift = property(amount, commercial, category);
    return { case_id, input };
  });
  const out = run(['call', model, '--input', save(dir, 'input.json', envelope)]);
  save(dir, 'results.json', out); assert.deepEqual(out.diagnostics, []);
  assert.deepEqual(out.results.map(r => r.case_id), specs.map(s => s[0]));
  for (const [i, { case_id, result: r }] of out.results.entries()) {
    const [, amount, , , spouse, valid] = specs[i];
    const capital = spouse ? r.ægtefælle.grundlag.kapitalindkomst : r.kapitalindkomst;
    const sourcePerson = spouse ? envelope.cases[i].input.ægtefælle.fakta : envelope.cases[i].input;
    assert.deepEqual(capital.ejendomsdrift_resultat.fakta, sourcePerson.kapitalindkomst.ejendomsdrift.fakta, case_id);
    assert.equal(capital.alle_input_gyldige, valid, case_id);
    assert.equal(r.vurdering.alle_kontroller_gyldige, valid, case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre, valid ? r.slutskat_øre : null, case_id);
    assert.equal(capital.kapitalindkomst_resultat.nettokapitalindkomst_kroner, valid ? amount : 0, case_id);
    assert.equal(capital.ejendomsdrift_resultat.par4_resultat.ikke_omfattet_af_par4_nr6_kroner,
      valid ? 0 : amount, case_id);
    if (!valid) assert.ok(r.vurdering.fejl.some(f => f.sti === `${spouse ? 'ægtefælle.MedÆgtefælle.fakta.' : ''}${path}`
      && f.forklaring.includes('ikke automatisk skattefrit') && f.forklaring.includes('personskat-ejendomsdrift.md')), case_id);
  }
  assert.equal(out.results.find(r => r.case_id === 'excluded-zero').result.vurdering.slutskat_til_sammenligning_øre, 21194454);
  const schema = run(['schema', model, '--format', 'compact-json']);
  const fields = [];
  for (const prefix of ['', 'ægtefælle.MedÆgtefælle.fakta.']) {
    for (const member of ['$variant', ...['kategori', 'beliggenhed', 'erhvervsmæssigt_udlejet',
      'særlige_betingelser_for_nr6_til_nr8_opfyldt', 'overskud_eller_underskud_kroner']
      .map(name => `MedEjendomsdriftEfterPar4Nr6.fakta.${name}`)]) {
      const fieldPath = `${prefix}${path}.${member}`;
      const matches = schema.field_metadata.filter(f => f.path === fieldPath);
      assert.equal(matches.length, 1, fieldPath);
      const [field] = matches; fields.push(field);
      assert.equal(field.anchor, 'PersonskatEjendomsdriftInput');
      assert.ok(field.question?.length > 0);
      const sources = schema.source_groups[field.source_group].map(id => schema.source_objects[id]);
      assert.ok(sources.some(s => s.role === 'guidance' && JSON.stringify(s).includes('du-udlejer-en-bolig-som-du-ikke-selv-bor-i')));
      assert.ok(sources.some(s => s.role === 'warning' && JSON.stringify(s).includes('ikke automatisk skattefrit')));
      if (member === '$variant' || member.endsWith('overskud_eller_underskud_kroner')) {
        assert.ok(field.help.includes('tilbageholder'));
        assert.ok(field.help.includes('personskat-ejendomsdrift.md'));
      }
    }
  }
  save(dir, 'fields.json', fields);
});

test('part-year period and documented annual bases retain main and spouse property route failures', enabled, () => {
  const file = 'examples/danish-income-tax/personskat-par14.calculate.runa', entry = 'beregn_personskat_delår';
  const dir = mkdtempSync(join(tmpdir(), 'futuruna-property-partyear-'));
  console.log(`Fictional property part-year: ${dir}`);
  const envelope = run(['template', file, '--entry', entry, '--format', 'json']);
  const person = baseline({ cases: [{ input: envelope.cases[0].input.personskat }] });
  person.lønmodtager.bruttoløn_kroner = 300000;
  const annual = structuredClone(person); annual.lønmodtager.bruttoløn_kroner = 604972;
  const base = { personskat: person, skattepligtsændring: v('FuldSkattepligtOphører'),
    skattepligtsperiode: { fra_dato: { år: 2025, måned: 1, dag: 1 }, til_dato: { år: 2025, måned: 6, dag: 30 } },
    valg_afgivet_ved_oplysninger: false, omvalg_dato: null,
    kilder: [{ identifikation: 'fiktiv-loen', beregningsfelt: v('Par14Bruttoløn'), delårsbeløb_kroner: 300000,
      omregningsmetode: v('Par14ForholdsmæssigtLøbendeBeløb'), faktisk_helårsbeløb_kroner: null }],
    helårsgrundlag: { $variant: 'DokumenteretHelårsPersonskat', personskat: annual } };
  const ids = ['known', 'period-misrouted', 'annual-misrouted', 'couple-known', 'spouse-period-misrouted', 'spouse-annual-misrouted'];
  envelope.cases = ids.map(case_id => {
    const input = structuredClone(base);
    const couple = case_id.startsWith('spouse-') || case_id === 'couple-known';
    if (couple) { addSpouse(input.personskat); addSpouse(input.helårsgrundlag.personskat); }
    if (case_id.endsWith('misrouted')) {
      let target = case_id.includes('annual') ? input.helårsgrundlag.personskat : input.personskat;
      if (couple) target = target.ægtefælle.fakta;
      target.kapitalindkomst.ejendomsdrift = property(20000);
    }
    return { case_id, input };
  });
  const out = run(['call', file, '--entry', entry, '--input', save(dir, 'input.json', envelope)]);
  save(dir, 'results.json', out); assert.deepEqual(out.diagnostics, []);
  assert.deepEqual(out.results.map(r => r.case_id), ids);
  for (const { case_id, result: r } of out.results) {
    const valid = case_id.endsWith('known');
    assert.equal(r.vurdering.alle_kontroller_gyldige, valid, case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre, valid ? r.slutskat_efter_par14_øre : null, case_id);
    if (case_id === 'known') assert.equal(r.vurdering.slutskat_til_sammenligning_øre, 10383101);
    if (case_id.includes('annual-misrouted')) assert.equal(r.helårsgrundlag_gyldigt, false, case_id);
    if (case_id.includes('period-misrouted')) assert.ok(r.delårsresultat.vurdering.fejl.some(f =>
      f.sti === `${case_id.startsWith('spouse-') ? 'ægtefælle.MedÆgtefælle.fakta.' : ''}${path}`), case_id);
  }
});

test('documented ordinary business facts give a usable alternative without automatic reclassification', enabled, () => {
  const dir = mkdtempSync(join(tmpdir(), 'futuruna-property-business-'));
  console.log(`Fictional ordinary business route: ${dir}`);
  const envelope = run(['template', model, '--format', 'json']), input = baseline(envelope);
  // Additional explicitly invented source facts, NOT a guessed decomposition
  // of the earlier net amount: rent30000, eligible operating costs10000,
  // ordinary business without VSO, no interest/other property taxes here.
  input.lønmodtager.personlig_indkomst.ordinære_forhold.virksomheder_uden_virksomhedsordning = [{
    identifikation: 'fiktiv-erhvervsudlejning', indkomstår: 2025,
    indtægter: [{ identifikation: 'fiktiv-leje', art: v('OrdinærDriftsindtægt'), beløb_kroner: 30000 }],
    udgifter: [{ identifikation: 'fiktive-driftsudgifter', afgrænsning: v('SelvstændigErhvervsindkomstUdgift'), beløb_kroner: 10000 }],
    ligningslovsfradrag_efter_par3_stk2_nr2: [], erhvervsposter_efter_par3_stk2_nr4_til_10: [],
  }];
  const duplicate = structuredClone(input); duplicate.kapitalindkomst.ejendomsdrift = property(20000);
  envelope.cases = [{ case_id: 'correct-route', input }, { case_id: 'unresolved-second-route', input: duplicate }];
  const out = run(['call', model, '--input', save(dir, 'input.json', envelope)]);
  save(dir, 'results.json', out); assert.deepEqual(out.diagnostics, []);
  assert.deepEqual(out.results.map(r => r.case_id), ['correct-route', 'unresolved-second-route']);
  const [good, unresolved] = out.results.map(r => r.result);
  assert.equal(good.vurdering.alle_kontroller_gyldige, true);
  assert.equal(good.vurdering.slutskat_til_sammenligning_øre, good.slutskat_øre);
  assert.ok(good.slutskat_øre > 21194454, 'profit must reach tax, not vanish');
  assert.equal(good.skat.bruttoløn_kroner, 620000);
  assert.equal(good.skat.arbejdsmarkedsbidrag_kroner, 49600);
  assert.equal(good.skat.personlig_indkomst_efter_am_kroner, 570400);
  assert.equal(good.kapitalindkomst.kapitalindkomst_resultat.nettokapitalindkomst_kroner, 0);
  assert.equal(unresolved.vurdering.slutskat_til_sammenligning_øre, null);
  assert.ok(unresolved.vurdering.fejl.some(f => f.sti === path));
  console.log(JSON.stringify({ business_profit_kroner: 20000, tax_øre: good.slutskat_øre,
    am_kroner: good.skat.arbejdsmarkedsbidrag_kroner }));
});
