// Fictional correction facts, never inferred from an official tax total.
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

test('pension re-payments retain documented timing and withhold missing correction facts', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build or network.',
}, async t => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-pension-redeposit-'));
  console.log(`Fictional pension correction evidence: ${directory}`);
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
  // The example explicitly supplies all other absences and whole-year facts.
  const originalPost = base.lønmodtager.pension.pbl18_indbetalinger[0];
  originalPost.betaling.beløb_kroner = 40000;
  const cases = [];
  const add = (case_id, mutate, expected, spouse = false) => {
    const input = structuredClone(base);
    mutate(input.lønmodtager.pension.pbl18_indbetalinger);
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
  const unknown = posts => {
    posts[0].betaling.betalingsår = 2026;
    posts[0].betaling.hidrører_fra_par22e_tilbagebetaling = true;
  };
  add('ordinary-private-control', () => {}, [2025, 40000, 0]);
  add('missing-private-correction-facts', unknown, null);
  add('missing-employer-correction-facts', posts => {
    unknown(posts);
    posts[0].indbetalingskilde = v('Pbl18Arbejdsgiverindbetaling');
    posts[0].betaling.beløb_kroner = 50000;
    posts[0].betaling.arbejdsmarkedsbidrag_kroner = 4000;
  }, null);
  add('missing-spouse-correction-facts', unknown, null, true);
  const date = (år, måned, dag) => ({ år, måned, dag });
  const documented = (posts, employer = false) => {
    unknown(posts);
    if (employer) {
      posts[0].indbetalingskilde = v('Pbl18Arbejdsgiverindbetaling');
      posts[0].betaling.beløb_kroner = 50000;
      posts[0].betaling.arbejdsmarkedsbidrag_kroner = 4000;
    }
    posts[0].betaling.par22e_genindbetaling = {
      tilbagebetaling: {
        identifikation: 'refund-1', kildereference: 'fictional-institution:1-3',
        oprindelig_indbetaling_identifikation: 'original-1',
        oprindelig_ordning: v('Pbl22ERateopsparing'),
        oprindelig_indbetalingsdato: date(2025, 12, 22),
        oprindeligt_indbetalt_brutto_kroner: employer ? 50000 : 40000,
        tilbagebetalingsdato: date(2026, 1, 5),
        tilbagebetalt_brutto_kroner: employer ? 50000 : 40000,
        tilbagebetalt_til_indbetaleren: true,
      },
      genindbetalingsdato: date(2026, 1, 19),
    };
  };
  add('documented-private-bank', posts => documented(posts), [2025, 40000, 0]);
  add('documented-employer-bank', posts => documented(posts, true), [2025, 0, 46000]);
  add('documented-spouse-bank', posts => documented(posts), [2025, 40000, 0], true);
  add('late-bank-repayment-is-ordinary', posts => {
    documented(posts);
    posts[0].betaling.par22e_genindbetaling.genindbetalingsdato.dag = 20;
  }, [2026, 0, 0]);
  add('late-insurance-repayment-keeps-due-year', posts => {
    documented(posts, true);
    posts[0].ordning = v('Pbl18Rateforsikring');
    posts[0].betaling.par22e_genindbetaling.genindbetalingsdato.dag = 20;
  }, [2025, 0, 46000]);
  add('insurance-due-year-precedes-correction-year', posts => {
    documented(posts);
    posts[0].ordning = v('Pbl18Rateforsikring');
    posts[0].betaling.forfaldsår = 2026;
  }, [2026, 0, 0]);
  const split = posts => {
    documented(posts);
    posts[0].betaling.beløb_kroner = 20000;
    posts.push(structuredClone(posts[0]));
    posts[1].identifikation += '-second';
  };
  add('split-one-refund', split, [2025, 40000, 0]);
  add('split-refund-overallocated', posts => {
    split(posts); posts[1].betaling.beløb_kroner += 1;
  }, null);
  add('same-refund-conflicting-source-facts', posts => {
    split(posts);
    posts[1].betaling.par22e_genindbetaling.tilbagebetaling.tilbagebetalingsdato.dag = 6;
  }, null);
  const twoRefunds = posts => {
    split(posts);
    posts[0].betaling.par22e_genindbetaling.tilbagebetaling.tilbagebetalt_brutto_kroner = 20000;
    posts[1].betaling.par22e_genindbetaling.tilbagebetaling.tilbagebetalt_brutto_kroner = 20000;
    posts[1].betaling.par22e_genindbetaling.tilbagebetaling.identifikation = 'refund-2';
  };
  add('two-partial-refunds', twoRefunds, [2025, 40000, 0]);
  add('two-refunds-exceed-original', posts => {
    twoRefunds(posts);
    for (const post of posts) {
      post.betaling.beløb_kroner = 30000;
      post.betaling.par22e_genindbetaling.tilbagebetaling.tilbagebetalt_brutto_kroner = 30000;
    }
  }, null);
  add('two-refunds-conflicting-original', posts => {
    twoRefunds(posts);
    posts[1].betaling.par22e_genindbetaling.tilbagebetaling.oprindeligt_indbetalt_brutto_kroner += 1;
  }, null);
  add('stale-original-counted-again', posts => {
    documented(posts);
    posts.push(structuredClone(originalPost));
    posts[1].identifikation = 'original-1';
  }, null);
  add('partial-original-remainder', posts => {
    documented(posts);
    posts[0].betaling.par22e_genindbetaling.tilbagebetaling.oprindeligt_indbetalt_brutto_kroner = 60000;
    posts.push(structuredClone(originalPost));
    posts[1].identifikation = 'original-1'; posts[1].betaling.beløb_kroner = 20000;
  }, [2025, 60000, 0]);
  add('refund-after-deadline', posts => {
    documented(posts);
    posts[0].betaling.par22e_genindbetaling.tilbagebetaling.tilbagebetalingsdato.dag = 20;
    posts[0].betaling.par22e_genindbetaling.genindbetalingsdato.dag = 21;
  }, null);
  add('spouse-false-correction-flag-with-facts', posts => {
    documented(posts); posts[0].betaling.hidrører_fra_par22e_tilbagebetaling = false;
  }, null, true);
  add('january-payment-cannot-be-after-april', posts => {
    documented(posts); posts[0].betaling.betalt_senest_bankjusteret_1_april_efter_forfald = false;
  }, null);
  add('ordinary-employer-control', posts => {
    posts[0].indbetalingskilde = v('Pbl18Arbejdsgiverindbetaling');
    posts[0].betaling.beløb_kroner = 50000;
    posts[0].betaling.arbejdsmarkedsbidrag_kroner = 4000;
  }, [2025, 0, 46000]);
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
      if (valid) {
        assert.equal(annual.postresultater[0].fradragsår, c.expected[0]);
        assert.equal(annual.rate_og_ophørende_fradrag_kroner, c.expected[1]);
        assert.equal(pension.arbejdsgiver_rate_resultat.bortseelsesberettiget_efter_am_kroner, c.expected[2]);
        assert.equal(typeof r.vurdering.slutskat_til_sammenligning_øre, 'number');
      } else {
        assert.equal(r.vurdering.slutskat_til_sammenligning_øre, null);
        const path = `${c.spouse ? 'ægtefælle.MedÆgtefælle.fakta.' : ''}lønmodtager.pension.pbl18_indbetalinger`;
        assert.ok(r.vurdering.fejl.some(f => f.sti === path));
      }
    });
  }
  await t.test('documented private correction preserves the ordinary control tax', () => {
    assert.equal(output.results.find(r => r.case_id === 'documented-private-bank').result.vurdering.slutskat_til_sammenligning_øre,
      output.results[0].result.vurdering.slutskat_til_sammenligning_øre);
  });
  await t.test('employer correction preserves ordinary tax, employment and extra-pension deductions', () => {
    const ordinary = output.results.find(r => r.case_id === 'ordinary-employer-control').result;
    const corrected = output.results.find(r => r.case_id === 'documented-employer-bank').result;
    assert.equal(corrected.vurdering.slutskat_til_sammenligning_øre, ordinary.vurdering.slutskat_til_sammenligning_øre);
    assert.deepEqual(corrected.ligningsfradrag, ordinary.ligningsfradrag);
    assert.equal(typeof corrected.skat.ekstra_pensionsfradrag_kroner, 'number');
    assert.deepEqual(corrected.skat, ordinary.skat);
    assert.equal(corrected.pension.arbejdsgiver_rate_resultat.bortseelsesberettiget_efter_am_kroner,
      ordinary.pension.arbejdsgiver_rate_resultat.bortseelsesberettiget_efter_am_kroner);
  });
  await t.test('correction metadata reaches taxpayer and spouse with source traces', () => {
    const schema = run(['schema', model, '--entry', 'beregn_personskat', '--format', 'compact-json']);
    const fields = [];
    for (const prefix of ['', 'ægtefælle.MedÆgtefælle.fakta.']) {
      for (const suffix of ['hidrører_fra_par22e_tilbagebetaling', 'par22e_genindbetaling']) {
        const path = `${prefix}lønmodtager.pension.pbl18_indbetalinger.betaling.${suffix}`;
        const matches = schema.field_metadata.filter(f => f.path === path);
        assert.equal(matches.length, 1, path);
        const f = matches[0]; fields.push(f);
        assert.equal(f.anchor, 'Pbl18Årsbetaling');
        assert.ok(f.question.length > 30);
        assert.ok(f.help.includes('personskat-pensionskorrektion.md'));
        const sources = JSON.stringify(schema.source_groups[f.source_group].map(id => schema.source_objects[id]));
        for (const url of ['https://www.retsinformation.dk/eli/lta/2024/1243',
          'https://info.skat.dk/data.aspx?oid=2048432', 'https://info.skat.dk/data.aspx?oid=2288101']) {
          assert.ok(sources.includes(url), path);
        }
      }
    }
    assert.equal(fields[0].help, fields[2].help);
    assert.equal(fields[1].help, fields[3].help);
    save('guidance.json', fields);
  });
});
