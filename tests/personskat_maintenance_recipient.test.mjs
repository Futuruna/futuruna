// Fictional source facts only; all income/tax arithmetic stays in Futuruna.
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
const childBirthday = { år: 2012, måned: 6, dag: 10 };
function maintenance() {
  return {
    identifikation: 'fiktivt-bidrag-marts', indkomstår: 2025,
    rolle: variant('Bidragsmodtager'),
    retsgrundlag: variant('Separation', { fastsat_eller_ændret_efter_lovens_ikrafttrædelse: true }),
    modtager_identifikation: 'fiktivt-barn',
    modtager: variant('Barn', { fakta: { fødselsdato: { ...childBirthday },
      bopæl: variant('HosDenAndenForælder'), forsørger_eller_bidragspligt_over_for_det_offentlige: true } }),
    fastsættelse: variant('FastsatAfOffentligMyndighed'),
    bidragsart: variant('LøbendeUnderholdsbidrag', { beløbsgrundlag: variant('HeleForfaldsmånedensBidrag') }),
    betalingsvej: variant('BetaltTilDenBidragsberettigede'), restancestatus: variant('OrdinærBetaling'),
    forfaldsdato: { år: 2025, måned: 3, dag: 1 }, betalingsdato: { år: 2025, måned: 3, dag: 1 },
    beløb_kroner: 3000, betalt_af_bidragsyderen_selv: true,
  };
}

test('child-maintenance recipient must match the assessed person, not merely the receiving bank account', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build or network.',
}, () => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-maintenance-recipient-'));
  console.log(`Fictional maintenance evidence: ${directory}`);
  const save = (name, data) => {
    const path = join(directory, name);
    writeFileSync(path, JSON.stringify(data) + '\n', { flag: 'wx', mode: 0o600 }); return path;
  };
  const run = args => {
    const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 600000,
      maxBuffer: 32 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
    assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
  };
  const envelope = run(['template', model, '--format', 'json']);
  const base = envelope.cases[0].input;
  // Invented whole-year Danish resident in Copenhagen; no church, other income,
  // property, losses, pension/ATP or other deductions. Template absence is a
  // fictional fact here, never a default for an actual taxpayer.
  Object.assign(base.lønmodtager, { skatteår: 2025, bruttoløn_kroner: 300000,
    kommune: variant('København'), betaler_kirkeskat: false });
  Object.assign(base.lønmodtager.pension, { fødselsdato: { år: 1980, måned: 1, dag: 1 },
    atp: variant('IngenAtpIndbetalinger'), udbetalingsoplysninger: { for_året_komplette: true, for_foregående_år_komplette: true } });
  Object.assign(base.lønmodtager.ligningsfradrag, { boligjob: variant('IngenBoligjobudgifter'),
    enlig_forsørger: variant('IntetEkstraBørnetilskud'),
    arbejdsfradrag_udland: variant('IngenUdlandsudelukkelseIFællesForhold', {
      dbo_hjemmehørende_udland_i_nogen_periode: false, noget_arbejde_udført_udland: null,
      nogen_udenlandsk_arbejdsgiver: null, kildereference: 'Fiktiv DBO-hjemmehørende DK hele året',
    }),
  });
  const cases = [];
  const add = (case_id, edit, valid, spouse = false) => {
    const input = structuredClone(base); edit(input); cases.push({ case_id, input, valid, spouse });
  };
  add('baseline-parent', () => {}, true);
  add('parent-with-child-receipt', input => {
    input.lønmodtager.personlig_indkomst.underholdsbidrag.bidrag = [maintenance()];
  }, false);
  add('actual-child-recipient', input => {
    input.lønmodtager.personlig_indkomst.underholdsbidrag.bidrag = [maintenance()];
    input.lønmodtager.pension.fødselsdato = { ...childBirthday };
    input.lønmodtager.personfradrag_alder_status = variant('Under18Ugift');
    input.lønmodtager.bruttoløn_kroner = 0;
  }, true);
  add('parent-payer', input => {
    const row = maintenance(); row.rolle = variant('Bidragsyder', { modtageridentitet_oplyst: true });
    input.lønmodtager.personlig_indkomst.underholdsbidrag.bidrag = [row];
  }, true);
  add('adult-alimony-recipient', input => {
    const row = maintenance(); row.modtager_identifikation = 'fiktiv-voksen';
    row.modtager = variant('ÆgtefælleEllerTidligereÆgtefælle', { parterne_bor_sammen: false });
    input.lønmodtager.personlig_indkomst.underholdsbidrag.bidrag = [row];
  }, true);
  add('spouse-with-child-receipt', input => {
    const fields = ['lønmodtager', 'kapitalindkomst', 'aktieavance', 'udenlandske_sociale_bidrag',
      'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter'];
    const fakta = Object.fromEntries(fields.map(f => [f, structuredClone(input[f])]));
    fakta.lønmodtager.personlig_indkomst.underholdsbidrag.bidrag = [maintenance()];
    input.ægtefælle = variant('MedÆgtefælle', { fakta, samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] });
  }, false, true);
  add('different-birthday-same-year', input => {
    input.lønmodtager.personlig_indkomst.underholdsbidrag.bidrag = [maintenance()];
    input.lønmodtager.pension.fødselsdato = { ...childBirthday, dag: 11 };
    input.lønmodtager.personfradrag_alder_status = variant('Under18Ugift');
    input.lønmodtager.bruttoløn_kroner = 0;
  }, false);
  add('parent-with-nontaxable-child-receipt', input => {
    const row = maintenance(); row.beløb_kroner = 1603;
    input.lønmodtager.personlig_indkomst.underholdsbidrag.bidrag = [row];
  }, false);
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  const output = run(['call', model, '--input', save('cases.json', envelope)]);
  save('results.json', output);
  assert.deepEqual(output.diagnostics, []);
  assert.equal(output.results.length, cases.length);
  console.log(JSON.stringify(output.results.map(({ case_id, result }) => ({ case_id,
    valid: result.vurdering.alle_kontroller_gyldige,
    maintenance_income: result.personlig_indkomst.underholdsbidrag.samlet_personlig_indkomst_kroner,
    comparison_ore: result.vurdering.slutskat_til_sammenligning_øre }))));
  for (const [i, row] of output.results.entries()) {
    const c = cases[i], r = row.result;
    assert.equal(row.case_id, c.case_id);
    assert.equal(r.vurdering.alle_kontroller_gyldige, c.valid, c.case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre, c.valid ? r.slutskat_øre : null, c.case_id);
    const path = `${c.spouse ? 'ægtefælle.MedÆgtefælle.fakta.' : ''}lønmodtager.personlig_indkomst.underholdsbidrag`;
    assert.ok(r.vurdering.kontroller.some(control => control.sti === path && control.gyldig === c.valid), c.case_id);
    if (!c.valid) assert.ok(r.vurdering.fejl.some(error => error.sti === path), c.case_id);
  }
  const child = output.results[2].result;
  assert.equal(child.personlig_indkomst.underholdsbidrag.samlet_personlig_indkomst_kroner, 1397);
  assert.equal(child.skat.arbejdsmarkedsbidrag_kroner, 0);
  const result = Object.fromEntries(output.results.map(row => [row.case_id, row.result]));
  assert.equal(result['parent-payer'].personlig_indkomst.underholdsbidrag.samlet_ligningsmæssigt_fradrag_kroner, 2816);
  assert.equal(result['adult-alimony-recipient'].personlig_indkomst.underholdsbidrag.samlet_personlig_indkomst_kroner, 3000);
  assert.equal(result['adult-alimony-recipient'].skat.arbejdsmarkedsbidrag_kroner, result['baseline-parent'].skat.arbejdsmarkedsbidrag_kroner);
  const schema = run(['schema', model, '--format', 'compact-json']);
  for (const prefix of ['', 'ægtefælle.MedÆgtefælle.fakta.']) {
    const path = `${prefix}lønmodtager.personlig_indkomst.underholdsbidrag.bidrag.rolle.$variant`;
    const field = schema.field_metadata.find(f => f.path === path);
    assert.ok(field?.help.includes('skattemæssige modtager'), path);
    assert.ok(field.question.includes('skattemæssige modtager'), path);
  }
});
