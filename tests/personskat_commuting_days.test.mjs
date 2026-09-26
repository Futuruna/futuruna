// Fictional, disjoint groups of travel days; no private documents or network.
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
function run(args) {
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 600000,
    maxBuffer: 32 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
}
function commute(days, index) {
  return {
    identifikation: `fiktiv-daggruppe-${index}`, befordringsmål_identifikation: 'fiktivt-arbejde',
    arbejdsdage: days, daglige_befordringskilometer: 74,
    bopæl_i_yderkommune_eller_lille_ø: false,
    befordringsformål: v('IndtægtsgivendeArbejdsplads'),
    modtaget_skattefri_befordringsgodtgørelse_for_strækning: false,
    modtaget_uddannelsesbefordringsrabat_eller_godtgørelse_for_strækning: false,
    ligningslov9d: v('UdenLigningslov9D'), fradrag_udelukket_folketingshverv_m_v: false,
    arbejdsgiverbetalt_befordring: v('UdenArbejdsgiverbetaltBefordring'),
    broer: { storebælt_bil_motorcykel_passager: 0, storebælt_kollektiv_passager: 0,
      øresund_bil_motorcykel_passager: 0, øresund_kollektiv_passager: 0,
      dokumenteret_og_afholdt_af_skattepligtige: false },
    særlig_transport: { faktisk_dokumenteret_udgift_kroner: 0,
      geografiske_forhold_tidsforbrug_økonomisk_rimelighed_kræver_transporten: false },
  };
}

test('commuting day groups share one annual calendar, independently for each spouse', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build or network.',
}, () => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-commuting-days-'));
  console.log(`Fictional commuting-day evidence: ${directory}`);
  const save = (name, value) => {
    const path = join(directory, name);
    writeFileSync(path, JSON.stringify(value) + '\n', { flag: 'wx', mode: 0o600 }); return path;
  };
  const envelope = run(['template', model, '--format', 'json']);
  const base = envelope.cases[0].input;
  // All empty branches below describe this invented person, not user defaults.
  base.ægtefælle = v('UdenÆgtefælle');
  Object.assign(base.lønmodtager, { skatteår: 2025, bruttoløn_kroner: 600000,
    kommune: v('København'), kirkeskat: v('IngenKirkeskatHeleÅret') });
  Object.assign(base.lønmodtager.pension, { fødselsdato: { år: 1990, måned: 1, dag: 1 },
    atp: v('IngenAtpIndbetalinger'),
    udbetalingsoplysninger: { for_året_komplette: true, for_foregående_år_komplette: true } });
  Object.assign(base.lønmodtager.ligningsfradrag, { boligjob: v('IngenBoligjobudgifter'),
    enlig_forsørger: v('IntetEkstraBørnetilskud'),
    arbejdsfradrag_udland: v('IngenUdlandsudelukkelseIFællesForhold', {
      dbo_hjemmehørende_udland_i_nogen_periode: false, noget_arbejde_udført_udland: null,
      nogen_udenlandsk_arbejdsgiver: null, kildereference: 'Fiktivt kun danske forhold hele året',
    }),
  });
  // Year-end boundaries are mechanical consistency probes, not assertions that
  // a real taxpayer travelled every day. Same-day journeys belong in one group.
  const specs = [
    ['two-full-working-years', 2025, [220, 220], false],
    ['ordinary-split', 2025, [100, 120], true],
    ['ordinary-single', 2025, [220], true],
    ['common-year-limit', 2025, [200, 165], true],
    ['common-year-over', 2025, [200, 166], false],
    ['leap-year-limit', 2024, [200, 166], true],
    ['leap-year-over', 2024, [200, 167], false],
    ['negative-must-not-offset', 2025, [220, 220, -75], false],
    ['invalid-cannot-recover', 2025, [366, -1], false],
    ['zero-day-row', 2025, [0, 220], true],
    ['no-commuting', 2025, [], true],
    ['spouse-overlap', 2025, [220, 220], false, true],
    ['two-person-calendars', 2025, [220], true, true],
  ];
  envelope.cases = specs.map(([case_id, year, days, , spouse]) => {
    const input = structuredClone(base);
    input.lønmodtager.skatteår = year;
    input.lønmodtager.ligningsfradrag.befordring.forhold = days.map(commute);
    if (spouse) {
      const names = ['lønmodtager', 'kapitalindkomst', 'aktieavance', 'udenlandske_sociale_bidrag',
        'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter'];
      const fakta = Object.fromEntries(names.map(name => [name, structuredClone(input[name])]));
      input.lønmodtager.ligningsfradrag.befordring.forhold = [commute(220, 0)];
      input.ægtefælle = v('MedÆgtefælle', { fakta, samlevende_ved_indkomstårets_udløb: true,
        kildeskat25a_fordelinger: [] });
    }
    return { case_id, input };
  });
  const output = run(['call', model, '--input', save('cases.json', envelope)]);
  save('results.json', output);
  assert.deepEqual(output.diagnostics, []); assert.equal(output.results.length, specs.length);
  console.log(JSON.stringify(output.results.map(({ case_id, result: r }, i) => ({
    case_id, valid: r.vurdering.alle_kontroller_gyldige,
    deduction: (specs[i][4] ? r.ægtefælle.grundlag : r).ligningsfradrag.befordring.samlet_ligningsfradrag_kroner,
  }))));
  for (const [i, { case_id, result: r }] of output.results.entries()) {
    const [name, , days, valid, spouse] = specs[i];
    assert.equal(case_id, name);
    assert.equal(r.vurdering.alle_kontroller_gyldige, valid, name);
    const b = (spouse ? r.ægtefælle.grundlag : r).ligningsfradrag.befordring;
    assert.deepEqual(b.input.forhold.map(row => row.arbejdsdage), days, `${name}: source facts preserved`);
    assert.equal(b.alle_input_gyldige, valid, name);
    if (!valid) {
      assert.equal(r.vurdering.slutskat_til_sammenligning_øre, null, name);
      const path = `${spouse ? 'ægtefælle.MedÆgtefælle.fakta.' : ''}lønmodtager.ligningsfradrag.befordring.forhold.arbejdsdage`;
      assert.ok(r.vurdering.fejl.some(c => c.sti === path && c.forklaring.includes('tilsammen')), name);
      assert.equal(b.samlet_ligningsfradrag_kroner, 0, `${name}: no invalid annual deduction`);
    } else {
      assert.equal(r.vurdering.slutskat_til_sammenligning_øre, r.slutskat_øre, name);
    }
  }
  const split = output.results.find(r => r.case_id === 'ordinary-split').result;
  const single = output.results.find(r => r.case_id === 'ordinary-single').result;
  assert.equal(single.ligningsfradrag.befordring.samlet_ligningsfradrag_kroner, 24530);
  assert.equal(split.slutskat_øre, single.slutskat_øre, 'disjoint grouping must preserve the ordinary result');
  const schema = run(['schema', model, '--format', 'compact-json']);
  for (const prefix of ['', 'ægtefælle.MedÆgtefælle.fakta.']) {
    const path = `${prefix}lønmodtager.ligningsfradrag.befordring.forhold.arbejdsdage`;
    const field = schema.field_metadata.find(f => f.path === path);
    assert.ok(field?.help.includes('tilsammen') && field.help.includes('kalenderdage'), path);
    assert.ok(field.help.includes('beviser ikke'), `${path}: honest aggregate-check limit`);
    const sources = schema.source_groups[field.source_group]?.map(id => schema.source_objects[id]);
    assert.ok(sources?.some(s => s.role === 'guidance' && JSON.stringify(s).includes(
      'https://skat.dk/borger/fradrag/koerselsfradrag/koerselsfradrag-befordringsfradrag')), `${path}: primary guidance`);
  }
});
