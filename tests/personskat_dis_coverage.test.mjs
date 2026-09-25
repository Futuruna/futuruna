// Invented source facts only. A computable component does not establish that
// the canonical annual calculation supports its special tax position.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const v = ($variant, fields = {}) => ({ $variant, ...fields });
const disPath = 'lønmodtager.personlig_indkomst.sømandsbeskatning';
function run(args) {
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 600000,
    maxBuffer: 32 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
}
function seafarer(year, age) {
  return {
    indkomster: [{ identifikation: 'fiktiv-kulbrinteindkomst', indkomstår: year,
      person: { skattepligt: v('SøblKulbrinteskattepligtigEfterPar21Stk2'),
        statsborgerskab: v('SøblAndetStatsborgerskab'), relation: v('SøblAlmindeligLønmodtager') },
      skib: { identifikation: 'fiktivt-eu-skib',
        registrering: v('SøblUdenlandskSkibRegistreretIEUEØS', { flag: v('SøblEUEØSFlag') }),
        bruttotonnage: 12000, arbejdsgiverstatus: v('SøblUdenlandskArbejdsgiverGodkendtEfterPar11A') },
      arbejde: {
        anvendelse: v('SøblUdelukkendeAnvendtTil', { aktivitet: v('SøblTransportAfGodsMellemForskelligeDestinationer') }),
        arbejdsområde: v('SøblArbejdeIndenForEUEØS'), passagerrute: v('SøblIngenPassagersejlads'),
        arbejdsrolle: v('SøblNormalDriftsbesætning'), par8_valg: v('SøblAnvendPar10RefusionEllerAlmindeligBeskatning') },
      løn: { indkomsttype: v('SøblLønVedArbejdeOmBord'),
        løngrundlag: v('SøblSkattefriNettolønFastsatUnderHensynTilFritagelsen'), beløb_kroner: 100000 } }],
    skibsårsdrifter: [], andre_ligningslov7u_indkomster: [],
    dødsboskattegrundlag: v('Søbl5IntetDødsboskattegrundlag'),
    kulbrinteskattegrundlag: v('Søbl5BKulbrinteskattegrundlag', { kildefakta: {
      personstatus: v('KulbrintePersonIkkeOmfattetAfKildeskattelov1'),
      arbejdsgiverhjemting: v('KulbrinteArbejdsgiverUdenHjemtingIDanmark'),
      indkomstkategori: v('KulbrinteLønEllerAndetIkkeErhvervsmæssigtVederlag'),
      dansk_beskatningsret: v('KulbrinteDanskBeskatningsretBekræftet'),
      beskatningsvalg: v('KulbrinteEndeligBruttoskatEfterPar21Stk2'), alder_ved_indkomstårets_udløb: age,
    } }),
  };
}

test('unsupported special DIS positions explain withheld canonical comparisons', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no build or network.',
}, () => {
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-dis-coverage-'));
  console.log(`Fictional DIS coverage evidence: ${evidence}`);
  const save = (name, value) => {
    const path = join(evidence, name);
    writeFileSync(path, JSON.stringify(value) + '\n', { flag: 'wx', mode: 0o600 }); return path;
  };
  const envelope = run(['template', model, '--format', 'json']);
  const base = envelope.cases[0].input;
  // Explicitly fictional absence of unrelated facts, not real-report defaults.
  Object.assign(base.lønmodtager, { bruttoløn_kroner: 0, kommune: v('København'), betaler_kirkeskat: false });
  Object.assign(base.lønmodtager.pension, { atp: v('IngenAtpIndbetalinger'),
    udbetalingsoplysninger: { for_året_komplette: true, for_foregående_år_komplette: true } });
  Object.assign(base.lønmodtager.ligningsfradrag, {
    boligjob: v('IngenBoligjobudgifter'), enlig_forsørger: v('IntetEkstraBørnetilskud'),
    arbejdsfradrag_udland: v('IngenUdlandsudelukkelseIFællesForhold', {
      dbo_hjemmehørende_udland_i_nogen_periode: true, noget_arbejde_udført_udland: false,
      nogen_udenlandsk_arbejdsgiver: true, kildereference: 'Fiktivt alene arbejde i Danmark',
    }),
  });
  const specs = [
    ['adult-component-ready-but-unsupported', 2026, 1990, 36, false, 8000],
    ['age-conflict-does-not-bypass-coverage', 2026, 1990, 17, false, 0],
    ['spouse-component-ready-but-unsupported', 2026, 1990, 36, false, 8000, true],
    ['no-hydrocarbon-source', 2026, 1990, null, true, 0],
  ];
  envelope.cases = specs.map(([case_id, year, birth, age, , , spouse = false]) => {
    const input = structuredClone(base);
    input.lønmodtager.skatteår = year;
    input.lønmodtager.pension.fødselsdato = { år: birth, måned: 12, dag: 31 };
    input.lønmodtager.personfradrag_alder_status = v(year - birth < 18 ? 'Under18Ugift' : 'Fyldt18EllerGift');
    let person = input;
    if (spouse) {
      const names = ['lønmodtager', 'kapitalindkomst', 'aktieavance', 'udenlandske_sociale_bidrag',
        'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter'];
      person = Object.fromEntries(names.map(name => [name, structuredClone(input[name])]));
      input.ægtefælle = v('MedÆgtefælle', { fakta: person, samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] });
    }
    if (age !== null) person.lønmodtager.personlig_indkomst.sømandsbeskatning = seafarer(year, age);
    return { case_id, input };
  });
  const output = run(['call', model, '--input', save('cases.json', envelope)]);
  save('results.json', output);
  assert.deepEqual(output.diagnostics, []); assert.equal(output.results.length, specs.length);
  const rows = output.results.map(({ case_id, result: r }, i) => {
    const p = specs[i][6] ? r.ægtefælle.grundlag : r;
    const relief = p.personlig_indkomst.sømandsbeskatning.kulbrinte_lempelse;
    return { case_id, valid: r.vurdering.alle_kontroller_gyldige,
      amRelief: relief.arbejdsmarkedsbidragsfritagelse_kroner,
      finalHydrocarbonTax: relief.samlet_skat_efter_søbl5b_kroner };
  });
  console.log(JSON.stringify(rows));
  for (const [i, { case_id, result: r }] of output.results.entries()) {
    const [name, , , , valid, am, spouse = false] = specs[i];
    assert.equal(case_id, name);
    assert.equal(r.vurdering.alle_kontroller_gyldige, valid, name);
    if (!valid) {
      assert.equal(r.vurdering.slutskat_til_sammenligning_øre, null, name);
      const path = `${spouse ? 'ægtefælle.MedÆgtefælle.fakta.' : ''}${disPath}`;
      const control = r.vurdering.fejl.find(c => c.sti === path);
      assert.ok(control, name);
      for (const phrase of ['modelbegrænsning', 'ikke i sig selv en fejl', 'Bevar den faktiske']) {
        assert.ok(control.forklaring.includes(phrase), `${name}: ${phrase}`);
      }
      // Components are diagnostic only, even when internally computable.
      const p = spouse ? r.ægtefælle.grundlag : r;
      assert.equal(p.personlig_indkomst.sømandsbeskatning.kulbrinte_lempelse.beregningsklar, true);
      assert.equal(rows[i].amRelief, am, name);
      assert.equal(rows[i].finalHydrocarbonTax, 0, name);
    } else {
      assert.notEqual(r.vurdering.slutskat_til_sammenligning_øre, null, name);
      assert.equal(rows[i].amRelief, am, name);
      assert.equal(rows[i].finalHydrocarbonTax, 0, name);
    }
  }
  const schema = run(['schema', model, '--format', 'compact-json']);
  for (const prefix of ['', 'ægtefælle.MedÆgtefælle.fakta.']) {
    for (const suffix of ['indkomster.person.skattepligt',
      'kulbrinteskattegrundlag.Søbl5BKulbrinteskattegrundlag.kildefakta.personstatus']) {
      const path = `${prefix}${disPath}.${suffix}`;
      const fields = schema.field_metadata.filter(f => f.path === path);
      assert.equal(fields.length, 1, path);
      const field = fields[0];
      assert.ok(field.help.includes('modelbegrænsning'), path);
      assert.ok(field.help.includes('Bevar den faktiske'), path);
      const sources = schema.source_groups[field.source_group]?.map(id => schema.source_objects[id]);
      assert.ok(JSON.stringify(sources).includes('https://www.retsinformation.dk/eli/lta/2023/1181'), path);
    }
  }
});
