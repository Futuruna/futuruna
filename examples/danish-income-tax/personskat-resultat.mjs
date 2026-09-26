// Read-only summary of canonical model output, not a second tax calculator.
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { amount, integer, list, readSavedOutput, record, requireThat, textValue, variant, visible } from './resultat-visning.mjs';

// Deliberately version the supported outer shape. A new top-level warning must
// not be silently discarded. A source-shape test guards this list against drift.
export const resultFields = `
arbejdsfradrag_udland enligforsørgerfradrag vurdering skat hovedskat_eksakt
sømandsbeskatning sømandsbeskatning_eksakt indgående_ægtefælle ægtefælle kildeskat25a
underskudsår personskattelov13a aktieindkomst_parår positiv_aktieindkomstskat
negativ_aktieindkomstskat negativ_aktieskat_fremførselsår endelig_aktieindkomstskat_kroner
samlet_skat_inkl_endelig_aktieindkomstskat_kroner samlet_skat_efter_negativ_aktieindkomstskat_kroner
ejendomsskatter ægtefælles_ejendomsskatter samlet_skat_inkl_ejendomsskatter_kroner
ligningslov33 ligningslov33a hovedpersons_ligningslov33_og_33a_nedslag_øre
ægtefælles_ligningslov33_og_33a_nedslag_øre samlet_skat_inkl_ejendomsskatter_efter_ligningslov33_kroner
samlet_skat_inkl_ejendomsskatter_efter_ligningslov33_og_33a_kroner
samlet_skat_inkl_ejendomsskatter_efter_negativ_aktieindkomstskat_kroner
samlet_skat_inkl_ejendomsskatter_efter_fremført_negativ_aktieindkomstskat_kroner
eksakt_ordinær_skat_efter_dis_øre endelig_aktieindkomstskat_øre
samlet_skat_inkl_endelig_aktieindkomstskat_øre samlet_skat_inkl_ejendomsskatter_øre
samlet_skat_inkl_ejendomsskatter_efter_ligningslov33_øre
samlet_skat_inkl_ejendomsskatter_efter_ligningslov33_og_33a_øre
samlet_skat_inkl_ejendomsskatter_efter_negativ_aktieindkomstskat_øre
samlet_skat_inkl_ejendomsskatter_efter_fremført_negativ_aktieindkomstskat_øre
slutskat_øre slutskat_kroner_kompatibilitetsprojektion
slutskat_inkl_endelig_arbejdsudlejebeskatning_øre slutskat_inkl_endelig_arbejdsudlejebeskatning_kroner
personskattelov_par8a_stk5_kredit_øre kildeskat_par8a_kredit_input_gyldigt
arbejdsmarkedsbidragslov_par6_kredit_øre kildeskat_ambl_par6_kredit_input_gyldigt
afskrivningslov40c_acontoskat afskrivningslov40c_acontoskat_kredit_øre kildeskat_al40c_acontoskat_input_gyldigt
personlig_indkomst selvstændig_arbejdsmarkedsbidrag pension erhvervsbefordring ligningsfradrag
kapitalindkomst aktieavance kursgevinst_par32 udenlandske_sociale_bidrag cfc årsopgørelse
`.trim().split(/\s+/);

export const partyearResultFields = `
vurdering input_gyldigt periode_gyldig kilder_gyldige kilder_afstemt_med_personskat
helårsgrundlag_gyldigt kanonisk_beregning_understøttet helårsskattekomponenter_afstemt
skattepligtsdage valg delårsresultat delårsinput helårsinput helårsberegning helårsberegning_eksakt
statslige_skattekomponenter kommunale_og_kirkelige_skattekomponenter skatteloftsnedslag par11 sømandsbeskatning
statslig_indkomstskat_efter_par14_øre kommunal_og_kirkelig_indkomstskat_efter_par14_øre
arbejdsmarkedsbidrag_for_delåret_øre ejendomsskatter_øre efterfølgende_skat
slutskat_efter_par14_øre slutskat_efter_par14_kroner
slutskat_inkl_endelig_arbejdsudlejebeskatning_øre slutskat_inkl_endelig_arbejdsudlejebeskatning_kroner årsopgørelse
`.trim().split(/\s+/);
export const partyearIncomeFields = [
  ['bruttoløn_kroner', 'Bruttoløn'],
  ['øvrig_personlig_indkomst_kroner', 'Øvrig personlig indkomst'],
  ['nettokapitalindkomst_kroner', 'Nettokapitalindkomst'],
];
const partyearFlags = ['input_gyldigt', 'periode_gyldig', 'kilder_gyldige', 'kilder_afstemt_med_personskat',
  'helårsgrundlag_gyldigt', 'kanonisk_beregning_understøttet', 'helårsskattekomponenter_afstemt'];
export const partyearChoiceFields = ['input_gyldigt', 'valg_muligt', 'valg_afgivet_ved_oplysninger',
  'valg_gældende', 'omvalgsfrist', 'omvalg_dato_gyldig', 'omvalg_rettidigt', 'omvalg_gennemført', 'helårsomregning_skal_ske'];

const statuses = {
  BeregnetMedForbehold: 'Beregnet med forbehold — modelkontroller bestået; ikke en godkendt årsopgørelse.',
  UgyldigtBeregningsgrundlag: 'Ugyldigt beregningsgrundlag — intet beløb til sammenligning.',
};
export const incomeFields = [
  ['bruttoløn_kroner', 'Bruttoløn'],
  ['personlig_indkomst_efter_am_kroner', 'Personlig indkomst efter AM'],
  ['nettokapitalindkomst_kroner', 'Nettokapitalindkomst'],
  ['beskæftigelsesfradrag_kroner', 'Beskæftigelsesfradrag'],
  ['seniorbeskæftigelsesfradrag_kroner', 'Seniorbeskæftigelsesfradrag'],
  ['jobfradrag_kroner', 'Jobfradrag'],
  ['ekstra_pensionsfradrag_kroner', 'Ekstra pensionsfradrag'],
  ['øvrige_ligningsmæssige_fradrag_kroner', 'Øvrige ligningsmæssige fradrag'],
  ['samlede_ligningsmæssige_fradrag_kroner', 'Samlede ligningsmæssige fradrag'],
  ['almindelig_skattepligtig_indkomst_kroner', 'Almindelig skattepligtig indkomst'],
];

function object(value, where) {
  requireThat(value !== null && typeof value === 'object' && !Array.isArray(value), `${where}: objekt forventet.`);
}
function checkRow(check) {
  record(check, ['sti', 'gyldig', 'forklaring'], 'Inputkontrol');
  textValue(check.sti, 'Kontrolsti'); textValue(check.forklaring, 'Kontrolforklaring');
  requireThat(typeof check.gyldig === 'boolean', 'Kontrollens gyldighed mangler.');
}
function sameChecks(left, right) {
  return left.length === right.length && left.every((c, i) =>
    c.sti === right[i].sti && c.gyldig === right[i].gyldig && c.forklaring === right[i].forklaring);
}
function assessment(result, taxField = 'slutskat_øre') {
  const a = result.vurdering;
  object(a, 'Vurdering');
  const grouped = Object.hasOwn(a, 'kontrolgrundlag');
  record(a, ['status', 'alle_kontroller_gyldige', 'slutskat_til_sammenligning_øre',
    'samlet_modeldækning_bekræftet', 'kontroller', 'fejl', 'forbehold', ...(grouped ? ['kontrolgrundlag'] : [])], 'Vurdering');
  const status = variant(a.status, statuses, 'Vurdering');
  for (const name of ['kontroller', 'fejl', 'forbehold']) list(a[name], name);
  a.kontroller.forEach(checkRow); a.fejl.forEach(checkRow);
  let calculationCount = a.kontroller.length;
  if (grouped) {
    const g = a.kontrolgrundlag;
    record(g, ['beregning', 'afregning'], 'Kontrolgrundlag');
    for (const name of ['beregning', 'afregning']) { list(g[name], name); g[name].forEach(checkRow); }
    requireThat(sameChecks([...g.beregning, ...g.afregning], a.kontroller),
      'Kontrolgrupperne svarer ikke til den samlede kontrolliste.');
    calculationCount = g.beregning.length;
  }
  const failed = a.kontroller.filter(c => !c.gyldig);
  requireThat(sameChecks(failed, a.fejl),
  'Fejllisten svarer ikke til de returnerede kontroller.');
  const valid = calculationCount > 0 && failed.length === 0;
  requireThat(a.alle_kontroller_gyldige === valid && (status === 'BeregnetMedForbehold') === valid,
    'Beregningsstatus og kontroller er indbyrdes modstridende.');
  requireThat(a.samlet_modeldækning_bekræftet === false,
    'Denne visning understøtter ikke en påstand om fuld modeldækning.');
  requireThat(a.forbehold.length > 0, 'Resultatets forbehold mangler.');
  a.forbehold.forEach(c => textValue(c, 'Forbehold'));
  integer(a.slutskat_til_sammenligning_øre, !valid, 'Sammenligningsbeløb');
  integer(result[taxField], false, 'Diagnostisk slutskat');
  requireThat(valid ? a.slutskat_til_sammenligning_øre === result[taxField]
    : a.slutskat_til_sammenligning_øre === null, 'Sammenligningsbeløb strider mod vurderingen.');
  return { status, valid };
}

function partyearContext(result, valid) {
  for (const field of partyearFlags) requireThat(typeof result[field] === 'boolean', `Delår: ${field} skal være boolsk.`);
  requireThat(result.input_gyldigt === valid && (!valid || partyearFlags.every(field => result[field])),
    'Delårets gyldighedsflag strider mod den yderste vurdering.');
  integer(result.skattepligtsdage, false, 'Skattepligtsdage');
  for (const name of ['delårsinput', 'helårsinput']) {
    object(result[name], name); integer(result[name].skatteår, false, 'Indkomstår');
    for (const [field] of partyearIncomeFields) integer(result[name][field], false, 'Indkomstgrundlag');
  }
  record(result.valg, partyearChoiceFields, 'Delårsvalg');
  for (const field of partyearChoiceFields.filter(field => field !== 'omvalgsfrist'))
    requireThat(typeof result.valg[field] === 'boolean', 'Delårets valgflag skal være boolske.');
  if (!valid) return [];
  requireThat(result.valg.input_gyldigt, 'Delårets ugyldige valg strider mod den yderste vurdering.');
  requireThat(result.delårsinput.skatteår === result.helårsinput.skatteår
    && result.skattepligtsdage > 0n && result.skattepligtsdage <= 366n,
  'Delårets viste år eller skattepligtsdage er indbyrdes modstridende.');
  const annual = result.valg.helårsomregning_skal_ske ? 'Omregnet årsgrundlag' : 'Faktisk årsgrundlag';
  return [
    `Skattepligtsdage i output: ${result.skattepligtsdage} (datoer og skattepligt er ikke kontrolleret her).`,
    `Metode i output: ${result.valg.helårsomregning_skal_ske ? 'helårsomregning' : 'valg af faktisk helårsindkomst'}.`,
    'Udvalgte indkomstgrundlag til mellemregningen — ikke årsopgørelsens endelige rubrikbeløb:',
    ...partyearIncomeFields.map(([field, label]) => `  ${label} — Periodens grundlag: ${amount(result.delårsinput[field], 'DKK')}; ${annual}: ${amount(result.helårsinput[field], 'DKK')}`),
    'Skat i delårsresultat og helårsberegning er mellemregninger og vises ikke som slutskat.',
  ];
}

export function renderPersonskatOutput(envelope) {
  record(envelope, ['$futuruna', 'results', 'diagnostics'], 'Resultat');
  const metadata = envelope.$futuruna;
  record(metadata, ['schema', 'schema_hash', 'entry'], 'Kontrakt');
  requireThat(metadata.schema === 'futuruna.calculate.output.v1'
    && ['beregn_personskat', 'beregn_personskat_delår'].includes(metadata.entry),
  'Brug JSON-output fra beregn_personskat eller beregn_personskat_delår; andre beregninger og input understøttes ikke.');
  const partyear = metadata.entry === 'beregn_personskat_delår';
  requireThat(typeof metadata.schema_hash === 'string' && /^[0-9a-f]{64}$/.test(metadata.schema_hash),
    'Kontraktens fingeraftryk mangler.');
  list(envelope.results, 'Resultater'); list(envelope.diagnostics, 'Diagnostik');
  requireThat(envelope.results.length + envelope.diagnostics.length > 0, 'Filen indeholder ingen sager eller diagnostik.');
  const ids = new Set();
  const overview = [];
  const details = [];
  let needsAttention = envelope.diagnostics.length > 0;
  for (const row of envelope.results) {
    record(row, ['case_id', 'result'], 'Sag'); textValue(row.case_id, 'Sagsnavn');
    requireThat(!ids.has(row.case_id), 'Gentaget sagsnavn i resultater.'); ids.add(row.case_id);
    const r = row.result;
    record(r, partyear ? partyearResultFields : resultFields, 'Personskatresultat');
    const { status, valid } = assessment(r, partyear ? 'slutskat_efter_par14_øre' : 'slutskat_øre');
    needsAttention ||= !valid;
    const context = partyear ? partyearContext(r, valid) : [];
    const canonical = partyear ? r.delårsresultat : r;
    if (partyear) record(canonical, resultFields, 'Indlejret Personskatresultat');
    const year = partyear ? r.delårsinput.skatteår : r.skat?.skatteår;
    integer(year, false, 'Indkomstår');
    if (!partyear) {
      object(r.skat, 'Indkomstoversigt');
      for (const [field] of incomeFields) integer(r.skat[field], false, 'Indkomst- eller fradragsbeløb');
    }
    object(canonical.ægtefælle, 'Ægtefælle'); object(r.årsopgørelse, 'Årsopgørelse');
    const spouse = canonical.ægtefælle.$variant;
    requireThat(spouse === 'IngenÆgtefælleberegning' || spouse === 'BeregnetÆgtefælle', 'Ukendt ægtefællevariant.');
    record(canonical.ægtefælle, spouse === 'IngenÆgtefælleberegning' ? ['$variant']
      : ['$variant', 'fakta', 'grundlag', 'skat', 'samlevende_ved_indkomstårets_udløb'], 'Ægtefælle');
    const settlement = r.årsopgørelse.$variant;
    requireThat(settlement === 'IngenÅrsopgørelse' || settlement === 'BeregnetÅrsopgørelse', 'Ukendt årsopgørelsesvariant.');
    record(r.årsopgørelse, settlement === 'IngenÅrsopgørelse' ? ['$variant']
      : ['$variant', 'input', 'resultat', 'afregning'], 'Årsopgørelse');
    overview.push(`  ${visible(row.case_id)}: ${statuses[status]} (${r.vurdering.fejl.length} fejlede kontroller)`);
    details.push('', `Sag: ${visible(row.case_id)}`, `Indkomstår i output: ${year}`, statuses[status]);
    if (valid) {
      details.push(`Modelleret slutskat${partyear ? ' efter PSL § 14' : ''} til sammenligning: ${amount(r.vurdering.slutskat_til_sammenligning_øre, 'øre')}`,
        '  Kun hovedpersonens skat, ikke summen af begge ægtefællers skat.',
        '  Kun den modellerede del. Dette er ikke restskat, overskydende skat eller en udbetaling.',
        '  Ingen sammenligning med et observeret beløb i din årsopgørelse er udført her.');
      if (partyear) details.push(...context);
      else {
        details.push('Udvalgte indkomster og fradrag — delbeløb kan overlappe; summér ikke denne liste:');
        for (const [field, label] of incomeFields) details.push(`  ${label}: ${amount(r.skat[field], 'DKK')}`);
      }
    } else {
      details.push('Beløb tilbageholdt — ukendt er ikke nul. Alle øvrige beløb er kun diagnostik og vises ikke her.',
        'Gennemgå de fejlede kontrollers kildefakta og genberegn. Udfyld ikke mangler med gæt.',
        'Fejlede kontroller:');
      if (r.vurdering.fejl.length === 0) details.push(r.vurdering.kontroller.length === 0
        ? '  Ingen kontroller returneret; grundlaget kan ikke godkendes.'
        : '  Intet beregningsgrundlag returneret; afregningskontroller alene er ikke tilstrækkelige.');
      for (const c of r.vurdering.fejl) details.push(`  ${visible(c.sti)}: ${visible(c.forklaring)}`);
    }
    details.push(spouse === 'IngenÆgtefælleberegning'
      ? 'Ægtefælle: ingen ægtefælleberegning i output. Det fastslår ikke civilstand.'
      : 'Ægtefælle: medtaget i modelkontrollerne; ægtefællens egne tal vises ikke i denne oversigt.');
    details.push(settlement === 'IngenÅrsopgørelse'
      ? 'Årsopgørelsesafregning: ikke beregnet. Det betyder ikke nul i restskat eller udbetaling.'
      : 'Årsopgørelsesafregning: særskilt output i JSON, ikke vist eller valideret i denne oversigt.');
    details.push('Alle returnerede modelkontroller — bestået er ikke bekræftelse af kildefakta:',
      'Forklaringerne er faste modeltekster; de beskriver også fejlscenarier, selv når kontrollen er bestået.');
    if (r.vurdering.kontroller.length === 0) details.push('  Ingen kontroller returneret.');
    const groups = r.vurdering.kontrolgrundlag
      ? [['Beregningsgrundlag:', r.vurdering.kontrolgrundlag.beregning], ['Betalingsafregning for dette trin:', r.vurdering.kontrolgrundlag.afregning]]
      : [[null, r.vurdering.kontroller]];
    for (const [label, checks] of groups) {
      if (label) details.push(label);
      if (label && checks.length === 0) details.push('  Ingen kontroller i denne gruppe.');
      for (const c of checks) details.push(`  ${c.gyldig ? 'Bestået' : 'FEJL'} — ${visible(c.sti)}: ${visible(c.forklaring)}`);
    }
    details.push('Alle forbehold fra den yderste vurdering:');
    for (const caveat of r.vurdering.forbehold) details.push(`  - ${visible(caveat)}`);
  }
  if (envelope.diagnostics.length > 0) details.push('', 'SAGER UDEN BEREGNINGSRESULTAT:');
  for (const diagnostic of envelope.diagnostics) {
    record(diagnostic, ['case_id', 'path', 'message'], 'Diagnostik');
    for (const field of ['case_id', 'path', 'message']) textValue(diagnostic[field], 'Diagnostiktekst');
    requireThat(!ids.has(diagnostic.case_id), 'Samme sag har både resultat og diagnostik; visningen afvises.');
    details.push(`  ${visible(diagnostic.case_id)} — ${visible(diagnostic.path)}: ${visible(diagnostic.message)}`);
  }
  const conclusion = needsAttention
    ? 'Kræver gennemgang: mindst én sag har ugyldigt grundlag eller intet beregningsresultat.'
    : 'Kun beregning med forbehold. Resultaterne beviser ikke, at hele årsopgørelsen er korrekt.';
  const lines = [
    'PERSONSKAT — lokal oversigt over gemt modeloutput, ikke skatterådgivning.',
    `Beregningsvej: ${partyear ? 'delår efter PSL § 14; den yderste vurdering og endelige delårsskat' : 'ordinær Personskat'}.`,
    'Ingen genberegning, dokumentkontrol, LLM eller kontrol mod din årsopgørelse.',
    'Viser modelvurdering, alle dens kontroller/forbehold, diagnostik og udvalgte beløb for hovedpersonen.',
    'Den fulde detailberegning er i JSON; denne oversigt validerer ikke alle dens underfelter.',
    'Beløb og kildehenvisninger kan være private. Del ikke output offentligt.',
    `Kontrakt fra filen: ${metadata.schema_hash} (ikke kontrolleret mod den aktuelle model).`,
    '', conclusion,
    `Overblik: ${envelope.results.length} sager med modeloutput; diagnostiklinjer: ${envelope.diagnostics.length}.`,
    ...overview, ...details, '', conclusion,
  ];
  return { text: lines.join('\n') + '\n', exitCode: needsAttention ? 2 : 0 };
}

function main(args) {
  if (args.length === 1 && args[0] === '--help') {
    console.log('Brug: node examples/danish-income-tax/personskat-resultat.mjs RESULTATER.json');
    console.log('Viser gemt beregn_personskat- eller beregn_personskat_delår-output lokalt. Skriver ingen filer og sender ingen data.');
    console.log('Exit: 0 = beregnet med forbehold; 2 = ugyldige sager/diagnostik; 1 = fil/format kan ikke vises. Ingen skattemæssig godkendelse.');
    return 0;
  }
  requireThat(args.length === 1 && !args[0].startsWith('--'), 'Angiv én resultatfil, eller brug --help.');
  // Complete validation before printing: a malformed later case cannot leave
  // behind an apparently successful partial report.
  const rendered = renderPersonskatOutput(readSavedOutput(args[0]));
  process.stdout.write(rendered.text);
  return rendered.exitCode;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { process.exitCode = main(process.argv.slice(2)); }
  catch (error) {
    const message = error.code ? `Filen kunne ikke læses (${error.code}).` : error.message;
    console.error(`Visning afvist: ${visible(message)} Ingen skatteberegning godkendt.`);
    process.exitCode = 1;
  }
}
