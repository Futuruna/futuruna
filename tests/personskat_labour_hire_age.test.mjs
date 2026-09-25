// Synthetic labour-hire facts only; no actual worker identities or tax reports.
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
function hire(year, age, ordinary = false) {
  return {
    identifikation: 'fiktivt-arbejdsudlejeforhold', indkomstår: year, alder_ved_indkomstårets_udløb: age,
    parter: { person_identifikation: 'fiktiv-person', udenlandsk_arbejdsgiver_identifikation: 'fiktiv-DE-udlejer',
      dansk_hvervgiver_identifikation: 'fiktiv-DK-hvervgiver',
      personstatus: v('ArbejdsudlejetIkkeOmfattetAfKsl1EllerKsl2Stk1Nr1'),
      ansættelsesforhold: v('AnsatHosUdenlandskUdlejer'),
      udlejerstatus: v('UdenlandskUdlejerUdenDanskFastDriftssted'),
      hvervgiverstatus: v('DanskHvervgiverMedHjemtingEllerFastDriftssted') },
    arbejde: { arbejdssted: v('ArbejdePåDanskLandEllerSøterritorium'), tilknytning: v('ArbejdeErKerneydelse'),
      udskillelse: v('ArbejdetErIkkePermanentUdskilt') },
    dansk_beskatningsret: v('ArbejdsudlejeDanskBeskatningsretBekræftet'),
    beskatningsvalg: ordinary ? v('ValgtOrdinærBeskatningEfterPar2Stk1Nr1', {
      valgdato: { år: year + 1, måned: 5, dag: 1 }, omgørelsesdato: null,
    }) : v('EndeligArbejdsudlejeskatEfterPar48B'),
    vederlag: v('DokumenteretArbejdsudlejevederlag', { løn_bonus_provision_tillæg_kroner: 100000,
      ferie_og_afspadsering_kroner: 0, rejse_og_befordringsgodtgørelse_kroner: 0,
      fri_kost_og_logi: v('IngenFriKostEllerLogi'), andre_skattepligtige_personalegoder_kroner: 0 }),
    udenlandsk_arbejdsgiverbidrag: { bidrag_kroner: 0, bidrag_vedrører_indkomstår: year,
      obligatorisk: false, omfattet_af_eu_socialsikringsforordning: false,
      aftale_om_at_bidrag_påhviler_lønmodtageren: false },
  };
}

test('labour-hire age must describe the canonical taxpayer, including a spouse', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build or network.',
}, () => {
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-labour-hire-age-'));
  console.log(`Fictional labour-hire evidence: ${evidence}`);
  const save = (name, value) => {
    const path = join(evidence, name);
    writeFileSync(path, JSON.stringify(value) + '\n', { flag: 'wx', mode: 0o600 }); return path;
  };
  const envelope = run(['template', model, '--format', 'json']);
  const base = envelope.cases[0].input;
  base.ægtefælle = { $variant: 'UdenÆgtefælle' }; // Explicit fictional absence.
  // Remaining empty branches describe only this explicitly invented profile.
  Object.assign(base.lønmodtager, { bruttoløn_kroner: 0, kommune: v('København'), kirkeskat: { $variant: 'IngenKirkeskatHeleÅret' } });
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
    ['adult', 2026, 1990, 36, true, 8000, 35600],
    ['adult-with-child-age', 2026, 1990, 17, false, null, null],
    ['young', 2026, 2009, 17, true, 0, 30000],
    ['young-with-adult-age', 2026, 2009, 36, false, null, null],
    ['eighteen-december', 2026, 2008, 18, true, 8000, 35600],
    ['seventeen-before-reform', 2025, 2008, 17, true, 8000, 35600],
    ['spouse-adult', 2026, 1990, 36, true, 8000, 35600, true],
    ['spouse-age-conflict', 2026, 1990, 17, false, null, null, true],
    ['ordinary-adult', 2026, 1990, 36, true, 0, 0, false, true],
    ['ordinary-age-conflict', 2026, 1990, 17, false, null, null, false, true],
  ];
  envelope.cases = specs.map(([case_id, year, birth, age, , , , spouse = false, ordinary = false]) => {
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
    person.lønmodtager.personlig_indkomst.arbejdsudleje = [hire(year, age, ordinary)];
    return { case_id, input };
  });
  const output = run(['call', model, '--input', save('cases.json', envelope)]);
  save('results.json', output);
  assert.deepEqual(output.diagnostics, []); assert.equal(output.results.length, specs.length);
  console.log(JSON.stringify(output.results.map(({ case_id, result: r }, i) => {
    const p = specs[i][7] ? r.ægtefælle.grundlag : r;
    return { case_id, valid: r.vurdering.alle_kontroller_gyldige,
      hire_total: p.personlig_indkomst.samlet_endelig_arbejdsudlejebeskatning_kroner };
  })));
  for (const [i, { case_id, result: r }] of output.results.entries()) {
    const [name, , , , valid, am, total, spouse = false, ordinary = false] = specs[i];
    assert.equal(case_id, name);
    assert.equal(r.vurdering.alle_kontroller_gyldige, valid, name);
    if (!valid) {
      assert.equal(r.vurdering.slutskat_til_sammenligning_øre, null, name);
      const path = `${spouse ? 'ægtefælle.MedÆgtefælle.fakta.' : ''}lønmodtager.personlig_indkomst.arbejdsudleje.alder_ved_indkomstårets_udløb`;
      assert.ok(r.vurdering.fejl.some(c => c.sti === path && c.forklaring.includes('fødselsdato')), name);
    } else {
      const p = spouse ? r.ægtefælle.grundlag : r;
      assert.equal(p.personlig_indkomst.endeligt_arbejdsmarkedsbidrag_fra_arbejdsudleje_kroner, am, name);
      assert.equal(p.personlig_indkomst.samlet_endelig_arbejdsudlejebeskatning_kroner, total, name);
      if (ordinary) assert.equal(r.skat.arbejdsmarkedsbidrag_kroner, 8000, name);
    }
  }
  const schema = run(['schema', model, '--format', 'compact-json']);
  for (const prefix of ['', 'ægtefælle.MedÆgtefælle.fakta.']) {
    for (const [suffix, phrase] of [['', 'denne persons'], ['.alder_ved_indkomstårets_udløb', 'fødselsdato']]) {
      const path = `${prefix}lønmodtager.personlig_indkomst.arbejdsudleje${suffix}`;
      const fields = schema.field_metadata.filter(f => f.path === path);
      assert.equal(fields.length, 1, path);
      const field = fields[0];
      assert.ok(field.help.includes(phrase), path);
      const sources = schema.source_groups[field.source_group]?.map(id => schema.source_objects[id]);
      assert.ok(JSON.stringify(sources).includes('https://www.retsinformation.dk/eli/lta/2025/96'), path);
    }
  }
});
