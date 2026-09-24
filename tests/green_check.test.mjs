// Fictional cases only. No network, private reports or automatic compiler build.
// Legal rate/eligibility boundaries and recorded SKAT 2025 observations are
// distinguished in groen-check.md. This does not duplicate the tax formula.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const model = 'examples/danish-income-tax/groen-check.calculate.runa';
const variant = ($variant, fields = {}) => ({ $variant, ...fields });
const date = (år, måned = 1, dag = 1) => ({ år, måned, dag });
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
function baseline() {
  return {
    skatteår: 2025,
    person: {
      fødselsdato: date(1950), skattepligt_den_1_januar: variant('GrønCheckFuldSkattepligt'),
      forskerordning: false, modtager_førtids_senior_eller_tidlig_pension_ved_årets_udløb: false,
      årsforløb: variant('GrønCheckOrdinærtHelår'), kildereference: 'fiktive-personfakta',
    },
    indkomst: { personlig_indkomst_efter_am_kroner: 277800, pbl16_tillæg_kroner: 0, nettokapitalindkomst_kroner: 0, kildereference: 'fiktiv-indkomst' },
    ægtefælle: variant('GrønCheckUgiftEllerIkkeSamlevendeVedÅrsslut'),
    børneforhold: variant('GrønCheckOplysteBørn', { børn: [], oplysninger_komplette: true, kildereference: 'fiktiv-fuldstændig-liste' }),
    oplyst_samlet_grøn_check_øre: null,
  };
}
const child = (id, benefit = 'GrønCheckHalvBørneydelse') => ({
  lokal_reference: id, fødselsdato: date(2010), ophold_den_1_januar: variant('GrønCheckBarnIDanmark'),
  har_indgået_ægteskab_den_1_januar: false,
  anbragt_døgnforanstaltning_eller_offentligt_forsørget_den_1_januar: false,
  børneydelse_ved_årets_udløb: variant(benefit), kildereference: 'fiktiv-ydelsesoversigt',
});
const young = i => { i.person.fødselsdato = date(1990); };
const income = (i, n) => { i.indkomst.personlig_indkomst_efter_am_kroner = n; };
const spouse = (i, n) => { i.ægtefælle = variant('GrønCheckSamlevendeHelår', { ægtefælles_nettokapitalindkomst_kroner: n, kildereference: 'fiktiv-kapitaloversigt' }); };
const scenarios = [];
function add(id, change, expected, status = 'GrønCheckBeregnet', more = {}) {
  const input = baseline(); change(input);
  scenarios.push({ case_id: id, input, expected, status, more });
}
// Exact independent observations, synthetic anonymous SKAT annual-2025 input.
add('skat-2025-supplement-equality', () => {}, 128500, undefined, { tillæg_øre: 28000 });
add('skat-2025-supplement-above', i => income(i, 277801), 100500, undefined, { tillæg_øre: 0 });
add('skat-2025-first-krone-phaseout', i => income(i, 475301), 100493);
add('skat-2025-second-krone-phaseout', i => income(i, 475302), 100485);
add('skat-2025-child-half', i => { young(i); income(i, 475301); i.børneforhold.børn = [child('a')]; }, 11993);
add('skat-2025-overlapping-phaseouts', i => { income(i, 486968); i.børneforhold.børn = [child('a')]; }, 24983, undefined, { pensionist_efter_aftrapning_øre: 12990, børn_efter_aftrapning_øre: 11993 });
add('skat-2025-capital-removes-supplement', i => { i.indkomst.nettokapitalindkomst_kroner = 52401; }, 100500);

// Statutory/model boundaries, not additional externally observed tax returns.
for (const [year, total] of [[2023, 115500], [2024, 128000], [2025, 128500], [2026, 115500]]) {
  add(`annual-base-${year}`, i => { i.skatteår = year; income(i, 200000); }, total);
}
add('2026-supplement-equality', i => { i.skatteår = 2026; income(i, 291100); }, 115500);
add('2026-supplement-above', i => { i.skatteår = 2026; income(i, 291101); }, 87500);
add('2026-first-krone-phaseout', i => { i.skatteår = 2026; income(i, 498201); }, 87493);
add('2025-last-krone-before-zero', i => income(i, 488699), 8);
add('2025-fully-phased-out', i => income(i, 488700), 0);
add('nonpensioner-no-adult-credit', young, 0);
add('private-pension-income-not-public-status', i => { young(i); income(i, 200000); }, 0);
add('early-public-pension', i => { young(i); i.person.modtager_førtids_senior_eller_tidlig_pension_ved_årets_udløb = true; }, 128500);
add('birthday-67-at-year-end', i => { i.person.fødselsdato = date(1958, 12, 31); }, 128500);
add('birthday-67-next-year', i => { i.person.fødselsdato = date(1959); }, 0);
add('age-established-no-public-pension-answer-needed', i => { i.person.modtager_førtids_senior_eller_tidlig_pension_ved_årets_udløb = null; }, 128500);
add('researcher-excluded', i => { i.person.forskerordning = true; i.børneforhold.børn = [child('a')]; }, 0, undefined, { adgang_til_kompensation: false });
add('no-january-tax-liability', i => { i.person.skattepligt_den_1_januar = variant('GrønCheckIngenSkattepligt'); }, 0);
add('pbl16-addback', i => { i.indkomst.pbl16_tillæg_kroner = 1; }, 100500);
add('single-capital-allowance', i => { income(i, 475300); i.indkomst.nettokapitalindkomst_kroner = 52401; }, 100493);
add('negative-capital-does-not-reduce-basis', i => { income(i, 277801); i.indkomst.nettokapitalindkomst_kroner = -50000; }, 100500);
add('spouses-negative-capital', i => { spouse(i, -40000); i.indkomst.nettokapitalindkomst_kroner = -30000; }, 128500);
add('spouses-capital-below-individual-allowances', i => { spouse(i, 50000); i.indkomst.nettokapitalindkomst_kroner = 50000; }, 128500);
add('half-child', i => { young(i); i.børneforhold.børn = [child('a')]; }, 12000);
add('whole-child', i => { young(i); i.børneforhold.børn = [child('a', 'GrønCheckHelBørneydelse')]; }, 24000);
add('three-half-children-cap', i => { young(i); i.børneforhold.børn = ['a','b','c'].map(id => child(id)); }, 24000);
add('mixed-children-cap', i => { young(i); i.børneforhold.børn = [child('a','GrønCheckHelBørneydelse'),child('b'),child('c')]; }, 36000);
add('three-whole-children-cap', i => { young(i); i.børneforhold.børn = ['a','b','c'].map(id => child(id,'GrønCheckHelBørneydelse')); }, 48000);
for (const [id, edit] of [
  ['turns-18-this-year', b => { b.fødselsdato = date(2007,12,31); }],
  ['born-after-january-first', b => { b.fødselsdato = date(2025,1,2); }],
  ['permanent-foreign-residence', b => { b.ophold_den_1_januar = variant('GrønCheckBarnAndetUdland'); }],
  ['married-child', b => { b.har_indgået_ægteskab_den_1_januar = true; }],
  ['publicly-supported-child', b => { b.anbragt_døgnforanstaltning_eller_offentligt_forsørget_den_1_januar = true; }],
  ['no-benefit-right', b => { b.børneydelse_ved_årets_udløb = variant('GrønCheckIngenBørneydelse'); }],
]) add(id, i => { young(i); const b = child('a'); edit(b); i.børneforhold.børn = [b]; }, 0);
for (const stay of ['GrønCheckBarnKortvarigtUdland', 'GrønCheckBarnUddannelsesophold']) {
  add(stay, i => { young(i); const b = child('a'); b.ophold_den_1_januar = variant(stay); i.børneforhold.børn = [b]; }, 12000);
}
add('born-january-first', i => { young(i); const b = child('a'); b.fødselsdato = date(2025); i.børneforhold.børn = [b]; }, 12000);
add('observation-one-ore-difference', i => { i.oplyst_samlet_grøn_check_øre = 128499; }, 128500, 'GrønCheckAfviger');
add('observation-exact-match', i => { i.oplyst_samlet_grøn_check_øre = 128500; }, 128500);
for (const [id, edit] of [
  ['person', i => { i.person = null; }],
  ['income', i => { i.indkomst = null; }],
  ['income-source', i => { i.indkomst.kildereference = ' '; }],
  ['spouse', i => { i.ægtefælle = variant('GrønCheckÆgtefælleforholdUkendt'); }],
  ['children', i => { i.børneforhold = variant('GrønCheckBørnUoplyst'); }],
  ['children-completeness', i => { i.børneforhold.oplysninger_komplette = false; }],
  ['researcher-status', i => { i.person.forskerordning = null; }],
  ['young-pension-status', i => { young(i); i.person.modtager_førtids_senior_eller_tidlig_pension_ved_årets_udløb = null; }],
  ['child-public-support', i => { const b = child('a'); b.anbragt_døgnforanstaltning_eller_offentligt_forsørget_den_1_januar = null; i.børneforhold.børn = [b]; }],
]) add(`unknown-${id}`, edit, null, 'GrønCheckUfuldstændig');
for (const [id, edit] of [
  ['future-year', i => { i.skatteår = 2027; }],
  ['old-year', i => { i.skatteår = 2022; }],
  ['death-or-partial-year', i => { i.person.årsforløb = variant('GrønCheckAndetÅrsforløb'); }],
  ['cross-border', i => { i.person.skattepligt_den_1_januar = variant('GrønCheckGrænsegænger'); }],
  ['partial-marriage', i => { i.ægtefælle = variant('GrønCheckAndetÆgtefælleforløb'); }],
  ['spouse-capital', i => { spouse(i, 110000); income(i,475300); }],
  ['own-spousal-capital', i => { spouse(i,0); income(i,475300); i.indkomst.nettokapitalindkomst_kroner = 110000; }],
  ['special-child-recipient', i => { i.skatteår=2026; i.børneforhold.børn=[child('a','GrønCheckRestEfterYdelseTilBarnet')]; }],
]) add(id, edit, null, 'GrønCheckUdenForDækning');
for (const [id, edit] of [
  ['invalid-date', i => { i.person.fødselsdato = date(1950,2,30); }],
  ['duplicate-child', i => { i.børneforhold.børn = [child('a'),child('a')]; }],
  ['excessive-integer', i => { income(i,Number.MAX_SAFE_INTEGER); }],
  ['negative-pbl', i => { i.indkomst.pbl16_tillæg_kroner = -1; }],
  ['negative-observation', i => { i.oplyst_samlet_grøn_check_øre = -1; }],
]) add(id, edit, null, 'GrønCheckUgyldigtInput');

test('green-check review stays additive and documents unresolved coverage', () => {
  const source = readFileSync(join(root, model), 'utf8');
  assert.match(source, /@ calculate\("Grøn check/);
  assert.match(source, /null.*ukendt/);
  const guide = readFileSync(join(root, 'examples/danish-income-tax/groen-check.md'), 'utf8');
  assert.match(guide, /td-5f8fb4/);
  assert.match(guide, /td-6d49ab/);
  assert.match(guide, /ikke automatisk/);
});

test('typed green-check review executes legal boundaries and independent 2025 observations', {
  skip: binary ? false : 'set FUTURUNA_MODEL_TEST_RUNA; no compiler build or network',
}, () => {
  const run = args => {
    const p=spawnSync(binary,args,{cwd:root,encoding:'utf8',timeout:60000,maxBuffer:8*1024*1024,
      env:{...process.env,FUTURUNA_CALCULATION_JOBS:'1'}});
    assert.ifError(p.error); assert.equal(p.status,0,p.stderr); return JSON.parse(p.stdout);
  };
  const envelope=run(['template',model,'--format','json']);
  envelope.cases=scenarios.map(({case_id,input})=>({case_id,input}));
  const directory=mkdtempSync(join(tmpdir(),'futuruna-green-check-test-'));
  const path=join(directory,'synthetic.json');
  writeFileSync(path,JSON.stringify(envelope),{mode:0o600,flag:'wx'});
  const output=run(['call',model,'--input',path]);
  assert.deepEqual(output.diagnostics,[]);
  assert.equal(output.results.length,scenarios.length);
  for(const s of scenarios) {
    const r=output.results.find(v=>v.case_id===s.case_id)?.result;
    assert.ok(r,s.case_id); assert.equal(r.status.$variant,s.status,s.case_id);
    assert.equal(r.oplyst_samlet_grøn_check_øre,s.input.oplyst_samlet_grøn_check_øre,s.case_id);
    assert.ok(r.forbehold.length>=3,s.case_id);
    if(s.expected===null) {
      assert.equal(r.beregning,null,s.case_id); assert.equal(r.difference_øre,null,s.case_id);
      assert.ok(r.uafklaret.length>0,s.case_id);
    } else {
      assert.equal(r.beregning.samlet_kredit_øre,s.expected,s.case_id);
      for(const [field,value] of Object.entries(s.more)) assert.equal(r.beregning[field],value,`${s.case_id}: ${field}`);
      assert.deepEqual(r.uafklaret,[],s.case_id);
      assert.equal(r.difference_øre,s.input.oplyst_samlet_grøn_check_øre===null?null:s.input.oplyst_samlet_grøn_check_øre-s.expected,s.case_id);
    }
  }
  console.log(`Validated ${scenarios.length} fictional cases; evidence: ${directory}`);
});
