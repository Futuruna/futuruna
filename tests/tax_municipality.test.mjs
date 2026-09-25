// Input guidance, not a second tax implementation. No network or private data.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const source = readFileSync(join(root, model), 'utf8');

function field(name) {
  const match = source.match(new RegExp(`^= ${name} = Beregningsfelt\\(\\n([\\s\\S]*?)^\\)`, 'm'));
  assert.ok(match, `missing metadata: ${name}`);
  return match[1];
}

test('taxpayer and spouse are asked for tax-year municipality, not current residence', () => {
  for (const name of ['personskat_felt_kommune', 'personskat_felt_ægtefælle_kommune']) {
    const text = field(name);
    assert.match(text, /label = "(?:Ægtefællens s|S)kattekommune/);
    assert.match(text, /question = Some\(".*skattekommune.*indkomståret/);
    assert.match(text, /5\. september året før indkomståret/);
    assert.match(text, /nuværende bopæl/);
    assert.match(text, /tilflytning fra udlandet/);
    assert.match(text, /§ 2/);
    assert.ok(source.includes(`Field(value = ${name})`), `${name}: metadata must be attached`);
  }
});

test('report review preserves its observed municipality without claiming independent verification', () => {
  const report = readFileSync(join(root, 'examples/danish-income-tax/aarsopgoerelse-afstemning.calculate.runa'), 'utf8');
  const metadata = report.split('\n').find(line => line.includes('pathof(ÅrsopgørelseAfstemningsinput::kommune)'));
  assert.match(metadata, /Rapportens skattekommune/);
  assert.match(metadata, /ikke.*nuværende bopæl/);
  const guide = readFileSync(join(root, 'examples/danish-income-tax/pension-og-fradrag.md'), 'utf8');
  assert.match(guide, /5\. september 2025/);
  assert.match(guide, /København/);
  assert.match(guide, /Ballerup/);
});

const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
test('exported contract carries both municipality prompts and existing residence rules execute', {
  skip: binary ? false : 'set FUTURUNA_MODEL_TEST_RUNA to a verified compiler; no automatic build',
}, () => {
  const run = args => {
    const result = spawnSync(binary, args, {
      // Cold validation of the large canonical contract can exceed a minute;
      // warm exports reuse its dependency-validated cache. Still no rebuild.
      cwd: root, encoding: 'utf8', timeout: 180000, maxBuffer: 16 * 1024 * 1024,
      env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' },
    });
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stderr);
    return result.stdout;
  };
  const schema = JSON.parse(run(['schema', model, '--format', 'compact-json']));
  assert.equal(schema.schema, 'futuruna.calculate.compact.v1');
  for (const path of ['lønmodtager.kommune', 'ægtefælle.MedÆgtefælle.fakta.lønmodtager.kommune']) {
    const fields = schema.field_metadata.filter(field => field.path === path);
    assert.equal(fields.length, 1, path);
    assert.match(fields[0].label, /kattekommune/);
    assert.match(fields[0].question, /indkomståret/);
    assert.match(fields[0].help, /5\. september året før indkomståret/);
  }
  const output = run(['examples/danish-income-tax/kommuneskattelov-skattekommune.scenario.runa']);
  assert.match(output, /kommunal_flytning_bruger_skattekommunens_årssats/);
});
