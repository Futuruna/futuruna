// Real metadata consumers, not source-text matching. No tax recalculation,
// private documents, compiler build or network access.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
const rates = 'https://skat.dk/borger/fradrag/arbejdsrelaterede-fradrag/beskaeftigelses-og-jobfradrag';
const historicalRates = 'https://svmn.dk/tal-og-metode/satser/tidsserier/centrale-beloebsgraenser-i-skattelovgivningen-2018-2024';
const allocation = 'https://www.retsinformation.dk/eli/ft/201112L00194';
const url = source => source.data?.arguments?.find(argument => argument.field === 'url')?.value?.value;
const roles = sources => new Set(sources.map(source => source.role));
function run(args) {
  const result = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 600000,
    maxBuffer: 24 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(result.error); assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
}
function checkSingleParentSources(sources, context) {
  for (const role of ['source', 'historical_source', 'rate_source', 'preparatory_work', 'assumption', 'warning']) {
    assert.ok(roles(sources).has(role), `${context}: ${role}`);
  }
  for (const value of [rates, historicalRates, allocation]) {
    assert.ok(sources.some(source => url(source) === value), `${context}: ${value}`);
  }
  assert.ok(sources.some(source => source.role === 'assumption'
    && source.data.value.includes('helårsfradrag_kroner') && source.data.value.includes('øre igen')), context);
  assert.ok(sources.some(source => source.role === 'warning'
    && source.data.value.includes('Historisk afrunding er ikke uafhængigt verificeret')), context);
}

test('supplementary rules expose rates and distinguish rounding assumptions from law', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build.',
}, () => {
  for (const kind of ['senior', 'enlig']) {
    const document = run(['meta', '--json', `examples/danish-income-tax/ligningsloven-par9j-${kind}.runa`]);
    assert.deepEqual(document.diagnostics, []);
    const anchor = document.anchors.find(anchor => anchor.label === `ll9j_${kind}`);
    assert.ok(anchor.text_start_line > 0, 'keep the verbatim law anchor');
    const sources = anchor.references.flatMap(reference => reference.attachments);
    if (kind === 'enlig') checkSingleParentSources(sources, kind);
    else {
      assert.deepEqual(roles(sources), new Set(['source', 'rate_source', 'assumption', 'warning']));
      assert.ok(sources.some(source => source.role === 'rate_source' && url(source) === rates));
      assert.ok(sources.some(source => source.role === 'source'
        && url(source) === 'https://www.retsinformation.dk/eli/lta/2024/482'));
      assert.ok(sources.some(source => source.role === 'assumption'
        && source.data.value.includes('afkortes til hele øre') && source.data.value.includes('oprundes')));
      assert.ok(sources.some(source => source.role === 'warning' && source.data.value.includes('uafklaret')));
    }
    const filtered = run(['meta', '--json', '--role', 'rate_source',
      `examples/danish-income-tax/ligningsloven-par9j-${kind}.runa`]);
    assert.deepEqual(filtered.diagnostics, []);
    assert.ok(filtered.references.flatMap(reference => reference.attachments)
      .some(source => source.role === 'rate_source' && url(source) === rates));
  }
});

test('source roles and limitations follow all single-parent fields to taxpayer and spouse', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build.',
}, () => {
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-supplementary-metadata-'));
  console.log(`Supplementary deduction metadata evidence: ${evidence}`);
  const schema = run(['schema', 'examples/danish-income-tax/personskat.calculate.runa',
    '--entry', 'beregn_personskat', '--format', 'compact-json']);
  const checked = [];
  for (const prefix of ['', 'ægtefælle.MedÆgtefælle.fakta.']) {
    const base = `${prefix}lønmodtager.ligningsfradrag.enlig_forsørger`;
    for (const suffix of ['$variant',
      'OplystEkstraBørnetilskud.oplysninger_for_året_komplette',
      'OplystEkstraBørnetilskud.kvartaler',
      ...['indkomstår', 'kvartal', 'berettiget', 'modtaget', 'kildereference']
        .map(field => `OplystEkstraBørnetilskud.kvartaler.${field}`)]) {
      const path = `${base}.${suffix}`;
      const fields = schema.field_metadata.filter(field => field.path === path);
      assert.equal(fields.length, 1, path);
      const field = fields[0];
      assert.ok(field.question?.length > 0, path);
      const sources = schema.source_groups[field.source_group]?.map(id => schema.source_objects[id]);
      assert.ok(sources?.length > 0, path);
      checkSingleParentSources(sources, path);
      checked.push({ field, sources });
    }
  }
  assert.equal(checked.length, 16);
  writeFileSync(join(evidence, 'fields.json'), JSON.stringify({ schema_hash: schema.schema_hash, checked }, null, 2),
    { flag: 'wx', mode: 0o600 });
});
