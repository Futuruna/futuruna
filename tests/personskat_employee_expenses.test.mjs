// Fictional facts only. Exercise the public calculation boundary, not a JS tax formula.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
const variant = ($variant, fields = {}) => ({ $variant, ...fields });
const expensePath = 'lønmodtager.ligningsfradrag.øvrige_lønmodtagerudgifter';
const shareField = 'udgiftsart.Ll9Stk1DriftsmiddelAfskrivning.erhvervsmæssig_andel_basispoint';

test('employee expense shares distinguish invalid facts from no entitlement for taxpayer and spouse', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build or network.',
}, () => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-employee-expenses-'));
  console.log(`Fictional employee-expense evidence: ${directory}`);
  const save = (name, data) => {
    const path = join(directory, name);
    writeFileSync(path, JSON.stringify(data) + '\n', { flag: 'wx', mode: 0o600 });
    return path;
  };
  const run = args => {
    const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 600000,
      maxBuffer: 32 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
    assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
  };
  const envelope = run(['template', model, '--entry', 'beregn_personskat', '--format', 'json']);
  const base = envelope.cases[0].input;
  // Explicit invented adult, whole-year Danish employee; no other income,
  // pensions/ATP, property, losses or deductions. Never infer these absences
  // from a real person's template or from an absent report line.
  Object.assign(base.lønmodtager, { skatteår: 2025, bruttoløn_kroner: 600000,
    kommune: variant('København'), betaler_kirkeskat: false });
  Object.assign(base.lønmodtager.pension, { fødselsdato: { år: 1980, måned: 1, dag: 1 },
    atp: variant('IngenAtpIndbetalinger'),
    udbetalingsoplysninger: { for_året_komplette: true, for_foregående_år_komplette: true } });
  Object.assign(base.lønmodtager.ligningsfradrag, {
    boligjob: variant('IngenBoligjobudgifter'), enlig_forsørger: variant('IntetEkstraBørnetilskud'),
    arbejdsfradrag_udland: variant('IngenUdlandsudelukkelseIFællesForhold', {
      dbo_hjemmehørende_udland_i_nogen_periode: false, noget_arbejde_udført_udland: null,
      nogen_udenlandsk_arbejdsgiver: null, kildereference: 'Fiktiv DBO-hjemmehørende DK hele året',
    }),
  });
  const cases = [];
  const add = (case_id, share, expected, spouse = false, clothes = false) => {
    const input = structuredClone(base);
    let person = input;
    if (spouse) {
      const fields = ['lønmodtager', 'kapitalindkomst', 'aktieavance', 'udenlandske_sociale_bidrag',
        'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter'];
      person = Object.fromEntries(fields.map(f => [f, structuredClone(input[f])]));
      input.ægtefælle = variant('MedÆgtefælle', { fakta: person,
        samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] });
    }
    person.lønmodtager.ligningsfradrag.øvrige_lønmodtagerudgifter = {
      skatteyderstatus: variant('Ll9Stk1Lønmodtager'),
      udgifter: [{ identifikation: 'fiktiv-udgift', indkomstår: 2025,
        arbejdsforhold_identifikation: 'fiktivt-arbejde',
        udgiftsart: clothes ? variant('Ll9Stk1SærligtArbejdstøj', {
          særligt_til_arbejdet: true, kan_anvendes_som_almindeligt_tøj: true,
        }) : variant('Ll9Stk1DriftsmiddelAfskrivning', { erhvervsmæssig_andel_basispoint: share }),
        afholdt_eller_beregnet_beløb_kroner: 20000,
        arbejdsgiver_refunderet_efter_regning_kroner: 0,
        dokumentation: variant('Ll9Stk1DokumenteretMedBilag'),
      }],
    };
    cases.push({ case_id, input, share, expected, spouse, clothes });
  };
  add('half-work-use', 5000, 2700);
  add('zero-work-use', 0, 0);
  add('full-work-use', 10000, 12700);
  add('negative-work-use', -1, null);
  add('above-full-work-use', 10001, null);
  add('spouse-half-work-use', 5000, 2700, true);
  add('spouse-above-full-work-use', 10001, null, true);
  add('ordinary-clothes-ineligible-not-invalid', null, 0, false, true);
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  const output = run(['call', model, '--entry', 'beregn_personskat', '--input', save('cases.json', envelope)]);
  save('results.json', output);
  assert.deepEqual(output.diagnostics, []);
  assert.equal(output.results.length, cases.length);
  const expenseResult = (r, c) => (c.spouse ? r.ægtefælle.grundlag : r).ligningsfradrag.øvrige_lønmodtagerudgifter;
  console.log(JSON.stringify(output.results.map(({ case_id, result }, i) => ({ case_id,
    valid: result.vurdering.alle_kontroller_gyldige,
    deduction: expenseResult(result, cases[i]).samlet_fradrag_kroner,
    comparison_ore: result.vurdering.slutskat_til_sammenligning_øre,
  }))));
  // Check the core behavioral regression before checking new explanatory metadata.
  for (const [i, { case_id, result: r }] of output.results.entries()) {
    const c = cases[i], valid = c.expected !== null, expenses = expenseResult(r, c);
    assert.equal(case_id, c.case_id);
    assert.equal(r.vurdering.alle_kontroller_gyldige, valid, case_id);
    assert.equal(r.vurdering.status.$variant, valid ? 'BeregnetMedForbehold' : 'UgyldigtBeregningsgrundlag', case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre, valid ? r.slutskat_øre : null, case_id);
    assert.equal(expenses.alle_input_gyldige, valid, case_id);
    assert.equal(expenses.udgiftsresultater[0].input_gyldigt, valid, case_id);
    assert.equal(expenses.samlet_fradrag_kroner, c.expected ?? 0, case_id);
    if (!c.clothes) assert.equal(expenses.udgiftsresultater[0].fakta.udgiftsart.erhvervsmæssig_andel_basispoint, c.share);
  }
  for (const [i, { result: r }] of output.results.entries()) {
    const c = cases[i], path = `${c.spouse ? 'ægtefælle.MedÆgtefælle.fakta.' : ''}${expensePath}`;
    const control = r.vurdering.kontroller.find(check => check.sti === path);
    assert.equal(control?.gyldig, c.expected !== null, c.case_id);
    assert.ok(control.forklaring.includes('10000'), 'explain the work-use range');
    if (c.expected === null) assert.ok(r.vurdering.fejl.some(check => check.sti === path));
  }
  const schema = run(['schema', model, '--entry', 'beregn_personskat', '--format', 'compact-json']);
  for (const prefix of ['', 'ægtefælle.MedÆgtefælle.fakta.']) {
    for (const [suffix, phrases] of [
      [shareField, ['0 til 10000', '50 %', 'ukendt']],
      ['afholdt_eller_beregnet_beløb_kroner', ['rubrik 58', 'før erhvervsandel', 'bundgrænse', 'købsprisen']],
    ]) {
      const path = `${prefix}${expensePath}.udgifter.${suffix}`;
      const fields = schema.field_metadata.filter(field => field.path === path);
      assert.equal(fields.length, 1, path);
      const field = fields[0];
      for (const phrase of phrases) assert.ok(field.help?.includes(phrase), `${path}: ${phrase}`);
      assert.ok(field.question?.length > 0, path);
      assert.equal(field.anchor, 'Ligningslov9Stk1Årsinput');
      const sources = schema.source_groups[field.source_group]?.map(id => schema.source_objects[id]);
      assert.ok(JSON.stringify(sources).includes('arbejdstoej-faglitteratur-og-kurser-med-mere'), `${path}: source trace`);
      assert.equal(field.unit, suffix === shareField ? 'basispoint' : 'kr./år');
    }
  }
});
