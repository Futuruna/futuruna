# Ekstern kontrol: øretrin og supplerende arbejdsfradrag

Den 24.–25. september 2026 blev yderligere **30 fiktive cases** aflæst i SKATs
offentlige beregnere. Modellen matcher fradragene i 27; **tre
afvigelser på én krone står åbne**. Disse er ikke godkendt ekstern konformitet.
Kontrollen præciserer den tidligere
[11-case-kontrol](skatdk-arbejdsfradrag-ekstern.md): matematisk oprunding direkte
fra procentproduktet forklarer ikke alle observerede beløb.

## Kilder og fiktive fakta

- [Årsberegner 2025](https://www.tastselv.skat.dk/borger/beregn2025/profil.do):
  aflæs fradrag og skat i »Specifikation af beregnet skat«. Ingen
  versionsbetegnelse blev registreret.
- [Forskudsberegner 2026](https://www.tastselv.skat.dk/fskbrgn2/Skprofil.aspx?indkomstaar=2026):
  observeret profilversion **26.3.5.1**, komponenter i resultattabellen.
- [SKATs fradragsvejledning](https://skat.dk/borger/fradrag/arbejdsrelaterede-fradrag/beskaeftigelses-og-jobfradrag):
  satser og lofter. Seniorreglens lovgrundlag er fortsat
  [lov nr. 482 af 22. maj 2024](https://www.retsinformation.dk/eli/lta/2024/482).
  Denne kontrol ændrer ikke aldersperiode, berettigelse eller årssatser.

Fælles profil: ugift, København (101), ingen kirkeskat, virksomhed, ejerbolig,
udlandsforhold, ATP, pension, andre indkomster eller fradrag. Fuld skattepligt
hele året; løn i rubrik 11 (2025) eller felt 201 (2026). Øvrige valgfrie
beløbsfelter står blanke, som i den tidligere kontrol. Det er en bevidst
fiktiv nulprofil, ikke en regel om at tolke manglende personoplysninger som nul.

Almindelige cases er født 01.01.1990 og har ingen børn. Seniorcases er født
01.01.1960 og har ingen børn. Enligforsørgercases er født 01.01.1990, vælger
én børneenkeltydelse i beregnerens profil og oplyser det angivne antal
kvartaler med ekstra børnetilskud. Modeltesten bruger tilsvarende eksplicitte,
fiktive kvartalsfakta med fuldstændighed bekræftet. Antallet af børn erstatter
ikke dokumentation for ret til og modtagelse af ekstra børnetilskud i en rigtig sag.

Der er kun brugt friske anonyme formularsessioner: ingen login, CPR-numre eller
private bilag. Rå HTML, sessionsnøgler og cookies indgår ikke i repoets fixtures.

## Modelkonvention udledt af observationerne

For positive almindelige § 9 J/§ 9 K-fradrag og seniorfradrag bruges nu:

1. Beregn procentproduktet med heltal, og afkort til hele øre.
2. Oprund ørebeløbet til hele kroner; bevar årets loft.

Eksempel: 100.187 × 12,3 % = 12.323,001 kr. bliver 12.323,00 kr. og dermed
12.323 kr., ikke 12.324 kr. Dette er heller ikke afrunding til nærmeste øre:
100.122 × 12,3 % = 12.315,006 kr. gav 12.315 kr. hos SKAT.

For enligforsørgertillæg afkortes det begrænsede **årsbeløb til øre først**.
Derefter ganges med antal berettigede kvartaler og divideres med fire,
afkortes til øre igen og oprundes til kroner. Årsbeløbet oprundes ikke til
kroner før fordelingen, og hvert kvartal afrundes ikke separat.
100.661 × 11,5 % = 11.576,015 kr. giver således 11.576,01 kr.; tre fjerdedele
af dette er 8.682,0075 kr., som bliver 8.682,00 kr. og derefter 8.682 kr.

Dette er en **modelinferens fra offentlig beregneradfærd**, ikke dokumentation
for beregnerens interne kode eller en generel lovbestemmelse om afrunding.
Den forklarer nedenstående match, men ikke de tre særskilt viste afvigelser.
Ældre år bruger samme konvention som en udtrykkelig modelantagelse;
uafhængig historisk verifikation mangler fortsat (td-3f1c08).

## Aflæste fradrag

Alle beløb er kroner. Senior- og enligforsørgertillæg er separate fradrag,
ikke hele beskæftigelsesfradraget og ikke skattebesparelser.

| År | Løn | Kvartaler | Senior | Enligforsørger |
| --- | ---: | ---: | ---: | ---: |
| 2026 | 100000 | 0 | 1400 | 0 |
| 2026 | 100001 | 0 | 1401 | 0 |
| 2026 | 100286 | 0 | 1404 | 0 |
| 2026 | 100429 | 0 | 1406 | 0 |
| 2026 | 435642 | 0 | 6099 | 0 |
| 2026 | 435643 | 0 | 6099 | 0 |
| 2026 | 435644 | 0 | 6100 | 0 |
| 2025 | 100001 | 4 | 0 | 11501 |
| 2025 | 100010 | 3 | 0 | 8626 |
| 2025 | 100087 | 4 | 0 | 11510 |
| 2026 | 100001 | 1 | 0 | 2876 |
| 2026 | 100010 | 3 | 0 | 8626 |
| 2026 | 100087 | 4 | 0 | 11510 |
| 2026 | 100487 | 1 | 0 | 2889 |
| 2026 | 100661 | 3 | 0 | 8682 |
| 2026 | 100035 | 3 | 0 | 8629 |
| 2026 | 439991 | 4 | 0 | 50599 |
| 2026 | 439992 | 4 | 0 | 50600 |
| 2026 | 600000 | 1 | 0 | 12650 |

| År | Løn | Almindeligt beskæftigelsesfradrag | Jobfradrag |
| --- | ---: | ---: | ---: |
| 2025 | 100187 | 12323 | 0 |
| 2025 | 100122 | 12315 | 0 |
| 2025 | 100870 | 12408 | 0 |
| 2026 | 100251 | 12782 | 0 |
| 2026 | 100353 | 12795 | 0 |
| 2026 | 100008 | 12752 | 0 |
| 2025 | 224589 | 27625 | 4 |
| 2026 | 235289 | 30000 | 4 |

### Kendte afvigelser — ikke match

| År | Fradrag | Løn | Eksakt procentprodukt, kr. | SKAT, kr. | Model, kr. |
| --- | --- | ---: | ---: | ---: | ---: |
| 2026 | Almindeligt | 100204 | 12776,01 | 12776 | 12777 |
| 2026 | Almindeligt | 100604 | 12827,01 | 12827 | 12828 |
| 2026 | Senior | 100215 | 1403,01 | 1403 | 1404 |

Casen med løn 100.604 blev aflæst den 25. september i en ny anonym session,
fortsat profilversion 26.3.5.1. Den viser samme afvigelse ved et andet
indkomstbeløb, ikke en forklaring af beregnerens interne aritmetik.
Med samme fiktive fakta giver den kanoniske model 18.660,94 kr. i skat inklusive
AM, mens den offentlige beregner viste 18.661,18 kr. Forskellen er **24 øre i
skat**, ikke én krone i skat; én krone er forskellen i selve fradraget. Den
forskel er fortsat synlig, ikke normaliseret til et match.

Årsagen er ikke fastslået. Kontrolcasen fra 2025 med løn 100.870 har produktet
12.407,01 kr. og giver 12.408 kr. Vi har ikke kildegrundlag for generelt at
slette den sidste øre, indføre en tolerance eller forklare forskellen som en
lovændring eller bevist flydestøjsfejl. Modellen bevarer den eksakte øre.
Opfølgning: td-68c9d3. Ingen afvigelse her beviser, at en borgers årsopgørelse
er forkert; ret ikke borgerens fakta for at fremtvinge et match. Den kanoniske
beregnings `vurdering.forbehold` gør nu også denne afrundingsusikkerhed synlig
i selve resultatet, både ved gyldigt og ugyldigt grundlag. Forbeholdet ændrer
ikke beløb, gyldighed eller sammenligningstolerance og siger ikke, at enhver
difference skyldes afrunding. Kontrol af historisk forskudsberegner for 2025
blev afvist af en omdirigering til beregnerens fejlside; der er ikke opnået ny
historisk konformitetsevidens.

## Sammenhæng med den beregnede skat

Ti af de nye cases har også aflæste forventninger gennem den kanoniske
[integrationstest](../../tests/personskat_pension_timing.rs),
`supplementary_deductions_and_sub_ore_cases_match_official_tax_components`.
Her kontrolleres fradrag, skattepligtig indkomst, kommuneskat før personfradrag
og samlet beregnet skat inklusive AM — begge skattebeløb i øre.

| År | Profil | Løn | Skattepligtig indkomst, kr. | Kommuneskat, øre | Beregnet skat inkl. AM, øre |
| --- | --- | ---: | ---: | ---: | ---: |
| 2026 | Senior | 100001 | 77849 | 1820888 | 1810680 |
| 2026 | Senior | 435643 | 336048 | 7860162 | 14243633 |
| 2026 | Senior | 435644 | 336048 | 7860162 | 14243645 |
| 2025 | Enligforsørger, 4 kvartaler | 100001 | 68199 | 1602676 | 1675292 |
| 2026 | Enligforsørger, 3 kvartaler | 100010 | 70632 | 1652082 | 1641982 |
| 2026 | Enligforsørger, 3 kvartaler | 100661 | 71092 | 1662841 | 1665135 |
| 2026 | Enligforsørger, 4 kvartaler | 439992 | 294994 | 6899909 | 13366232 |
| 2025 | Almindelig | 100187 | 79850 | 1876475 | 1952556 |
| 2026 | Almindelig | 100251 | 79449 | 1858312 | 1852866 |
| 2026 | Almindelig | 235289 | 186462 | 4361346 | 6928262 |

**Dette er før grøn check, forskudsbetalinger, renter og øvrig slutafregning.**
Forskudsberegnerens »Samlet forskudsskat efter grøn check« er ikke testens
forventning; børnevalget kan udløse grøn check uden for de sammenlignede
komponenter. Ingen af disse tests validerer tilbagebetaling eller restskat.
Samlet modeldækning forbliver ubekræftet, også når komponenterne matcher.

## Kør den afgrænsede kontrol

Brug den afprøvede compiler fra
[tax-audit-opsætningen](../../website/public/ai-setup.md#tax-audit-runtime-check):

```sh
"$RUNA_BIN" examples/danish-income-tax/skatdk-fradrag-oere-ekstern.scenario.runa
"$RUNA_BIN" run examples/danish-income-tax/skatdk-fradrag-oere-ekstern.scenario.runa
```

Det [eksekverbare scenarie](skatdk-fradrag-oere-ekstern.scenario.runa) bruger
aflæste beløb, ikke forventninger beregnet af modellen. Det udskriver både
27 match og de tre åbne afvigelser. En bestået kontrol af, at afvigelserne stadig
er synlige, betyder ikke, at de stemmer med SKAT. Testene kører offline.

Native/interpreter-paritet gælder dette lille fradragsmodul. Skattetesten bruger
`runa call`; den validerer ikke hele Personskats native kodegenerering.
Ekstra pensionsfradrags afrunding er undersøgt i en
[særskilt kontrol for 2025/2026](skatdk-pensionsfradrag-ekstern.md).
Historiske år og fuld delårskonformitet er ikke verificeret af disse
observationer. Inputtyper ændres ikke; genberegn
tidligere resultater efter modelopdateringen. Modellen er forskningssoftware,
ikke individuel skatterådgivning.
