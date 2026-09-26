// Actual metadata indexing; no personal documents, network or compiler build.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
test('interest-expense mappings retain typed source guidance, warnings and code links', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build.',
}, () => {
  const result = spawnSync(binary, ['meta', '--json', '--type', 'PersonskatÅrsopgørelseMappingAdvarsel',
    'examples/danish-income-tax/personskat-aarsopgoerelse-kildemapping.runa'], {
    cwd: root, encoding: 'utf8', timeout: 120000, maxBuffer: 2 * 1024 * 1024,
  });
  assert.ifError(result.error); assert.equal(result.status, 0, result.stderr);
  const meta = JSON.parse(result.stdout);
  assert.equal(meta.schema, 'futuruna.meta.v1');
  assert.deepEqual(meta.diagnostics, []);
  const label = 'personskat_aarsopgoerelse_renteudgifter';
  const anchors = meta.anchors.filter(anchor => anchor.label === label);
  assert.equal(anchors.length, 1);
  assert.equal(anchors[0].references.length, 1);
  const reference = anchors[0].references[0];
  assert.deepEqual(reference.attachments.map(item => item.role), ['source', 'guidance', 'guidance', 'guidance', 'warning']);
  const text = JSON.stringify(reference.attachments);
  for (const phrase of ['2021/1284', 'https://skat.dk/borger/fradrag/fradrag-for-renter',
    'https://info.skat.dk/data.aspx?oid=2047212', 'freibetraege-fuer-zinsaufwendungen',
    'egen fradragsberettiget årsrente', 'Rubrik 44', 'sum og dens underposter',
    'Modtaget negativ lånerente', 'er indtægt, ikke fradragsvisning',
    'ikke automatisk absolut værdi', 'Ukendt fortegn', 'uden afrunding',
    'Originale linjer og fortegn bevares', 'Ingen ægtefælleandel',
    'DirekteMapping beviser ikke', 'ikke krav om et ekstra bilag',
    'fra-bilag-til-input.md#renteudgifter-bevar-kildens-fortegn']) {
    assert.ok(text.includes(phrase), phrase);
  }
  const spans = meta.spans.filter(span => span.label === label);
  assert.equal(spans.length, 1);
  // The index also retains local bindings; they are not additional mapping rules.
  assert.deepEqual(spans[0].symbols.filter(s => s.kind === 'rule').map(s => [s.kind, s.name]), [
    ['rule', 'personskat_aarsopgoerelse_kortlaeg_renteudgifter'],
    ['rule', 'personskat_aarsopgoerelse_kortlaeg_afklarede_renteudgifter'],
  ]);
  for (const item of reference.attachments) {
    assert.ok(reference.typed_values.some(value => value.type === item.type && value.path === item.value_path));
  }

  const direction = meta.anchors.filter(anchor => anchor.label === 'personskat_aarsopgoerelse_renters_art');
  assert.equal(direction.length, 1);
  assert.equal(direction[0].references.length, 1);
  const classified = direction[0].references[0];
  assert.deepEqual(classified.attachments.map(item => item.role), ['source', 'guidance', 'guidance', 'warning']);
  const classifiedText = JSON.stringify(classified.attachments);
  for (const phrase of ['2021/1284', 'https://info.skat.dk/data.aspx?oid=2047212',
    'freibetraege-fuer-zinsaufwendungen', 'Betalt negativ indlånsrente', 'Modtaget negativ lånerente',
    'rettelser og blandede rubriksummer', 'split ikke et nettobeløb ved gæt',
    'Originale linjer, fortegn og øre bevares', 'DirekteMapping beviser ikke',
    'fra-bilag-til-input.md#renteudgifter-bevar-kildens-fortegn']) {
    assert.ok(classifiedText.includes(phrase), phrase);
  }
  for (const item of classified.attachments) {
    assert.ok(classified.typed_values.some(value => value.type === item.type && value.path === item.value_path));
  }
  const directionSpans = meta.spans.filter(span => span.label === 'personskat_aarsopgoerelse_renters_art');
  assert.equal(directionSpans.length, 1);
  assert.deepEqual(directionSpans[0].symbols.filter(s => s.kind === 'rule').map(s => s.name), [
    'personskat_aarsopgoerelse_kortlaeg_renteindtægter',
    'personskat_aarsopgoerelse_kortlaeg_afklarede_renter',
  ]);
});
