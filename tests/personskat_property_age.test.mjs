// Fictional owner/age facts; no private documents, network or compiler build.
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
const enabled = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build.' };
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const v = ($variant, fields = {}) => ({ $variant, ...fields });
const date = (år, måned = 1, dag = 1) => ({ år, måned, dag });
function run(args) {
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 1200000,
    maxBuffer: 32 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
}
function save(dir, name, value) {
  const file = join(dir, name);
  writeFileSync(file, JSON.stringify(value, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
  return file;
}
function ordinaryProperty() {
  return {
    ordinært_grundlag: {
      identifikation: 'fiktiv-bolig', kommune: v('København'), kategori: v('EjskEnBoligenhed'),
      beliggenhed: v('EjskDanmark'), erhvervsmæssigt_udlejet: false,
      særlige_betingelser_for_nr6_til_nr8_opfyldt: true, ejendomsværdi_kroner: 1000000,
      grundværdi_kroner: 800000, produktionsjord: false,
      ejendomsværdiskatteperiode: v('HeleEjendomsskatteåret'),
      grundskyldsperiode: v('HeleEjendomsskatteåret'), ejerandel_basispoint: 10000,
    },
    nedslagsfakta: {
      ejerskabshistorik: { oprindelig_erhvervelsesdato: date(2025), ejerskifter: [] },
      boliganvendelse: v('EjskHelårsbolig'), selvstændige_boligenheder: 1,
      ejendomsform: v('EjskIkkeEjerlejlighed'), fredet_og_omfattet_af_ligningslovens_par15k: false,
      par24_beregningsgrundlag: v('EjskPar24SammeVærdiSomPar13'),
      pensionistsuccession: v('EjskIngenPensionistsuccession'), udenlandske_ejendomsskatter: [],
    },
    overgangsomfang: { vurderingskategori: v('EjskEjerboligEfterEjendomsvurderingslovensPar3Stk1Nr1'),
      ejerkreds: v('EjskKunFysiskeEjere') },
    overgangsvurderinger: { rabat: v('EjskIngenRabatvurderingerOplyst'),
      stigningsbegrænsning: v('EjskIngenStigningsvurderingerOplyst') },
  };
}

test('a young property owner cannot assert retirement-age relief in a valid annual comparison', enabled, () => {
  const dir = mkdtempSync(join(tmpdir(), 'futuruna-property-age-cases-'));
  console.log(`Fictional property-age evidence: ${dir}`);
  const envelope = run(['template', model, '--format', 'json']);
  const base = buildFictionalCases(envelope).envelope.cases[0].input;
  base.lønmodtager.bruttoløn_kroner = 100000;
  base.lønmodtager.pension.pbl18_indbetalinger = [];
  base.lønmodtager.pension.fødselsdato = date(1990);
  base.ejendomsskatter.ejendomme = [ordinaryProperty()];
  const wrong = structuredClone(base);
  wrong.ejendomsskatter.person.ejer_folkepensionsalder = v('EjskFolkepensionsalderOpnået', {
    opnået_dato: date(2020),
  });
  envelope.cases = [{ case_id: 'young-owner', input: base }, { case_id: 'false-retirement', input: wrong }];
  const output = run(['call', model, '--input', save(dir, 'input.json', envelope)]);
  save(dir, 'results.json', output);
  assert.deepEqual(output.diagnostics, []);
  assert.deepEqual(output.results.map(r => r.case_id), ['young-owner', 'false-retirement']);
  const [known, result] = output.results.map(r => r.result);
  const summary = output.results.map(({ case_id, result: r }) => ({ case_id,
    valid: r.vurdering.alle_kontroller_gyldige, tax_ore: r.vurdering.slutskat_til_sammenligning_øre,
    property_tax_ore: r.ejendomsskatter.samlet_ejendomsskat_øre,
    retirement_relief_ore: r.ejendomsskatter.ejendomsresultater[0].nedslag.par25_nedslag_efter_par26_øre,
    errors: r.vurdering.fejl.map(f => f.sti),
  }));
  console.log(JSON.stringify(summary));
  assert.equal(known.vurdering.alle_kontroller_gyldige, true);
  assert.equal(known.vurdering.slutskat_til_sammenligning_øre, 2679954);
  assert.equal(result.vurdering.alle_kontroller_gyldige, false);
  assert.equal(result.vurdering.slutskat_til_sammenligning_øre, null);
  assert.deepEqual(result.vurdering.fejl.map(f => f.sti), ['ejendomsskatter.person.ejer_folkepensionsalder']);
  // Raw diagnostic computation is preserved, never passed off as valid tax.
  assert.equal(result.ejendomsskatter.samlet_ejendomsskat_øre, 326400);
  assert.match(result.vurdering.fejl[0].forklaring, /EjskFolkepensionsalderIkkeOpnået/);
  assert.match(result.vurdering.fejl[0].forklaring, /2060/);
});

function addSpouse(input) {
  const fakta = Object.fromEntries(['lønmodtager', 'kapitalindkomst', 'aktieavance',
    'udenlandske_sociale_bidrag', 'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter']
    .map(key => [key, structuredClone(input[key])]));
  fakta.ejendomsskatter.ejendomme = [];
  input.ægtefælle = v('MedÆgtefælle', { fakta,
    samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] });
  return fakta;
}
const reached = d => v('EjskFolkepensionsalderOpnået', { opnået_dato: d });
const ownerPath = 'ejendomsskatter.person.ejer_folkepensionsalder';
const partnerPath = 'ejendomsskatter.person.samlevende_ægtefælles_folkepensionsalder';
const spousePrefix = 'ægtefælle.MedÆgtefælle.fakta.';

test('year-end, both spouse directions and survivor succession retain distinct factual identities', enabled, () => {
  const dir = mkdtempSync(join(tmpdir(), 'futuruna-property-age-boundaries-'));
  console.log(`Fictional property-age boundaries: ${dir}`);
  const envelope = run(['template', model, '--format', 'json']);
  const base = buildFictionalCases(envelope).envelope.cases[0].input;
  base.lønmodtager.bruttoløn_kroner = 100000;
  base.lønmodtager.pension.pbl18_indbetalinger = [];
  base.lønmodtager.pension.fødselsdato = date(1990);
  base.ejendomsskatter.ejendomme = [ordinaryProperty()];
  const specs = [
    ['reaches-last-day', [], i => {
      i.lønmodtager.pension.fødselsdato = date(1958, 12, 31);
      i.ejendomsskatter.person.ejer_folkepensionsalder = reached(date(2025, 12, 31));
    }],
    ['missing-achieved-status', [ownerPath], i => {
      i.lønmodtager.pension.fødselsdato = date(1958, 12, 31);
    }],
    ['wrong-achievement-date', [ownerPath], i => {
      i.lønmodtager.pension.fødselsdato = date(1958, 12, 31);
      i.ejendomsskatter.person.ejer_folkepensionsalder = reached(date(2025));
    }],
    ['reaches-next-year', [], i => { i.lønmodtager.pension.fødselsdato = date(1959); }],
    ['spouse-false-own-age', [spousePrefix + ownerPath], i => {
      const s = addSpouse(i); i.ejendomsskatter.ejendomme = [];
      s.ejendomsskatter.ejendomme = [ordinaryProperty()];
      s.ejendomsskatter.person.ejer_folkepensionsalder = reached(date(2020));
    }],
    ['owner-false-partner-age', [partnerPath], i => {
      addSpouse(i);
      i.ejendomsskatter.person.samlevende_ægtefælles_folkepensionsalder = reached(date(2020));
    }],
    ['spouse-false-partner-age', [spousePrefix + partnerPath], i => {
      const s = addSpouse(i); i.ejendomsskatter.ejendomme = [];
      s.ejendomsskatter.ejendomme = [ordinaryProperty()];
      s.ejendomsskatter.person.samlevende_ægtefælles_folkepensionsalder = reached(date(2020));
    }],
    ['older-nonowner-spouse', [], i => {
      addSpouse(i).lønmodtager.pension.fødselsdato = date(1958, 12, 31);
      i.ejendomsskatter.person.samlevende_ægtefælles_folkepensionsalder = reached(date(2025, 12, 31));
    }],
    ['older-nonowner-main', [], i => {
      const s = addSpouse(i); i.ejendomsskatter.ejendomme = [];
      i.lønmodtager.pension.fødselsdato = date(1958, 12, 31);
      s.ejendomsskatter.ejendomme = [ordinaryProperty()];
      s.ejendomsskatter.person.samlevende_ægtefælles_folkepensionsalder = reached(date(2025, 12, 31));
    }],
    ['noncohabiting-partner-unused', [], i => {
      addSpouse(i); i.ægtefælle.samlevende_ved_indkomstårets_udløb = false;
      i.ejendomsskatter.person.samlevende_ægtefælles_folkepensionsalder = reached(date(2020));
    }],
    ['no-properties', [], i => {
      i.lønmodtager.pension.fødselsdato = date(1958, 12, 31); i.ejendomsskatter.ejendomme = [];
    }],
    ['ground-tax-only', [], i => {
      i.lønmodtager.pension.fødselsdato = date(1958, 12, 31);
      i.ejendomsskatter.ejendomme[0].ordinært_grundlag.erhvervsmæssigt_udlejet = true;
    }],
    ['survivor-below-retirement-age', [], i => {
      i.ejendomsskatter.ejendomme[0].nedslagsfakta.pensionistsuccession = v('EjskLængstlevendeBevarerRådighed', {
        hændelsesdato: date(2025, 6, 1), ægtefæller_ikke_separerede: true, rådighed_bevaret: true,
        ægtefælles_folkepensionsalder: reached(date(2020)), nyt_ægteskab: null,
      });
    }],
  ];
  envelope.cases = specs.map(([case_id, , change]) => {
    const input = structuredClone(base); change(input); return { case_id, input };
  });
  const out = run(['call', model, '--input', save(dir, 'input.json', envelope)]);
  save(dir, 'results.json', out);
  assert.deepEqual(out.diagnostics, []);
  assert.deepEqual(out.results.map(r => r.case_id), specs.map(s => s[0]));
  for (const [n, { case_id, result: r }] of out.results.entries()) {
    const errors = specs[n][1], valid = errors.length === 0;
    assert.equal(r.vurdering.alle_kontroller_gyldige, valid, case_id);
    assert.deepEqual(r.vurdering.fejl.map(f => f.sti), errors, case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre === null, !valid, case_id);
    for (const path of errors) assert.ok(r.vurdering.kontrolgrundlag.beregning
      .some(c => c.sti === path && !c.gyldig), `${case_id}: computation, not settlement guard`);
  }
  const relief = id => out.results.find(r => r.case_id === id).result.ejendomsskatter
    .ejendomsresultater[0].nedslag.par25_nedslag_efter_par26_øre;
  assert.equal(relief('reaches-last-day'), 600000);
  assert.equal(relief('reaches-next-year'), 0);
  assert.equal(relief('older-nonowner-spouse'), 600000);
  assert.equal(relief('survivor-below-retirement-age'), 600000);
  assert.equal(relief('noncohabiting-partner-unused'), 0);
});

test('generated age-field guidance carries law and warns against conflating current and historical people', enabled, () => {
  const schema = run(['schema', model, '--format', 'compact-json']);
  for (const prefix of ['', spousePrefix]) {
    for (const path of [ownerPath, partnerPath]) {
      const fields = schema.field_metadata.filter(f => f.path === prefix + path + '.$variant');
      assert.equal(fields.length, 1, prefix + path);
      const field = fields[0];
      assert.match(field.help, /fødselsdato/);
      assert.match(field.help, /første pensionsudbetaling/);
      assert.match(field.help, /historiske|Historiske/);
      const sources = schema.source_groups[field.source_group].map(id => schema.source_objects[id]);
      const text = JSON.stringify(sources);
      for (const url of ['https://www.retsinformation.dk/eli/lta/2023/678',
        'https://www.retsinformation.dk/eli/lta/2024/1123', 'https://www.retsinformation.dk/eli/lta/2025/703']) {
        assert.ok(text.includes(url), `${field.path}: ${url}`);
      }
      assert.ok(text.includes('ikke dokumentautentifikation'));
    }
  }
});
