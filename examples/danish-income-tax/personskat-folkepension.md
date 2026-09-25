# Folkepension og tillæg: hvad indgår i skatten?

Folkepension er ikke løn, og ikke alle tillæg beskattes ens. Brug ydelsens
navn og lovgrundlag på pensionsmeddelelsen eller tillægsafgørelsen — ikke kun
nettobeløbet på bankkontoen.

| Dokumenteret ydelse | Denne models behandling |
| --- | --- |
| Folkepensionens grundbeløb og pensionstillæg | Personlig indkomst uden AM-bidrag |
| Supplerende pensionsydelse (ældrecheck) | Personlig indkomst uden AM-bidrag |
| Personligt tillæg og varmetillæg efter socialpensionslovens § 14 | Skattefrit tillæg, ikke personlig indkomst |
| Helbredstillæg efter socialpensionslovens § 14 a | Skattefrit tillæg, ikke personlig indkomst |

Grundlaget er [Den juridiske vejlednings pensionsafsnit](https://info.skat.dk/data.aspx?oid=1976824),
[PSL § 3's personlige indkomst](https://info.skat.dk/data.aspx?oid=2061678),
[LL § 7, nr. 8](https://www.retsinformation.dk/eli/lta/2025/1500) og
[SKATs AM-vejledning](https://skat.dk/borger/am-bidrag).
Modellen dækker ordinære danske poster for 2023–2026. Den fastsætter ikke
retten til pension/tillæg eller størrelsen på din ydelse. Skattefrihed gælder
de angivne lovbestemte tillæg, ikke enhver betaling med ordet »tillæg«.
Beregningskontrakten er Preview og forskningssoftware, ikke individuel rådgivning.

## Prøv med et fiktivt grundbeløb

Brug [det kontrollerede beregningsprogram](../../website/public/ai-setup.md#tax-audit-runtime-check).
Fra projektmappen:

```sh
./target/release/runa template examples/danish-income-tax/personskat-folkepension.calculate.runa \
  --format json --output /tmp/futuruna-folkepension.json
```

Bevar skabelonens `$futuruna`-del. Erstat kun `cases[0].input` med:

```json
{
  "identifikation": "fiktivt-grundbeløb",
  "indkomstår": 2025,
  "kildereference": "fiktiv-pensionsmeddelelse",
  "art": {"$variant": "FolkepensionGrundbeløb"},
  "beløb_før_skat_kroner": 80000,
  "ordinær_dansk_udbetaling_uden_korrektioner": true
}
```

```sh
FUTURUNA_CALCULATION_JOBS=1 ./target/release/runa call \
  examples/danish-income-tax/personskat-folkepension.calculate.runa \
  --input /tmp/futuruna-folkepension.json
```

Det giver gyldigt input og 80.000 kr. personlig indkomst uden AM. Det er en
klassifikation af beløbet, ikke din samlede skat. Ved den dokumenterede art
`FolkepensionVarmetillægEfterPar14` står beløbet i stedet som
`skattefrit_tillæg_kroner`. Ukendt art eller afgrænsning er ugyldigt; de
diagnostiske nuller er ikke dokumentation for skattefrihed.

## Brug beløbene i din samlede skatteberegning

I en frisk Personskat-skabelon vælges `PersonskatFolkepensionsudbetaling` i
`lønmodtager.personlig_indkomst.ordinære_forhold.forenings_og_arbejdsløshedsydelser`.
Variantens `fakta` har formen ovenfor. Maskinstien er bevaret; den danske
etiket nævner folkepension. Ægtefællen har samme indgang og beskattes af egne
udbetalinger. Den nye variant ændrer fingeraftrykket, så gamle skabeloner skal
genereres på ny, og gennemgåede kildefakta overføres.

Opret særskilte poster for `FolkepensionGrundbeløb`, `FolkepensionPensionstillæg`,
`FolkepensionÆldrecheck`, `FolkepensionPersonligtTillægEfterPar14`,
`FolkepensionVarmetillægEfterPar14` og `FolkepensionHelbredstillægEfterPar14A`.
Brug årets faktiske udbetaling før A-skat, ikke en månedssats eller en
lovbestemt maksimumssats. Medtag ikke både en samlet betaling og dens dele.
Dublerede identiteter afvises, men forskellige identiteter beviser ikke,
at betalingerne er forskellige. En tom liste beviser heller ikke fravær.

Folkepension må ikke indtastes som løn eller som en privat PBL § 20-pension.
Ellers kan der opstå forkert AM, arbejdsfradrag eller modregning i det ekstra
pensionsfradrag. Løn, private pensionsudbetalinger og ATP-udbetalinger har
deres egne indgange. En privat pensionsindbetaling kan stadig give fradrag;
[LL § 9 L](https://skat.dk/borger/fradrag/ekstra-pensionsfradrag) afgrænser, hvilke
pensionsudbetalinger der skal modregnes. Folkepensionen her er ikke en sådan
PBL § 20-udbetaling. De dækkede poster indgår heller ikke i LL § 9 C's
aftrapningsindkomst; faktisk befordring og øvrige betingelser skal stadig oplyses.

Opsat pension, efterlevelsespension, udlandsforhold, tilbagebetaling og
omperiodisering er uden for denne indgang. Førtids-, senior- og tidlig pension
må heller ikke omdøbes til folkepension for at få en beregning. Sociale
pensioner beskattes som udgangspunkt i udbetalingsåret; en efterbetaling
skal ikke automatisk flyttes til det år, pensionen vedrører.

Læs altid `vurdering` før sammenligningsbeløbet. Ugyldige kildefakta giver
ingen gyldig samlet sammenligning. For en afledt grøn check og dens virkning
på afregningen bruges [Personskat med grøn check](personskat-groen-check.md).
Denne indkomstpost fastslår ikke alene de nødvendige grøn-check-fakta.

## To uafhængige kontrolprofiler

Den 25. september 2026 blev to fiktive profiler aflæst i
[SKATs anonyme årsberegner for 2025](https://www.tastselv.skat.dk/borger/beregn2025/profil.do):
født 01.01.1955, ugift, København, fuldt skattepligtig hele året, uden kirkeskat,
ATP, privat pension, bolig, virksomhed eller andre indkomster/fradrag.
De fravær er eksplicitte fiktive fakta, ikke standardantagelser om en borger.
180.000 kr. pension blev indtastet i rubrik 16A, og eventuel løn i rubrik 11.
Ingen login eller private oplysninger; ingen versionsbetegnelse registreret.

| Årlig løn, kr. | Pension, kr. | AM, kr. | Beskæftigelsesfradrag, kr. | Skattepligtig indkomst, kr. | Beregnet skat inkl. AM, kr. |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 180.000 | 0 | 0 | 180.000 | 45.594,84 |
| 100.000 | 180.000 | 8.000 | 12.300 | 259.700 | 83.373,54 |

Kontrollen vedrører beregnet skat **før** grøn check, forudbetalinger og
afregningstillæg. Den beviser ikke restskat, ydelsesret eller ydelsens størrelse.
Den permanente [integrationstest](../../tests/personskat_validity.rs),
`folkepension_and_exempt_supplements_preserve_tax_and_pension_deduction_bases`,
bruger de aflæste beløb. Dens tillægs-, ægtefælle-, pensionsfradrags-, kørsels-
og ugyldighedscases er modelregressioner, ikke flere eksterne observationer.
Øvrige [modelgrænser og afrundingsforbehold](personskat-validity.md) består.
