// Exercise the generated contract, not a regex pretending to resolve metadata.
// No private documents, tax arithmetic, compiler build or network.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
test('the two salary fields share one source-backed definition of the wage input', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build.',
}, () => {
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-salary-metadata-'));
  console.log(`Salary contract evidence: ${evidence}`);
  const p = spawnSync(binary, ['schema', 'examples/danish-income-tax/personskat.calculate.runa',
    '--entry', 'beregn_personskat', '--format', 'compact-json'], {
    cwd: root, encoding: 'utf8', timeout: 600000, maxBuffer: 16 * 1024 * 1024,
    env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' },
  });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr);
  const schema = JSON.parse(p.stdout);
  const fields = ['', 'ægtefælle.MedÆgtefælle.fakta.'].map(prefix => {
    const path = `${prefix}lønmodtager.bruttoløn_kroner`;
    const matches = schema.field_metadata.filter(field => field.path === path);
    assert.equal(matches.length, 1, path);
    return matches[0];
  });
  writeFileSync(join(evidence, 'salary-fields.json'), JSON.stringify(fields, null, 2) + '\n',
    { flag: 'wx', mode: 0o600 });
  assert.equal(fields[0].question, fields[1].question, 'spouse needs the same wage boundary');
  assert.equal(fields[0].help, fields[1].help, 'do not maintain divergent wage definitions');
  for (const field of fields) {
    for (const phrase of ['ATP', 'arbejdsgiveradministreret pension', 'AM-bidrag']) {
      assert.ok(field.question.includes(phrase), `${field.path}: ${phrase}`);
    }
    for (const phrase of ['egenbetaling', 'Private pensionsindbetalinger', 'ikke trækkes fra igen',
      'personlig indkomst', 'ikke løn', 'pension-og-fradrag.md']) {
      assert.ok(field.help.includes(phrase), `${field.path}: ${phrase}`);
    }
    assert.equal(field.unit, 'kr./år');
    assert.equal(field.anchor, 'PersonskatLønmodtagerInput');
    const sources = schema.source_groups[field.source_group]?.map(id => schema.source_objects[id]);
    assert.ok(sources?.length > 0, `${field.path}: attached sources`);
    for (const url of ['https://skat.dk/borger/am-bidrag', 'https://info.skat.dk/data.aspx?oid=2233519']) {
      assert.ok(JSON.stringify(sources).includes(url), `${field.path}: ${url}`);
    }
  }
});
