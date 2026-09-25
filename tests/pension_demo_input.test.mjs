// Fast input-construction regression for the actual public demo script.
// No compiler build, tax calculation, network or personal files.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');

test('pension demo supplies explicit fictional facts instead of unknown template defaults', () => {
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-pension-demo-input-'));
  const capture = join(evidence, 'captured.json');
  const binary = join(evidence, 'capture-runa');
  // Only the template structure used by the demo. The source model's guarded
  // sections start unknown. Stop at call: a mock must not certify tax results.
  const template = {
    $futuruna: { schema_hash: 'synthetic-capture-not-a-real-contract' },
    cases: [{ case_id: 'template', input: {
      lønmodtager: {
        pension: {
          pbl18_indbetalinger: [], atp: { $variant: 'AtpUoplyst' },
          udbetalingsoplysninger: { for_året_komplette: false, for_foregående_år_komplette: false },
        },
        ligningsfradrag: {
          arbejdsfradrag_udland: { $variant: 'ArbejdsfradragUdlandUoplyst' },
          boligjob: { $variant: 'BoligjobUoplyst' },
        },
      },
      ægtefælle: { $variant: 'ÆgtefællegrundlagUoplyst' },
    } }],
  };
  writeFileSync(binary, `#!${process.execPath}
const { readFileSync, writeFileSync } = require('node:fs');
const args = process.argv.slice(2);
if (args[0] === 'template') {
  console.log(${JSON.stringify(JSON.stringify(template))});
} else if (args[0] === 'call') {
  writeFileSync(${JSON.stringify(capture)}, readFileSync(args[args.indexOf('--input') + 1]), { flag: 'wx', mode: 0o600 });
  console.error('capture-only: no tax evaluated');
  process.exit(44);
} else { process.exit(45); }
`, { flag: 'wx', mode: 0o700 });
  const process_ = spawnSync(process.execPath, ['examples/danish-income-tax/pension-demo.mjs', binary], {
    cwd: root, encoding: 'utf8',
  });
  assert.ifError(process_.error);
  assert.notEqual(process_.status, 0, 'capture stub must not pretend to calculate tax');
  assert.match(process_.stderr, /capture-only: no tax evaluated/);
  const { cases } = JSON.parse(readFileSync(capture, 'utf8'));
  assert.equal(cases.length, 8);
  assert.equal(new Set(cases.map(({ case_id }) => case_id)).size, 8);
  for (const { case_id, input } of cases) {
    const facts = input.lønmodtager.ligningsfradrag.arbejdsfradrag_udland;
    assert.equal(facts.$variant, 'IngenUdlandsudelukkelseIFællesForhold', case_id);
    assert.equal(facts.dbo_hjemmehørende_udland_i_nogen_periode, false, case_id);
    // One explicit fictional condition disproves the exclusion; unrelated
    // conditions remain unknown rather than becoming invented facts.
    assert.equal(facts.noget_arbejde_udført_udland, null);
    assert.equal(facts.nogen_udenlandsk_arbejdsgiver, null);
    assert.match(facts.kildereference, /fiktiv/i);
    assert.equal(input.lønmodtager.pension.atp.$variant, 'IngenAtpIndbetalinger');
    assert.deepEqual(input.lønmodtager.pension.udbetalingsoplysninger,
      { for_året_komplette: true, for_foregående_år_komplette: true });
    assert.equal(input.lønmodtager.ligningsfradrag.boligjob.$variant,
      case_id === 'uoplyste-fradrag' ? 'BoligjobUoplyst' : 'IngenBoligjobudgifter');
  }
});
