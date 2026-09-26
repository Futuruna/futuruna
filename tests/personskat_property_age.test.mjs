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

function historicalProperty(input) {
  const property = ordinaryProperty();
  property.ordinært_grundlag.ejendomsværdi_kroner = 2000000;
  property.ordinært_grundlag.grundværdi_kroner = 0;
  property.nedslagsfakta.ejerskabshistorik.oprindelig_erhvervelsesdato = date(2020);
  const income = { personlig_indkomst_kroner: 0, kapitalindkomst_kroner: 0, aktieindkomst_kroner: 0 };
  const history = {
    kontekst_2024: { indkomstår: 2024, kildefakta: structuredClone(input.ejendomsskatter.person),
      egen_indkomst: income, ægtefælles_indkomst: structuredClone(income),
      gift_og_samlevende_ved_indkomstårets_udgang: false },
    ny_lov_helårsgrundlag: structuredClone(property.ordinært_grundlag),
    ny_lov_nedslagsfakta: structuredClone(property.nedslagsfakta),
    tidligere_ejendomsværdiskat: {
      ejendomsværdi_året_før_kroner: 1062500, ejendomsværdi_2001_kroner: 850000,
      ejendomsværdi_2002_kroner: 850000,
      historisk_begrænsning: { foregående_indkomstårs_ejendomsværdiskat_øre: 400000,
        par9b_nedsættelse_øre: 0, vurderet_helt_eller_delvis_benyttet_til_ejerbolig: true,
        ejerlejlighed_frigjort_for_lejemål: false, ombygning_over_100_procent: false },
      udenlandske_ejendomsskatter: [],
    },
    tidligere_grundskyld: { grundværdi_efter_fradrag_og_fritagelser_kroner: 0,
      foregående_års_afgiftspligtige_grundværdi_kroner: 0, grundskyld_promille_2023_tiendedele: 0 },
    byggeri: v('EjskIngenNyEllerOmbygning'), grundskyld_fritaget_basispoint: 0,
    grundskyld_kan_fordeles_på_samme_boligenhed: true,
  };
  property.overgangsvurderinger.rabat = v('EjskRabatvurderingerOplyst', {
    fakta: { eget_rabatgrundlag_2024: history, hændelser: [] },
  });
  input.ejendomsskatter.ejendomme = [property];
  return history;
}
const historicalPath = 'ejendomsskatter.ejendomme.overgangsvurderinger.rabat.EjskRabatvurderingerOplyst.fakta.eget_rabatgrundlag_2024';

test('historical own-age contradiction cannot change a valid annual tax comparison', enabled, () => {
  const dir = mkdtempSync(join(tmpdir(), 'futuruna-property-historical-age-'));
  console.log(`Fictional historical-age evidence: ${dir}`);
  const envelope = run(['template', model, '--format', 'json']);
  const base = buildFictionalCases(envelope).envelope.cases[0].input;
  base.lønmodtager.bruttoløn_kroner = 100000;
  base.lønmodtager.pension.pbl18_indbetalinger = [];
  base.lønmodtager.pension.fødselsdato = date(1990);
  historicalProperty(base);
  const wrong = structuredClone(base);
  wrong.ejendomsskatter.ejendomme[0].overgangsvurderinger.rabat.fakta.eget_rabatgrundlag_2024
    .kontekst_2024.kildefakta.ejer_folkepensionsalder = reached(date(2020));
  envelope.cases = [{ case_id: 'known-history', input: base }, { case_id: 'false-historical-age', input: wrong }];
  const out = run(['call', model, '--input', save(dir, 'input.json', envelope)]);
  save(dir, 'results.json', out); assert.deepEqual(out.diagnostics, []);
  assert.deepEqual(out.results.map(r => r.case_id), ['known-history', 'false-historical-age']);
  console.log(JSON.stringify(out.results.map(({ case_id, result: r }) => ({ case_id,
    valid: r.vurdering.alle_kontroller_gyldige, tax_ore: r.vurdering.slutskat_til_sammenligning_øre,
    property_tax_ore: r.ejendomsskatter.samlet_ejendomsskat_øre, errors: r.vurdering.fejl.map(f => f.sti) }))));
  const [known, bad] = out.results.map(r => r.result);
  assert.equal(known.vurdering.alle_kontroller_gyldige, true);
  assert.equal(known.ejendomsskatter.samlet_ejendomsskat_øre, 640000);
  assert.equal(bad.vurdering.alle_kontroller_gyldige, false);
  assert.equal(bad.vurdering.slutskat_til_sammenligning_øre, null);
  assert.deepEqual(bad.vurdering.fejl.map(f => f.sti), [historicalPath]);
  assert.match(bad.vurdering.fejl[0].forklaring, /fiktiv-bolig/);
  assert.match(bad.vurdering.fejl[0].forklaring, /udgangen af 2024/);
  assert.match(bad.vurdering.fejl[0].forklaring, /EjskFolkepensionsalderIkkeOpnået/);
  assert.equal(bad.ejendomsskatter.samlet_ejendomsskat_øre, 450000, 'diagnostic source computation preserved');
  assert.deepEqual(bad.ejendomsskatter.ejendomsresultater[0].fakta,
    wrong.ejendomsskatter.ejendomme[0], 'do not silently correct the source history');
});

test('historical owner identity uses 2024 and does not replace previous spouse or rebate donor facts', enabled, () => {
  const dir = mkdtempSync(join(tmpdir(), 'futuruna-property-historical-boundaries-'));
  console.log(`Fictional historical boundaries: ${dir}`);
  const envelope = run(['template', model, '--format', 'json']);
  const base = buildFictionalCases(envelope).envelope.cases[0].input;
  base.lønmodtager.bruttoløn_kroner = 100000;
  base.lønmodtager.pension.pbl18_indbetalinger = [];
  base.lønmodtager.pension.fødselsdato = date(1990);
  const specs = [
    ['first-reaches-age-in-2025', [], i => {
      const h = historicalProperty(i);
      i.lønmodtager.pension.fødselsdato = date(1958, 12, 31);
      i.ejendomsskatter.person.ejer_folkepensionsalder = reached(date(2025, 12, 31));
      // Historical status remains NotAchieved; today's status is not copied back.
      assert.equal(h.kontekst_2024.kildefakta.ejer_folkepensionsalder.$variant, 'EjskFolkepensionsalderIkkeOpnået');
    }],
    ['cannot-copy-2025-age-back-to-2024', [historicalPath], i => {
      const h = historicalProperty(i);
      i.lønmodtager.pension.fødselsdato = date(1958, 12, 31);
      i.ejendomsskatter.person.ejer_folkepensionsalder = reached(date(2025, 12, 31));
      h.kontekst_2024.kildefakta.ejer_folkepensionsalder = reached(date(2025, 12, 31));
    }],
    ['spouse-own-history-contradiction', [spousePrefix + historicalPath], i => {
      const s = addSpouse(i), h = historicalProperty(s);
      h.kontekst_2024.kildefakta.ejer_folkepensionsalder = reached(date(2020));
    }],
    ['historical-partner-is-not-current-partner', [], i => {
      addSpouse(i);
      const h = historicalProperty(i);
      h.kontekst_2024.gift_og_samlevende_ved_indkomstårets_udgang = true;
      h.kontekst_2024.kildefakta.samlevende_ægtefælles_folkepensionsalder = reached(date(2020));
    }],
    ['transferred-basis-belongs-to-former-spouse', [], i => {
      const h = historicalProperty(i), p = i.ejendomsskatter.ejendomme[0];
      h.kontekst_2024.kildefakta.ejer_folkepensionsalder = reached(date(2020));
      p.nedslagsfakta.ejerskabshistorik = { oprindelig_erhvervelsesdato: date(2025), ejerskifter: [] };
      p.overgangsvurderinger.rabat.fakta = { eget_rabatgrundlag_2024: null, hændelser: [{
        dato: date(2025), art: v('EjskÆgtefælleoverdragelse', {
          grund: v('EjskSkilsmisseoverdragelse', { boet_er_endnu_ikke_delt: true }),
          retning: v('EjskModtagerRabatFraÆgtefælle', { ny_ejerandel_basispoint: 10000,
            overtaget_rabatgrundlag: { overgangsomfang: structuredClone(p.overgangsomfang), rabat_2024: h } }),
        }),
      }] };
    }],
    ['reaches-age-last-day-of-2024', [], i => {
      const h = historicalProperty(i);
      i.lønmodtager.pension.fødselsdato = date(1957, 12, 31);
      i.ejendomsskatter.person.ejer_folkepensionsalder = reached(date(2024, 12, 31));
      h.kontekst_2024.kildefakta.ejer_folkepensionsalder = reached(date(2024, 12, 31));
    }],
    ['rented-today-still-needs-consistent-own-history', [historicalPath], i => {
      const h = historicalProperty(i);
      i.ejendomsskatter.ejendomme[0].ordinært_grundlag.erhvervsmæssigt_udlejet = true;
      h.kontekst_2024.kildefakta.ejer_folkepensionsalder = reached(date(2020));
    }],
    ['second-property-is-identified-in-error', [historicalPath], i => {
      historicalProperty(i);
      const p = structuredClone(i.ejendomsskatter.ejendomme[0]);
      p.ordinært_grundlag.identifikation = 'anden-fiktiv-bolig';
      const h = p.overgangsvurderinger.rabat.fakta.eget_rabatgrundlag_2024;
      h.ny_lov_helårsgrundlag.identifikation = 'anden-fiktiv-bolig';
      h.kontekst_2024.kildefakta.ejer_folkepensionsalder = reached(date(2020));
      i.ejendomsskatter.ejendomme.push(p);
    }],
  ];
  envelope.cases = specs.map(([case_id, , change]) => {
    const input = structuredClone(base); change(input); return { case_id, input };
  });
  const out = run(['call', model, '--input', save(dir, 'input.json', envelope)]);
  save(dir, 'results.json', out); assert.deepEqual(out.diagnostics, []);
  assert.deepEqual(out.results.map(r => r.case_id), specs.map(s => s[0]));
  for (const [n, { case_id, result: r }] of out.results.entries()) {
    const errors = specs[n][1], valid = errors.length === 0;
    assert.equal(r.vurdering.alle_kontroller_gyldige, valid, case_id);
    assert.deepEqual(r.vurdering.fejl.map(f => f.sti), errors, case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre === null, !valid, case_id);
    for (const path of errors) assert.ok(r.vurdering.kontrolgrundlag.beregning
      .some(c => c.sti === path && !c.gyldig), `${case_id}: calculation validity`);
  }
  assert.match(out.results.at(-1).result.vurdering.fejl[0].forklaring, /anden-fiktiv-bolig/);
  const donor = out.results.find(r => r.case_id === 'transferred-basis-belongs-to-former-spouse').result;
  assert.equal(donor.ejendomsskatter.ejendomsresultater[0].overgang.rabat_ejendomsværdiskat_øre, 366000);
});

test('generated age-field guidance carries law and warns against conflating current and historical people', enabled, () => {
  const schema = run(['schema', model, '--format', 'compact-json']);
  for (const prefix of ['', spousePrefix]) {
    const historical = schema.field_metadata.filter(f => f.path === prefix + historicalPath);
    assert.equal(historical.length, 1, prefix + historicalPath);
    for (const phrase of ['din lønmodtager.pension.fødselsdato', 'udgangen af 2024',
      'ikke beregningsåret', 'overtagne grundlag', 'andre personer']) {
      assert.ok(historical[0].help.includes(phrase), phrase);
    }
    const historicSources = JSON.stringify(schema.source_groups[historical[0].source_group]
      .map(id => schema.source_objects[id]));
    assert.ok(historicSources.includes('https://www.retsinformation.dk/eli/lta/2020/1590'));
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
