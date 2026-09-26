// Fictional evidence only. Approval is an external fact, not an AI decision.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { buildFictionalCases } from '../examples/danish-income-tax/bilag-demo.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
const v = ($variant, fields = {}) => ({ $variant, ...fields });

test('bank pension years require known treatment and preserve actual payment facts', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no build or network.',
}, async t => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-pension-bank-year-'));
  console.log(`Fictional bank-year evidence: ${directory}`);
  const save = (name, value) => {
    const path = join(directory, name);
    writeFileSync(path, JSON.stringify(value) + '\n', { flag: 'wx', mode: 0o600 }); return path;
  };
  const run = args => {
    const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 600000,
      maxBuffer: 64 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
    assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
  };
  const envelope = run(['template', model, '--format', 'json']);
  const base = buildFictionalCases(envelope).envelope.cases.find(c => c.case_id === 'privat-rate').input;
  const post = base.lønmodtager.pension.pbl18_indbetalinger[0];
  post.betaling.beløb_kroner = 40000;
  post.betaling.par22e_genindbetaling = null;
  post.betaling.bank_årsplacering = null;
  const cases = [];
  const add = (case_id, edit, expected, spouse = false) => {
    const input = structuredClone(base);
    edit(input.lønmodtager.pension.pbl18_indbetalinger);
    if (spouse) {
      const fields = ['lønmodtager', 'kapitalindkomst', 'aktieavance', 'udenlandske_sociale_bidrag',
        'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter'];
      const fakta = Object.fromEntries(fields.map(f => [f, structuredClone(input[f])]));
      input.lønmodtager = structuredClone(base.lønmodtager);
      input.lønmodtager.pension.pbl18_indbetalinger = [];
      input.ægtefælle = v('MedÆgtefælle', { fakta, samlevende_ved_indkomstårets_udløb: true,
        kildeskat25a_fordelinger: [] });
    }
    cases.push({ case_id, input, expected, spouse });
  };
  const employer = posts => {
    posts[0].indbetalingskilde = v('Pbl18Arbejdsgiverindbetaling');
    posts[0].betaling.beløb_kroner = 50000;
    posts[0].betaling.arbejdsmarkedsbidrag_kroner = 4000;
    delete posts[0].betaling.bank_årsplacering;
  };
  add('private-control', () => {}, [2025, 40000, 0]);
  add('unknown-bank-year', employer, null);
  add('unknown-next-year-bank', posts => {
    employer(posts); posts[0].betaling.betalingsår = 2026;
  }, null);
  add('unknown-spouse-bank', employer, null, true);
  const ordinary = posts => {
    employer(posts); posts[0].betaling.bank_årsplacering = v('Pbl19BankBetalingsår');
  };
  const approved = posts => {
    employer(posts); posts[0].betaling.betalingsår = 2026;
    posts[0].betaling.bank_årsplacering = v('Pbl19BankGodkendtÅr', {
      godkendelse: { identifikation: 'fictional-approval-1', kildereference: 'fictional-decision:1',
        løntilbageholdelse_kildereference: 'fictional-payroll:1', tilbageholdelsesår: 2025,
        godkendt_indkomstår: 2025, tilbageholdt_brutto_kroner: 50000, godkendt_brutto_kroner: 50000 },
      indbetalingsdato: { år: 2026, måned: 1, dag: 12 }, indbetalingen_omfattet_af_godkendelsen: true,
    });
  };
  add('ordinary-current-bank', ordinary, [2025, 0, 46000]);
  add('ordinary-next-year-bank', posts => {
    ordinary(posts); posts[0].betaling.betalingsår = 2026;
  }, [2026, 0, 0]);
  add('approved-bank', approved, [2025, 0, 46000]);
  add('approved-spouse-bank', approved, [2025, 0, 46000], true);
  add('approval-is-not-counted-in-payment-year', approved, [2025, 0, 0]);
  cases.at(-1).input.lønmodtager.skatteår = 2026;
  const split = posts => {
    approved(posts);
    posts[0].betaling.beløb_kroner = 25000; posts[0].betaling.arbejdsmarkedsbidrag_kroner = 2000;
    posts.push(structuredClone(posts[0])); posts[1].identifikation += '-second';
    posts[1].betaling.bank_årsplacering.indbetalingsdato.dag = 13;
  };
  add('split-one-approval', split, [2025, 0, 46000]);
  add('split-approval-overallocated', posts => {
    split(posts); posts[1].betaling.beløb_kroner += 1;
  }, null);
  add('same-approval-conflicting-facts', posts => {
    split(posts); posts[1].betaling.bank_årsplacering.godkendelse.kildereference = 'conflicting-decision';
  }, null);
  add('private-payment-cannot-use-employer-approval', posts => {
    approved(posts); posts[0].indbetalingskilde = v('Pbl18EgenIndbetaling');
    posts[0].betaling.arbejdsmarkedsbidrag_kroner = 0;
  }, null);
  add('shared-cap-uses-approved-year', posts => {
    approved(posts);
    const own = structuredClone(post); own.identifikation = 'private-extra';
    own.betaling.beløb_kroner = 30000; posts.push(own);
  }, [2025, 19500, 46000]);
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
      `year=${annual.postresultater[0].fradragsår}; tax-øre=${r.vurdering.slutskat_til_sammenligning_øre}`);
    await t.test(c.case_id, () => {
      assert.equal(row.case_id, c.case_id);
      const valid = c.expected !== null;
      assert.equal(r.vurdering.alle_kontroller_gyldige, valid);
      assert.equal(annual.input_gyldigt, valid);
      const supplied = (c.spouse ? c.input.ægtefælle.fakta : c.input).lønmodtager.pension.pbl18_indbetalinger;
      const normalized = structuredClone(supplied);
      for (const p of normalized) p.betaling.bank_årsplacering ??= null;
      assert.deepEqual(annual.input.indbetalinger, normalized, 'never rewrite actual payment or approval facts');
      if (valid) {
        assert.equal(typeof r.vurdering.slutskat_til_sammenligning_øre, 'number');
        assert.equal(annual.postresultater[0].fradragsår, c.expected[0]);
        assert.equal(annual.rate_og_ophørende_fradrag_kroner, c.expected[1]);
        assert.equal(pension.arbejdsgiver_rate_resultat.bortseelsesberettiget_efter_am_kroner, c.expected[2]);
      } else {
        assert.equal(r.vurdering.slutskat_til_sammenligning_øre, null);
        const path = `${c.spouse ? 'ægtefælle.MedÆgtefælle.fakta.' : ''}lønmodtager.pension.pbl18_indbetalinger`;
        assert.ok(r.vurdering.fejl.some(f => f.sti === path));
      }
    });
  }
  await t.test('approved contributions have the same tax effects as the ordinary-year control', () => {
    const byId = new Map(output.results.map(r => [r.case_id, r.result]));
    const control = byId.get('ordinary-current-bank');
    for (const id of ['approved-bank', 'split-one-approval']) {
      const corrected = byId.get(id);
      assert.deepEqual(corrected.skat, control.skat, id);
      assert.deepEqual(corrected.ligningsfradrag, control.ligningsfradrag, id);
      assert.equal(corrected.vurdering.slutskat_til_sammenligning_øre, control.vurdering.slutskat_til_sammenligning_øre);
    }
    const capped = byId.get('shared-cap-uses-approved-year').pension.pbl18_årsresultat;
    assert.equal(capped.rate_og_ophørende_fradragsloft_efter_arbejdsgiver_kroner, 19500);
  });
  await t.test('source-backed bank-year interview reaches taxpayer and spouse', () => {
    const schema = run(['schema', model, '--format', 'compact-json']);
    const fields = ['', 'ægtefælle.MedÆgtefælle.fakta.'].map(prefix => {
      const path = `${prefix}lønmodtager.pension.pbl18_indbetalinger.betaling.bank_årsplacering`;
      const found = schema.field_metadata.filter(f => f.path === path);
      assert.equal(found.length, 1, path);
      const f = found[0]; assert.equal(f.anchor, 'Pbl18Årsbetaling');
      assert.ok(f.question.includes('Skattestyrelsen'));
      assert.ok(f.help.includes('null uafklaret'));
      assert.ok(f.help.includes('personskat-bankpension-aar.md'));
      const sources = JSON.stringify(schema.source_groups[f.source_group].map(id => schema.source_objects[id]));
      assert.ok(sources.includes('https://info.skat.dk/data.aspx?oid=2048283'));
      return f;
    });
    assert.equal(fields[0].help, fields[1].help);
    save('guidance.json', fields);
  });
});
