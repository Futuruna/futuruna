// Fictional source facts only. The Futuruna engine performs every tax calculation.
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
function gift(id, agreement = 'fiktiv-aftale', paid = 6000) {
  return {
    identifikation: id, indkomstår: 2025, betalt_beløb_kroner: paid,
    modtager: { identifikation: 'fiktiv-forening', hjemsted: variant('LlGaveDanmark'),
      formål: variant('LlGaveAlmenvelgørendeØkonomiskTrængte'),
      godkendelse: variant('LlGaveGodkendtEfterPar12', { godkendelsesår: 2025 }),
      likvidationsprovenu_til_anden_velgørende_forening: true },
    indberettet_efter_skatteindberetningslov26: true,
    art: variant('Ll12BindendeLøbendeYdelse', {
      aftale_identifikation: agreement, ordinært_betalingsforløb_bekræftet: true,
      ydelsesfastsættelse: variant('Ll12FastÅrligtBeløb'),
      forfalden_årlig_ydelse_efter_aftalen_kroner: 10000,
      aftale_skriftlig: true, ingen_modydelse: true,
      aftalevarighed: variant('Ll12BestemtAftaleperiode', { aftaleperiode_år: 10 }),
      aftalen_kan_ikke_uden_videre_ophæves: true, navn_og_cpr_på_aftalen: true,
    }),
  };
}

test('canonical recurring gifts cap each agreement once and withhold inconsistent source facts', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build or network.',
}, () => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-recurring-gifts-'));
  console.log(`Fictional recurring-gift evidence: ${directory}`);
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
  // Explicit fictional whole-year Danish resident, adult employee in Copenhagen,
  // no church, property, other income, losses, pension/ATP or other deductions.
  // Empty template branches mean absence ONLY for this invented person.
  Object.assign(base.lønmodtager, { skatteår: 2025, bruttoløn_kroner: 300000,
    kommune: variant('København'), betaler_kirkeskat: false });
  Object.assign(base.lønmodtager.pension, { fødselsdato: { år: 1990, måned: 1, dag: 1 },
    atp: variant('IngenAtpIndbetalinger'), udbetalingsoplysninger: { for_året_komplette: true, for_foregående_år_komplette: true } });
  Object.assign(base.lønmodtager.ligningsfradrag, {
    boligjob: variant('IngenBoligjobudgifter'), enlig_forsørger: variant('IntetEkstraBørnetilskud'),
    arbejdsfradrag_udland: variant('IngenUdlandsudelukkelseIFællesForhold', {
      dbo_hjemmehørende_udland_i_nogen_periode: false, noget_arbejde_udført_udland: null,
      nogen_udenlandsk_arbejdsgiver: null, kildereference: 'Fiktiv DBO-hjemmehørende DK hele året',
    }),
  });
  const cases = [];
  const add = (case_id, gifts, expected, spouse = false) => {
    const input = structuredClone(base);
    if (spouse) {
      const fields = ['lønmodtager', 'kapitalindkomst', 'aktieavance', 'udenlandske_sociale_bidrag',
        'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter'];
      const fakta = Object.fromEntries(fields.map(f => [f, structuredClone(input[f])]));
      fakta.lønmodtager.ligningsfradrag.gaver.gaver = gifts;
      input.ægtefælle = variant('MedÆgtefælle', { fakta, samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] });
    } else input.lønmodtager.ligningsfradrag.gaver.gaver = gifts;
    cases.push({ case_id, input, expected, spouse });
  };
  const split = () => [gift('betaling-1'), gift('betaling-2')];
  add('aggregate', [gift('samlet-betaling', 'fiktiv-aftale', 12000)], 10000);
  add('split', split(), 10000);
  add('separate-agreements', [gift('betaling-1'), gift('betaling-2', 'anden-aftale')], 12000);
  const conflicting = split(); conflicting[1].art.forfalden_årlig_ydelse_efter_aftalen_kroner = 11000;
  add('conflicting', conflicting, null);
  add('unknown-agreement', [gift('betaling-1', '')], null);
  const special = split(); special[0].art.ordinært_betalingsforløb_bekræftet = false;
  add('special-or-unknown-history', special, null);
  add('spouse-split', split(), 10000, true);
  add('spouse-conflict', conflicting, null, true);
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  const output = run(['call', model, '--input', save('cases.json', envelope)]);
  save('results.json', output);
  assert.deepEqual(output.diagnostics, []);
  assert.equal(output.results.length, cases.length);
  for (const [i, row] of output.results.entries()) {
    const c = cases[i]; const r = row.result;
    assert.equal(row.case_id, c.case_id);
    const valid = c.expected !== null;
    assert.equal(r.vurdering.alle_kontroller_gyldige, valid, c.case_id);
    assert.equal(r.vurdering.status.$variant, valid ? 'BeregnetMedForbehold' : 'UgyldigtBeregningsgrundlag', c.case_id);
    const gifts = c.spouse ? r.ægtefælle.grundlag.ligningsfradrag.gaver : r.ligningsfradrag.gaver;
    assert.equal(gifts.alle_input_gyldige, valid, c.case_id);
    const path = `${c.spouse ? 'ægtefælle.MedÆgtefælle.fakta.' : ''}lønmodtager.ligningsfradrag.gaver`;
    assert.ok(r.vurdering.kontroller.some(control => control.sti === path && control.gyldig === valid));
    if (valid) {
      assert.equal(r.vurdering.slutskat_til_sammenligning_øre, r.slutskat_øre);
      assert.equal(gifts.samlet_fradrag_kroner, c.expected, c.case_id);
      assert.equal(gifts.par12_aftaler.length, c.case_id === 'separate-agreements' ? 2 : 1);
    } else {
      assert.equal(r.vurdering.slutskat_til_sammenligning_øre, null, c.case_id);
      assert.ok(r.vurdering.fejl.some(error => error.sti === path));
    }
  }
  const r = Object.fromEntries(output.results.map(row => [row.case_id, row.result]));
  assert.equal(r.aggregate.slutskat_øre, r.split.slutskat_øre, 'Payment splitting must not lower tax.');
  assert.ok(r['separate-agreements'].slutskat_øre < r.split.slutskat_øre);
  const agreement = r.split.ligningsfradrag.gaver.par12_aftaler[0];
  assert.equal(agreement.betalt_i_året_kroner, 12000);
  assert.equal(agreement.forfalden_årlig_ydelse_kroner, 10000);
  assert.equal(agreement.fradragsgrundlag_før_årsloft_kroner, 10000);
  assert.deepEqual(agreement.betalingsidentifikationer, ['betaling-1', 'betaling-2']);
  // Real schema projection, including spouse reuse, rather than a text-only
  // assertion about metadata. Compact output avoids the large expanded export.
  const schema = run(['schema', model, '--format', 'compact-json']);
  const metadata = schema.field_metadata ?? schema.contract?.field_metadata;
  assert.ok(Array.isArray(metadata), 'Expected calculation metadata array.');
  for (const prefix of ['', 'ægtefælle.MedÆgtefælle.fakta.']) {
    for (const field of ['aftale_identifikation', 'ordinært_betalingsforløb_bekræftet']) {
      const path = `${prefix}lønmodtager.ligningsfradrag.gaver.gaver.art.Ll12BindendeLøbendeYdelse.${field}`;
      assert.ok(metadata.some(f => f.path === path && typeof f.label === 'string' && f.help), path);
    }
  }
  console.log(JSON.stringify(output.results.map(({ case_id, result }) => ({ case_id,
    comparison_ore: result.vurdering.slutskat_til_sammenligning_øre }))));
});
