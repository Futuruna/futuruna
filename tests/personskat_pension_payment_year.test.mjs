// Fictional payment facts; no report totals are used as inputs. These are
// source-model consistency checks, not independent administrative conformance.
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
const v = ($variant, fields = {}) => ({ $variant, ...fields });

function payment(due, paid, timely, plan = 'Pbl18Rateforsikring', employer = false,
  placement = v('Pbl18IkkePar15APlacering')) {
  return {
    identifikation: 'fiktiv-betaling', ordning: v(plan),
    indbetalingskilde: v(employer ? 'Pbl18Arbejdsgiverindbetaling' : 'Pbl18EgenIndbetaling'),
    fradragsretshaver: v('Pbl18OrdningensEjer'),
    betaling: { beløb_kroner: employer ? 50000 : 40000, forfaldsår: due, betalingsår: paid,
      betalt_senest_bankjusteret_1_april_efter_forfald: timely,
      hidrører_fra_par22e_tilbagebetaling: false, par15a_fradragsplacering: placement,
      arbejdsmarkedsbidrag_kroner: employer ? 4000 : 0 },
    fordelingsforløb: v('Pbl18IngenTiårsfordeling'),
    indeksvalg: { fradragsvalgte_kontraktbidrag_kroner: [] },
    indeksordningsgrundlag: v('Pbl18IkkeIndeksordning'),
    forfaldne_ikke_tidligere_fratrukket_kroner: 0,
    særligt_ordningsgrundlag: v('Pbl18IntetSærligtOrdningsgrundlag'),
    begrænsninger: { pbl54_personkreds_opfyldt: true, afgiftspligt_for_hele_ordningen_indtrådt: false,
      udenlandsk_overførsel_med_tidligere_fradrag_uden_skatte_eller_afgiftskonsekvens: false },
  };
}

test('payment years, deadline assertions and special-plan choices agree in canonical intake', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build or network.',
}, async t => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-pension-payment-year-'));
  console.log(`Fictional payment-year evidence: ${directory}`);
  const save = (name, value) => {
    const path = join(directory, name);
    writeFileSync(path, JSON.stringify(value) + '\n', { flag: 'wx', mode: 0o600 }); return path;
  };
  const run = args => {
    const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 600000,
      maxBuffer: 32 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
    assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
  };
  const envelope = run(['template', model, '--format', 'json']);
  const base = envelope.cases[0].input;
  // All other empty branches explicitly describe this fictional adult Danish
  // full-year resident: no other income, deductions, pension/ATP or spouse.
  base.ægtefælle = v('UdenÆgtefælle');
  Object.assign(base.lønmodtager, { skatteår: 2025, bruttoløn_kroner: 600000,
    kommune: v('København'), kirkeskat: v('IngenKirkeskatHeleÅret') });
  Object.assign(base.lønmodtager.pension, { fødselsdato: { år: 1990, måned: 1, dag: 1 },
    atp: v('IngenAtpIndbetalinger'),
    udbetalingsoplysninger: { for_året_komplette: true, for_foregående_år_komplette: true } });
  Object.assign(base.lønmodtager.ligningsfradrag, {
    boligjob: v('IngenBoligjobudgifter'), enlig_forsørger: v('IntetEkstraBørnetilskud'),
    arbejdsfradrag_udland: v('IngenUdlandsudelukkelseIFællesForhold', {
      dbo_hjemmehørende_udland_i_nogen_periode: false, noget_arbejde_udført_udland: null,
      nogen_udenlandsk_arbejdsgiver: null, kildereference: 'Fiktiv DBO-hjemmehørende DK hele året',
    }),
  });
  const cases = [];
  const add = (case_id, post, expected, spouse = false) => {
    const input = structuredClone(base);
    if (spouse) {
      const fields = ['lønmodtager', 'kapitalindkomst', 'aktieavance', 'udenlandske_sociale_bidrag',
        'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter'];
      const fakta = Object.fromEntries(fields.map(f => [f, structuredClone(input[f])]));
      fakta.lønmodtager.pension.pbl18_indbetalinger = [post];
      input.ægtefælle = v('MedÆgtefælle', { fakta, samlevende_ved_indkomstårets_udløb: true,
        kildeskat25a_fordelinger: [] });
    } else input.lønmodtager.pension.pbl18_indbetalinger = [post];
    cases.push({ case_id, input, expected, spouse });
  };
  // [deduction year, own deduction, employer net rate payment in this year].
  add('same-year-insurance', payment(2025, 2025, true), [2025, 40000, 0]);
  add('timely-prior-insurance', payment(2024, 2025, true), [2024, 0, 0]);
  add('late-prior-insurance', payment(2024, 2025, false), [2025, 40000, 0]);
  add('private-bank-payment-year', payment(2024, 2025, true, 'Pbl18Rateopsparing'), [2025, 40000, 0]);
  add('employer-bank-payment-year', payment(2024, 2025, true, 'Pbl18Rateopsparing', true), [2025, 0, 46000]);
  add('employer-insurance-due-year', payment(2024, 2025, true, 'Pbl18Rateforsikring', true), [2024, 0, 0]);
  add('impossible-timely', payment(2023, 2025, true), null);
  add('impossible-late', payment(2025, 2025, false), null);
  const special = payment(2024, 2025, true, 'Pbl18Rateforsikring', false, v('Pbl18Par15AIndbetalingsår'));
  add('ordinary-plan-special-payment-choice', special, null);
  add('ordinary-plan-special-sale-choice', payment(2024, 2025, true, 'Pbl18Rateforsikring', false,
    v('Pbl18Par15AAfståelsesår', { fradragsår: 2024, indbetalt_senest_1_juli_efterfølgende: true })), null);
  add('spouse-impossible-timely', payment(2023, 2025, true), null, true);
  add('spouse-ordinary-plan-special-choice', special, null, true);
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  const output = run(['call', model, '--input', save('cases.json', envelope)]);
  save('results.json', output);
  assert.deepEqual(output.diagnostics, []);
  assert.equal(output.results.length, cases.length);
  for (const [i, row] of output.results.entries()) {
    const c = cases[i], r = row.result;
    const pension = c.spouse ? r.ægtefælle.grundlag.pension : r.pension;
    const annual = pension.pbl18_årsresultat;
    console.log(`${c.case_id}: valid=${r.vurdering.alle_kontroller_gyldige}; ` +
      `deduction=${annual.rate_og_ophørende_fradrag_kroner}; tax-øre=${r.vurdering.slutskat_til_sammenligning_øre}`);
    await t.test(c.case_id, () => {
      assert.equal(row.case_id, c.case_id);
      const valid = c.expected !== null;
      assert.equal(r.vurdering.alle_kontroller_gyldige, valid);
      assert.equal(annual.input_gyldigt, valid);
      assert.deepEqual(annual.postresultater[0].indbetaling, c.spouse
        ? c.input.ægtefælle.fakta.lønmodtager.pension.pbl18_indbetalinger[0]
        : c.input.lønmodtager.pension.pbl18_indbetalinger[0], 'never rewrite contradictory source facts');
      if (valid) {
        assert.equal(r.vurdering.status.$variant, 'BeregnetMedForbehold');
        assert.equal(typeof r.vurdering.slutskat_til_sammenligning_øre, 'number');
        assert.deepEqual(r.vurdering.fejl, []);
        assert.equal(annual.postresultater[0].fradragsår, c.expected[0]);
        assert.equal(annual.rate_og_ophørende_fradrag_kroner, c.expected[1]);
        assert.equal(pension.arbejdsgiver_rate_resultat.bortseelsesberettiget_efter_am_kroner, c.expected[2]);
      } else {
        assert.equal(r.vurdering.status.$variant, 'UgyldigtBeregningsgrundlag');
        assert.equal(r.vurdering.slutskat_til_sammenligning_øre, null);
        const path = `${c.spouse ? 'ægtefælle.MedÆgtefælle.fakta.' : ''}lønmodtager.pension.pbl18_indbetalinger`;
        assert.ok(r.vurdering.fejl.some(f => f.sti === path));
      }
    });
  }
  await t.test('payment guidance and legal sources reach taxpayer and spouse', () => {
    const schema = run(['schema', model, '--format', 'compact-json']);
    const paths = ['forfaldsår', 'betalingsår', 'betalt_senest_bankjusteret_1_april_efter_forfald',
      'par15a_fradragsplacering.$variant'];
    const fields = [];
    for (const suffix of paths) {
      const pair = ['', 'ægtefælle.MedÆgtefælle.fakta.'].map(prefix => {
        const path = `${prefix}lønmodtager.pension.pbl18_indbetalinger.betaling.${suffix}`;
        const found = schema.field_metadata.filter(f => f.path === path);
        assert.equal(found.length, 1, path);
        const field = found[0];
        assert.equal(field.anchor, 'Pbl18Årsbetaling');
        assert.ok(field.help.includes('pension-og-fradrag.md#betalingsår-og-forfaldsår'));
        const sources = schema.source_groups[field.source_group].map(id => schema.source_objects[id]);
        for (const evidence of ['Pensionsbeskatningsloven § 18', 'Pensionsbeskatningsloven § 19',
          'https://info.skat.dk/data.aspx?oid=2048283']) {
          assert.ok(JSON.stringify(sources).includes(evidence), `${path}: ${evidence}`);
        }
        fields.push(field); return field;
      });
      assert.equal(pair[0].help, pair[1].help);
      assert.equal(pair[0].question, pair[1].question);
      if (suffix.startsWith('betalt_')) {
        assert.ok(pair[0].question.includes('hvis 1. april var en banklukkedag'));
        assert.ok(!pair[0].question.includes('første bankdag efter 1. april'));
      }
      if (suffix.startsWith('par15a_')) assert.ok(pair[0].help.includes('Pbl18IkkePar15APlacering'));
    }
    save('guidance.json', fields);
  });
});
