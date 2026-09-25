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
function assessment(result) {
  const a = result.vurdering;
  record(a, ['status', 'alle_kontroller_gyldige', 'slutskat_til_sammenligning_øre',
    'samlet_modeldækning_bekræftet', 'kontroller', 'fejl', 'forbehold'], 'Vurdering');
  const status = variant(a.status, statuses, 'Vurdering');
  for (const name of ['kontroller', 'fejl', 'forbehold']) list(a[name], name);
  a.kontroller.forEach(checkRow); a.fejl.forEach(checkRow);
  const failed = a.kontroller.filter(c => !c.gyldig);
  requireThat(failed.length === a.fejl.length && failed.every((c, i) =>
    c.sti === a.fejl[i].sti && c.gyldig === a.fejl[i].gyldig && c.forklaring === a.fejl[i].forklaring),
  'Fejllisten svarer ikke til de returnerede kontroller.');
  const valid = a.kontroller.length > 0 && failed.length === 0;
  requireThat(a.alle_kontroller_gyldige === valid && (status === 'BeregnetMedForbehold') === valid,
    'Beregningsstatus og kontroller er indbyrdes modstridende.');
  requireThat(a.samlet_modeldækning_bekræftet === false,
    'Denne visning understøtter ikke en påstand om fuld modeldækning.');
  requireThat(a.forbehold.length > 0, 'Resultatets forbehold mangler.');
  a.forbehold.forEach(c => textValue(c, 'Forbehold'));
  integer(a.slutskat_til_sammenligning_øre, !valid, 'Sammenligningsbeløb');
  integer(result.slutskat_øre, false, 'Diagnostisk slutskat');
  requireThat(valid ? a.slutskat_til_sammenligning_øre === result.slutskat_øre
    : a.slutskat_til_sammenligning_øre === null, 'Sammenligningsbeløb strider mod vurderingen.');
  return { status, valid };
}

export function renderPersonskatOutput(envelope) {
  record(envelope, ['$futuruna', 'results', 'diagnostics'], 'Resultat');
  const metadata = envelope.$futuruna;
  record(metadata, ['schema', 'schema_hash', 'entry'], 'Kontrakt');
  requireThat(metadata.schema === 'futuruna.calculate.output.v1' && metadata.entry === 'beregn_personskat',
    'Brug JSON-output fra beregn_personskat; andre beregninger og input understøttes ikke.');
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
    record(r, resultFields, 'Personskatresultat');
    const { status, valid } = assessment(r);
    needsAttention ||= !valid;
    object(r.skat, 'Indkomstoversigt'); integer(r.skat.skatteår, false, 'Indkomstår');
    for (const [field] of incomeFields) integer(r.skat[field], false, 'Indkomst- eller fradragsbeløb');
    object(r.ægtefælle, 'Ægtefælle'); object(r.årsopgørelse, 'Årsopgørelse');
    const spouse = r.ægtefælle.$variant;
    requireThat(spouse === 'IngenÆgtefælleberegning' || spouse === 'BeregnetÆgtefælle', 'Ukendt ægtefællevariant.');
    record(r.ægtefælle, spouse === 'IngenÆgtefælleberegning' ? ['$variant']
      : ['$variant', 'fakta', 'grundlag', 'skat', 'samlevende_ved_indkomstårets_udløb'], 'Ægtefælle');
    const settlement = r.årsopgørelse.$variant;
    requireThat(settlement === 'IngenÅrsopgørelse' || settlement === 'BeregnetÅrsopgørelse', 'Ukendt årsopgørelsesvariant.');
    record(r.årsopgørelse, settlement === 'IngenÅrsopgørelse' ? ['$variant']
      : ['$variant', 'input', 'resultat', 'afregning'], 'Årsopgørelse');
    overview.push(`  ${visible(row.case_id)}: ${statuses[status]} (${r.vurdering.fejl.length} fejlede kontroller)`);
    details.push('', `Sag: ${visible(row.case_id)}`, `Indkomstår i output: ${r.skat.skatteår}`, statuses[status]);
    if (valid) {
      details.push(`Modelleret slutskat til sammenligning: ${amount(r.vurdering.slutskat_til_sammenligning_øre, 'øre')}`,
        '  Kun hovedpersonens skat, ikke summen af begge ægtefællers skat.',
        '  Kun den modellerede del. Dette er ikke restskat, overskydende skat eller en udbetaling.',
        '  Ingen sammenligning med et observeret beløb i din årsopgørelse er udført her.',
        'Udvalgte indkomster og fradrag — delbeløb kan overlappe; summér ikke denne liste:');
      for (const [field, label] of incomeFields) details.push(`  ${label}: ${amount(r.skat[field], 'DKK')}`);
    } else {
      details.push('Beløb tilbageholdt — ukendt er ikke nul. Alle øvrige beløb er kun diagnostik og vises ikke her.',
        'Gennemgå de fejlede kontrollers kildefakta og genberegn. Udfyld ikke mangler med gæt.',
        'Fejlede kontroller:');
      if (r.vurdering.fejl.length === 0) details.push('  Ingen kontroller returneret; grundlaget kan ikke godkendes.');
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
    for (const c of r.vurdering.kontroller) {
      details.push(`  ${c.gyldig ? 'Bestået' : 'FEJL'} — ${visible(c.sti)}: ${visible(c.forklaring)}`);
    }
    details.push('Alle returnerede forbehold:');
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
    console.log('Viser gemt beregn_personskat-output lokalt. Skriver ingen filer og sender ingen data.');
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
