// Construction tests do not pretend to execute tax. The opt-in integration
// uses the same public example and canonical model as a user would.
import assert from 'node:assert/strict';
import test from 'node:test';
import { buildFictionalCases, runDemo, sources } from '../examples/danish-income-tax/bilag-demo.mjs';

const v = ($variant) => ({ $variant });
function constructionTemplate() {
  return { $futuruna: { schema_hash: 'construction-only-not-a-real-contract' }, cases: [{
    case_id: 'placeholder', input: {
      lønmodtager: {
        skatteår: 0, bruttoløn_kroner: 0, kommune: v('København'), betaler_kirkeskat: false,
        pension: { fødselsdato: { år: 0, måned: 0, dag: 0 }, atp: v('AtpUoplyst'),
          pbl18_indbetalinger: [], udbetalingsoplysninger: { for_året_komplette: false, for_foregående_år_komplette: false } },
        ligningsfradrag: { enlig_forsørger: v('EkstraBørnetilskudUoplyst'), boligjob: v('BoligjobUoplyst'),
          arbejdsfradrag_udland: v('ArbejdsfradragUdlandUoplyst') },
      },
      kapitalindkomst: { renter: { renteindtægter_kroner: 0, renteudgifter_kroner: 0 } },
      ægtefælle: v('UdenÆgtefælle'),
    },
  }] };
}

test('construction keeps source facts, repeated observations and unknowns distinct', () => {
  const template = constructionTemplate(); const original = structuredClone(template);
  const { envelope, ledger, held } = buildFictionalCases(template);
  assert.deepEqual(template, original, 'keep the supplied template unchanged');
  const byId = Object.fromEntries(envelope.cases.map(c => [c.case_id, c.input]));
  assert.deepEqual(Object.keys(byId), ['privat-rate', 'egne-renter', 'arbejdsgiver-atp', 'atp-uoplyst']);
  for (const input of Object.values(byId)) {
    assert.equal(input.lønmodtager.bruttoløn_kroner, 600000, 'neither private pension nor payroll deductions applied twice');
    assert.equal(input.lønmodtager.skatteår, 2025);
    assert.deepEqual(input.lønmodtager.pension.udbetalingsoplysninger,
      { for_året_komplette: true, for_foregående_år_komplette: true });
  }
  const privatePayment = byId['privat-rate'].lønmodtager.pension.pbl18_indbetalinger;
  assert.equal(privatePayment.length, 1);
  assert.equal(privatePayment[0].betaling.beløb_kroner, 40001);
  assert.equal(privatePayment[0].indbetalingskilde.$variant, 'Pbl18EgenIndbetaling');
  const interest = byId['egne-renter'].kapitalindkomst.renter;
  assert.equal(interest.renteudgifter_kroner, 51001, 'not total debt interest, principal or bank + tax-report sum');
  assert.equal(interest.renteindtægter_kroner, 1000, 'no premature netting');
  assert.deepEqual(byId['egne-renter'].lønmodtager.pension.pbl18_indbetalinger, []);
  for (const id of ['arbejdsgiver-atp', 'atp-uoplyst']) {
    const pension = byId[id].lønmodtager.pension.pbl18_indbetalinger;
    assert.equal(pension.length, 1, 'employee and employer shares are one pension payment');
    assert.equal(pension[0].indbetalingskilde.$variant, 'Pbl18Arbejdsgiverindbetaling');
    assert.equal(pension[0].betaling.beløb_kroner, 50000);
    assert.equal(pension[0].betaling.arbejdsmarkedsbidrag_kroner, 4000);
  }
  const atp = byId['arbejdsgiver-atp'].lønmodtager.pension.atp;
  assert.equal(atp.poster.length, 1);
  assert.equal(atp.poster[0].indberettet_før_am_kroner, 3000, 'total, not employee payroll share');
  assert.equal(atp.poster[0].indberettet_efter_am_kroner, 2760);
  assert.deepEqual(byId['atp-uoplyst'].lønmodtager.pension.atp, v('AtpUoplyst'));
  assert.equal(held.length, 1);
  assert.equal(held[0].status, 'IkkeIndsendt');
  assert.equal(held[0].case_id, 'renteandel-uoplyst');
  assert.ok(!Object.hasOwn(byId, held[0].case_id), 'unknown interest must not become a zero/guessed input');
  assert.deepEqual(held[0].unavailable_documents, ['A', 'R']);
  for (const row of ledger) {
    assert.equal(row.baseline_confirmation, 'F:4', 'other defaults describe only the explicit fictional baseline');
    for (const mapping of row.mappings) {
      assert.deepEqual(mapping.path.split('.').reduce((o, k) => o[k], byId[row.case_id]), mapping.value);
      assert.ok(mapping.explanation.length > 0 && mapping.sources.length > 0);
      for (const ref of mapping.sources) {
        const [id, line] = ref.split(':');
        assert.ok(sources.dokumenter[id]?.linjer[line], ref);
        if (row.case_id === 'atp-uoplyst') assert.notEqual(id, 'T', 'no facts leaked from unavailable ATP document');
      }
    }
  }
  assert.deepEqual(ledger.find(r => r.case_id === 'egne-renter').excluded_sources.map(r => r.source), ['B:2', 'R:1']);
});

test('mapping rejects conflicting, missing and non-whole-krone source facts without fitting inputs', () => {
  const mutations = [
    ['A', '1', null, /andel skal være afklaret/],
    ['A', '1', 33, /hele kroner/],
    ['A', '1', 101, /andel skal være afklaret/],
    ['R', '1', 51002, /Kilderne stemmer ikke/],
    ['B', '1', -1, /ikke-negative kroner/],
    ['E', '4', 600001, /Lønoplysningerne er modstridende/],
    ['EP', '1', 50001, /Pensionens opdeling er modstridende/],
    ['EP', '3', 46001, /Pension før\/efter AM er modstridende/],
    ['F', '4', false, /bekræftelse mangler/],
    ['F', '6', false, /bekræftelse mangler/],
    ['T', '3', false, /bekræftelse mangler/],
  ];
  for (const [document, line, value, message] of mutations) {
    const changed = structuredClone(sources);
    changed.dokumenter[document].linjer[line].værdi = value;
    assert.throws(() => buildFictionalCases(constructionTemplate(), changed), message, `${document}:${line}`);
  }
  const wrongUnits = structuredClone(sources);
  wrongUnits.dokumenter.B.linjer['1'].enhed = 'øre/år';
  assert.throws(() => buildFictionalCases(constructionTemplate(), wrongUnits), /B:1/);
});

test('fictional source packet reaches canonical results and preserves missing-fact boundaries', {
  skip: !process.env.FUTURUNA_MODEL_TEST_RUNA && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build or network.',
}, () => {
  runDemo(process.env.FUTURUNA_MODEL_TEST_RUNA);
});
