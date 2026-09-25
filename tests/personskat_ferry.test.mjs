// Fictional commuting facts; expected amounts follow SKAT's published examples.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
const variant = ($variant, fields = {}) => ({ $variant, ...fields });
function commute() {
  return {
    identifikation: 'fiktiv-færge', befordringsmål_identifikation: 'fiktivt-arbejde',
    arbejdsdage: 100, daglige_befordringskilometer: 16,
    bopæl_i_yderkommune_eller_lille_ø: false,
    befordringsformål: variant('IndtægtsgivendeArbejdsplads'),
    modtaget_skattefri_befordringsgodtgørelse_for_strækning: false,
    modtaget_uddannelsesbefordringsrabat_eller_godtgørelse_for_strækning: false,
    ligningslov9d: variant('UdenLigningslov9D'), fradrag_udelukket_folketingshverv_m_v: false,
    arbejdsgiverbetalt_befordring: variant('UdenArbejdsgiverbetaltBefordring'),
    broer: { storebælt_bil_motorcykel_passager: 0, storebælt_kollektiv_passager: 0,
      øresund_bil_motorcykel_passager: 0, øresund_kollektiv_passager: 0,
      dokumenteret_og_afholdt_af_skattepligtige: false },
    særlig_transport: { faktisk_dokumenteret_udgift_kroner: 12000,
      geografiske_forhold_tidsforbrug_økonomisk_rimelighed_kræver_transporten: true },
  };
}

test('ferry/flight expense loses only the unused daily threshold, including spouse and low-income paths', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build or network.',
}, () => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-ferry-'));
  console.log(`Fictional commuting evidence: ${directory}`);
  const save = (name, data) => {
    const path = join(directory, name);
    writeFileSync(path, JSON.stringify(data) + '\n', { flag: 'wx', mode: 0o600 }); return path;
  };
  const run = args => {
    const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 600000,
      maxBuffer: 32 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
    assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
  };
  const envelope = run(['template', model, '--format', 'json']);
  const base = envelope.cases[0].input;
  base.ægtefælle = { $variant: 'UdenÆgtefælle' }; // Explicit fictional absence.
  // All absences below are invented source facts, never assumptions for real users.
  Object.assign(base.lønmodtager, { skatteår: 2025, bruttoløn_kroner: 600000,
    kommune: variant('København'), kirkeskat: { $variant: 'IngenKirkeskatHeleÅret' } });
  Object.assign(base.lønmodtager.pension, { fødselsdato: { år: 1980, måned: 1, dag: 1 },
    atp: variant('IngenAtpIndbetalinger'), udbetalingsoplysninger: { for_året_komplette: true, for_foregående_år_komplette: true } });
  Object.assign(base.lønmodtager.ligningsfradrag, { boligjob: variant('IngenBoligjobudgifter'),
    enlig_forsørger: variant('IntetEkstraBørnetilskud'),
    arbejdsfradrag_udland: variant('IngenUdlandsudelukkelseIFællesForhold', {
      dbo_hjemmehørende_udland_i_nogen_periode: false, noget_arbejde_udført_udland: null,
      nogen_udenlandsk_arbejdsgiver: null, kildereference: 'Fiktiv DBO-hjemmehørende DK hele året',
    }),
    befordring: { forhold: [commute()] },
  });
  const cases = [];
  const add = (case_id, edit, expected, supplement = 0, spouse = false) => {
    const input = structuredClone(base); edit(input);
    cases.push({ case_id, input, expected, supplement, spouse });
  };
  add('2025-short-road', () => {}, 10216);
  add('2026-short-road', input => { input.lønmodtager.skatteår = 2026; }, 9464);
  add('2025-long-road', input => {
    input.lønmodtager.ligningsfradrag.befordring.forhold[0].daglige_befordringskilometer = 34;
  }, 14230);
  add('2026-low-income', input => {
    input.lønmodtager.skatteår = 2026; input.lønmodtager.bruttoløn_kroner = 300000;
  // Existing percentage helper uses integer division: 9464 * 6400 / 10000.
  // This guards composition, not independent conformance of annual rounding.
  }, 9464, 6056);
  add('2025-spouse', input => {
    const fields = ['lønmodtager', 'kapitalindkomst', 'aktieavance', 'udenlandske_sociale_bidrag',
      'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter'];
    const fakta = Object.fromEntries(fields.map(f => [f, structuredClone(input[f])]));
    input.lønmodtager.ligningsfradrag.befordring.forhold = [];
    input.ægtefælle = variant('MedÆgtefælle', { fakta, samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] });
  }, 10216, 0, true);
  add('2025-insufficient-ticket', input => {
    input.lønmodtager.ligningsfradrag.befordring.forhold[0].særlig_transport.faktisk_dokumenteret_udgift_kroner = 1000;
  }, 0);
  add('2025-no-travel-days', input => {
    input.lønmodtager.ligningsfradrag.befordring.forhold[0].arbejdsdage = 0;
  }, 0);
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  const output = run(['call', model, '--input', save('cases.json', envelope)]);
  save('results.json', output);
  assert.deepEqual(output.diagnostics, []);
  assert.equal(output.results.length, cases.length);
  console.log(JSON.stringify(output.results.map(({ case_id, result }, i) => {
    const person = cases[i].spouse ? result.ægtefælle.grundlag : result;
    return { case_id, valid: result.vurdering.alle_kontroller_gyldige,
      deduction: person.ligningsfradrag.befordring.samlet_ligningsfradrag_kroner };
  })));
  for (const [i, { case_id, result: r }] of output.results.entries()) {
    const c = cases[i], person = c.spouse ? r.ægtefælle.grundlag : r;
    assert.equal(case_id, c.case_id);
    assert.equal(r.vurdering.alle_kontroller_gyldige, true, case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre, r.slutskat_øre, case_id);
    const b = person.ligningsfradrag.befordring;
    assert.equal(b.samlet_par9c_grundfradrag_kroner, c.expected, case_id);
    assert.equal(b.lavindkomsttillæg_kroner, c.supplement, case_id);
    assert.equal(b.samlet_ligningsfradrag_kroner, c.expected + c.supplement, case_id);
    // Source expense is preserved. Never pre-subtract the threshold in the input.
    const row = b.forholdsresultater[0];
    assert.equal(row.fakta.særlig_transport.faktisk_dokumenteret_udgift_kroner,
      case_id === '2025-insufficient-ticket' ? 1000 : 12000);
    const trace = row.ligningslov9c_resultat;
    const residual = case_id === '2025-long-road' || case_id === '2025-no-travel-days' ? 0
      : case_id.startsWith('2026') ? 253600 : 178400;
    assert.equal(trace.særlig_transport_resterende_bundgrænse_øre, residual, case_id);
    assert.equal(trace.særlig_transportfradrag_øre,
      case_id === '2025-long-road' ? 1200000 : c.expected * 100, case_id);
    assert.ok(trace.særlig_transport_kilder.some(source => source.$variant === 'Ll9cDenJuridiskeVejledning2026_2CA43313'));
  }
  const schema = run(['schema', model, '--format', 'compact-json']);
  for (const prefix of ['', 'ægtefælle.MedÆgtefælle.fakta.']) {
    for (const [suffix, helpText] of [
      ['arbejdsdage', 'ensartede rejsedage'],
      ['daglige_befordringskilometer', 'ikke færgens eller flyets kilometer'],
      ['særlig_transport.faktisk_dokumenteret_udgift_kroner', 'før bundgrænse'],
      ['særlig_transport.geografiske_forhold_tidsforbrug_økonomisk_rimelighed_kræver_transporten', 'LL § 9 C'],
    ]) {
      const path = `${prefix}lønmodtager.ligningsfradrag.befordring.forhold.${suffix}`;
      const field = schema.field_metadata.find(f => f.path === path);
      assert.ok(field?.help.includes(helpText), path);
      assert.ok(field.question.length > 0, path);
      assert.ok(field.source_group !== undefined, `${path}: source trace`);
      const sources = schema.source_groups[field.source_group].map(id => schema.source_objects[id]);
      assert.ok(JSON.stringify(sources).includes('https://info.skat.dk/data.aspx?oid=2061740'),
        `${path}: specific ferry/flight guidance`);
    }
  }
});
