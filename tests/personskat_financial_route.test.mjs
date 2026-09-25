// Fictional income routing: exclusion from one provision is not tax exemption.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { buildFictionalCases } from '../examples/danish-income-tax/bilag-demo.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
const enabled = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no build or private documents.' };
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const path = 'kapitalindkomst.finansielle_poster';
const v = $variant => ({ $variant });
function run(args) {
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 1200000,
    maxBuffer: 64 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
}
function save(dir, name, value) {
  const file = join(dir, name);
  writeFileSync(file, JSON.stringify(value), { flag: 'wx', mode: 0o600 }); return file;
}
function baseline(envelope) {
  const input = buildFictionalCases(envelope).envelope.cases[0].input;
  input.lønmodtager.pension.pbl18_indbetalinger = [];
  return input;
}
function certificate(amount, entity = 'Ssl1Stk1Nr6Investeringsforening') {
  return { identifikation: 'fiktivt-medlemsbevis', indkomstår: 2025, kilde: {
    $variant: 'PsPar4Nr5AMedlemsbevis', opgjort_gevinst_eller_tab_kroner: amount,
    selskabsskattelov1_stk1_nr6_input: { enhed: v(entity), hjemmehørende_i_danmark: true,
      omfattet_af_par3_undtagelse: false, omfattet_af_fondsbeskatningsloven: false },
  } };
}
function intermediary(amount, listed = true, received = true, trading = false) {
  return { identifikation: 'fiktiv-formidler', indkomstår: 2025, kilde: {
    $variant: 'PsPar4Nr5BFormidlerbeløb', modtaget_beløb_kroner: amount,
    formidler: v(listed ? 'Par4Nr5bPengeinstitut' : 'Par4Nr5bIkkeNævntFormidler'),
    formidler_har_modtaget_gebyrer_provisioner_eller_pengeydelser_fra_investeringsinstitut: received,
    aktieavance_aktivklassifikation_input: {
      indkomstår: 2025, aktiv: v('AblInvesteringsselskabPar19TilKlassifikation'),
      par17_modprøve: {
        næringsstatus: v(trading ? 'AblPar17UdøverNæringVedKøbOgSalgAfAktier' : 'AblPar17UdøverIkkeNæringVedKøbOgSalgAfAktier'),
        erhvervelsesstatus: v(trading ? 'AblPar17ErhvervetSomLedINæringsvej' : 'AblPar17IkkeErhvervetSomLedINæringsvej'),
      },
      investeringsklassifikation: { $variant: 'AblPar19BPar19CKlassifikation', input: {
        indkomstår: 2025, meddelelse: v('AblIngenPar19BMeddelelse'),
        oplysninger: v('AblPar19BOplysningerIkkeIndsendt'),
        aktivmasse: { indkomstår: 2025, direkte_aktiver: [{ $variant: 'AblDirekteInvesteringsaktiv',
          art: v('AblAndetVærdipapir'), gennemsnitlig_værdi_kroner: 100000 }], ejerposter: [] },
      } },
    },
  } };
}
function addSpouse(input) {
  const fakta = Object.fromEntries(['lønmodtager', 'kapitalindkomst', 'aktieavance',
    'udenlandske_sociale_bidrag', 'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter']
    .map(key => [key, structuredClone(input[key])]));
  input.ægtefælle = { $variant: 'MedÆgtefælle', fakta,
    samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] };
  return fakta;
}

test('unrouted nonzero financial income cannot disappear from a valid annual comparison', enabled, () => {
  const dir = mkdtempSync(join(tmpdir(), 'futuruna-financial-route-'));
  console.log(`Fictional financial-route evidence: ${dir}`);
  const envelope = run(['template', model, '--format', 'json']), base = baseline(envelope);
  envelope.cases = [['baseline', []], ['investment-certificate', [certificate(15000)]],
    ['unlisted-intermediary', [intermediary(9000, false)]]].map(([case_id, posts]) => {
    const input = structuredClone(base); input.kapitalindkomst.finansielle_poster = posts;
    return { case_id, input };
  });
  const out = run(['call', model, '--input', save(dir, 'input.json', envelope)]);
  save(dir, 'results.json', out); assert.deepEqual(out.diagnostics, []);
  assert.deepEqual(out.results.map(r => r.case_id), envelope.cases.map(r => r.case_id));
  console.log(JSON.stringify(out.results.map(({ case_id, result: r }) => ({ case_id,
    valid: r.vurdering.alle_kontroller_gyldige, tax_øre: r.vurdering.slutskat_til_sammenligning_øre,
    capital_kroner: r.kapitalindkomst.kapitalindkomst_resultat.nettokapitalindkomst_kroner }))));
  assert.equal(out.results[0].result.vurdering.slutskat_til_sammenligning_øre, 21194454);
  for (const { case_id, result: r } of out.results.slice(1)) {
    const post = r.kapitalindkomst.finansielle_postresultater[0];
    assert.deepEqual(post.fakta, envelope.cases.find(c => c.case_id === case_id).input.kapitalindkomst.finansielle_poster[0]);
    assert.equal(post.kapitalpost.beløb_kroner, 0);
    assert.equal(post.lovresultat.resultat.omfattet_af_kapitalindkomst, false);
    assert.equal(r.vurdering.alle_kontroller_gyldige, false, case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre, null, case_id);
    assert.equal(r.kapitalindkomst.finansielle_poster_gyldige, false, case_id);
    assert.ok(r.vurdering.fejl.some(f => f.sti === path), case_id);
  }
});

test('covered gains, losses, reclassification and nondeductible expenses retain distinct outcomes', enabled, () => {
  const dir = mkdtempSync(join(tmpdir(), 'futuruna-financial-boundaries-'));
  console.log(`Fictional financial boundaries: ${dir}`);
  const envelope = run(['template', model, '--format', 'json']), base = baseline(envelope);
  const expense = months => ({ identifikation: 'fiktiv-provision', indkomstår: 2025,
    kilde: { $variant: 'PsPar4Nr7Provision', input: { indkomstår: 2025,
      art: v('Ll8Stk3StiftelsesprovisionEllerEngangspræmie'), udgift_kroner: 3000, løbetid_måneder: months } } });
  const specs = [
    ['covered-gain', certificate(40000, 'Ssl1Stk1Nr6AndenForening'), true, 40000, 0, false],
    ['covered-loss', certificate(-10000, 'Ssl1Stk1Nr6Korporation'), true, -10000, 0, false],
    ['excluded-loss', certificate(-1), false, 0, 0, false],
    ['other-entity', certificate(15000, 'Ssl1Stk1Nr6AndenEnhed'), false, 0, 0, false],
    ['excluded-zero', certificate(0), true, 0, 0, false],
    ['covered-intermediary', intermediary(9000), true, 9000, 0, false],
    ['personal-reclassification', intermediary(7000, true, true, true), true, 0, 7000, false],
    ['unrelated-payment', intermediary(9000, true, false), false, 0, 0, false],
    ['intermediary-zero', intermediary(0, false, false), true, 0, 0, false],
    ['deductible-expense', expense(23), true, -3000, 0, false],
    ['nondeductible-expense', expense(24), true, 0, 0, false],
    ['spouse-excluded', certificate(15000), false, 0, 0, true],
  ];
  envelope.cases = specs.map(([case_id, post, , , , spouse]) => {
    const input = structuredClone(base), person = spouse ? addSpouse(input) : input;
    person.kapitalindkomst.finansielle_poster = [post]; return { case_id, input };
  });
  const out = run(['call', model, '--input', save(dir, 'input.json', envelope)]);
  save(dir, 'results.json', out); assert.deepEqual(out.diagnostics, []);
  assert.deepEqual(out.results.map(r => r.case_id), specs.map(s => s[0]));
  for (const [i, { case_id, result: r }] of out.results.entries()) {
    const [, post, valid, amount, personal, spouse] = specs[i];
    const capital = spouse ? r.ægtefælle.grundlag.kapitalindkomst : r.kapitalindkomst;
    assert.deepEqual(capital.finansielle_postresultater[0].fakta, post, case_id);
    assert.equal(capital.finansielle_poster_gyldige, valid, case_id);
    assert.equal(capital.alle_input_gyldige, valid, case_id);
    assert.equal(capital.kapitalindkomst_resultat.nettokapitalindkomst_kroner, amount, case_id);
    assert.equal(capital.kapitalindkomst_resultat.omklassificeret_personlig_indkomst_kroner, personal, case_id);
    assert.equal(r.vurdering.alle_kontroller_gyldige, valid, case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre, valid ? r.slutskat_øre : null, case_id);
    if (!valid) assert.ok(r.vurdering.fejl.some(f => f.sti === `${spouse ? 'ægtefælle.MedÆgtefælle.fakta.' : ''}${path}`
      && f.forklaring.includes('personskat-finansielle-poster.md')), case_id);
    if (valid && amount === 0 && personal === 0) assert.equal(r.slutskat_øre, 21194454, case_id);
  }
  const schema = run(['schema', model, '--format', 'compact-json']);
  const fields = [];
  for (const prefix of ['', 'ægtefælle.MedÆgtefælle.fakta.']) {
    for (const member of ['$variant', 'PsPar4Nr5AMedlemsbevis.opgjort_gevinst_eller_tab_kroner',
      'PsPar4Nr5AMedlemsbevis.selskabsskattelov1_stk1_nr6_input.enhed',
      'PsPar4Nr5BFormidlerbeløb.modtaget_beløb_kroner', 'PsPar4Nr5BFormidlerbeløb.formidler',
      'PsPar4Nr5BFormidlerbeløb.formidler_har_modtaget_gebyrer_provisioner_eller_pengeydelser_fra_investeringsinstitut']) {
      const fieldPath = `${prefix}${path}.kilde.${member}`;
      const matches = schema.field_metadata.filter(f => f.path === fieldPath);
      assert.equal(matches.length, 1, fieldPath);
      const [field] = matches; fields.push(field);
      assert.equal(field.anchor, 'PersonskatPar4FinansielPostfakta');
      assert.ok(field.help.includes('personskat-finansielle-poster.md'), fieldPath);
      const sources = schema.source_groups[field.source_group].map(id => schema.source_objects[id]);
      assert.ok(sources.some(s => s.role === 'source' && JSON.stringify(s).includes('1284')));
      assert.ok(sources.some(s => s.role === 'warning' && JSON.stringify(s).includes('ikke automatisk skattefrit')));
    }
  }
  save(dir, 'fields.json', fields);
});

test('part-year comparison keeps main and documented annual spouse financial route failures', enabled, () => {
  const file = 'examples/danish-income-tax/personskat-par14.calculate.runa', entry = 'beregn_personskat_delår';
  const dir = mkdtempSync(join(tmpdir(), 'futuruna-financial-partyear-'));
  console.log(`Fictional financial part-year: ${dir}`);
  const envelope = run(['template', file, '--entry', entry, '--format', 'json']);
  const person = baseline({ cases: [{ input: envelope.cases[0].input.personskat }] });
  person.lønmodtager.bruttoløn_kroner = 300000;
  const annual = structuredClone(person); annual.lønmodtager.bruttoløn_kroner = 604972;
  const base = { personskat: person, skattepligtsændring: v('FuldSkattepligtOphører'),
    skattepligtsperiode: { fra_dato: { år: 2025, måned: 1, dag: 1 }, til_dato: { år: 2025, måned: 6, dag: 30 } },
    valg_afgivet_ved_oplysninger: false, omvalg_dato: null,
    kilder: [{ identifikation: 'fiktiv-loen', beregningsfelt: v('Par14Bruttoløn'), delårsbeløb_kroner: 300000,
      omregningsmetode: v('Par14ForholdsmæssigtLøbendeBeløb'), faktisk_helårsbeløb_kroner: null }],
    helårsgrundlag: { $variant: 'DokumenteretHelårsPersonskat', personskat: annual } };
  const ids = ['known', 'period-main-excluded', 'couple-known', 'annual-spouse-excluded'];
  envelope.cases = ids.map(case_id => {
    const input = structuredClone(base);
    if (case_id === 'couple-known' || case_id === 'annual-spouse-excluded') {
      addSpouse(input.personskat); addSpouse(input.helårsgrundlag.personskat);
    }
    if (case_id === 'period-main-excluded') input.personskat.kapitalindkomst.finansielle_poster = [certificate(15000)];
    if (case_id === 'annual-spouse-excluded') {
      input.helårsgrundlag.personskat.ægtefælle.fakta.kapitalindkomst.finansielle_poster = [intermediary(9000, false)];
    }
    return { case_id, input };
  });
  const out = run(['call', file, '--entry', entry, '--input', save(dir, 'input.json', envelope)]);
  save(dir, 'results.json', out); assert.deepEqual(out.diagnostics, []);
  assert.deepEqual(out.results.map(r => r.case_id), ids);
  for (const { case_id, result: r } of out.results) {
    const valid = case_id.endsWith('known');
    assert.equal(r.vurdering.alle_kontroller_gyldige, valid, case_id);
    if (!valid) assert.equal(r.vurdering.slutskat_til_sammenligning_øre, null, case_id);
  }
  assert.equal(out.results[0].result.vurdering.slutskat_til_sammenligning_øre, 10383101);
});
