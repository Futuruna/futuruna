# Kontrollér grøn check

Den [kompakte beregning](groen-check.calculate.runa) viser pensionistbeløb,
børnekompensation, indkomstaftrapning og lavindkomsttillæg hver for sig. Den
kan sammenligne summen med rapportens oplyste kredit uden at bruge den kredit
som beregningsinput. Reglerne udføres med heltalsaritmetik, ikke en LLM.

Dette er en **delberegning ud fra oplyste indkomsttal og kildefakta**, ikke en
uafhængig genberegning af hele skatten eller dokumentautentifikation. Den er
additiv og ændrer **ikke automatisk** Personskats årsopgørelse eller den
[betingede rapportafstemning](aarsopgoerelse-afstemning.md).
Vil du genberegne fra Personskats indkomst- og fradragsfakta, så brug den
[særskilte samlede indgang](personskat-groen-check.md). Den udleder indkomst,
indsætter kreditten én gang og kontrollerer den endelige slutopgørelse.
Modellen er researchsoftware; beregningskontrakterne er Preview.

## Kør lokalt

Brug først [runtime-kontrollen](../../website/public/ai-setup.md#tax-audit-runtime-check).
Fra checkoutens rod, med din allerede verificerede compiler:

```sh
runa template examples/danish-income-tax/groen-check.calculate.runa --format json --output /absolut/sti/uden-for-repo/groen-check-input.json
runa call examples/danish-income-tax/groen-check.calculate.runa --input /absolut/sti/uden-for-repo/groen-check-input.json --output /absolut/sti/uden-for-repo/groen-check-resultat.json
```

Erstat stierne med en eksisterende privat mappe og udfyld input før `call`.
Bevar den genererede `$futuruna`-kontrakt. `schema` viser alle typer, alternativer
og spørgsmål; `person` og `indkomst` er valgfrie JSON-objekter, ikke tekstfelter.
`null` betyder ukendt, mens et faktisk 0 betyder kendt nul.

Du skal kende:

- Fødselsdato, skattepligt den 1. januar, forskerordning og årets forløb.
  Folkepensionsalderen beregnes fra datoen: modtagelse af folkepension er ikke
  et ekstra krav, når alderen er nået. Under alderen kræves status for
  førtids-, senior- eller tidlig pension efter socialpensionslovene ved årets
  udløb. Privat ratepension er ikke denne status.
- Personlig indkomst efter AM, eventuelt PBL § 16-tillæg og nettokapitalindkomst
  i hele DKK med kilde. Brug ikke bruttoløn, skattepligtig indkomst efter
  ligningsmæssige fradrag eller kun beløbet over topskattegrænsen. Indkomsten
  genberegnes ikke fra lønsedler i denne delberegning.
- Ægteskab/samliv og, for den dækkede helårsgren, ægtefællens nettokapitalindkomst
  med kilde. En fuld ægtefællerapport er ikke nødvendig, men ukendt indkomst
  må ikke udfyldes med nul.
- En komplet børneliste, også når den er tom. For hvert barn: lokal reference
  uden CPR, fødselsdato, ophold, ægteskab og offentlig forsørgelse den 1. januar
  samt din ret til halv/hel børne- og ungeydelse ved årets udløb. Dette er ikke
  det ekstra børnetilskud til enlige forsørgere. Der sker ikke kvartalsvis
  periodisering af den grønne check.

`oplyst_samlet_grøn_check_øre` er valgfri. Summér rapportens grønne check,
børnekompensation og tillæg én gang. 1.285,00 DKK angives som `128500` øre.

## Sådan læses resultatet

`GrønCheckBeregnet` betyder, at modellen har beregnet komponenterne. Kun en
oplyst observation giver en `difference_øre`; `null` er ingen sammenligning.
`GrønCheckAfviger` viser en forskel, ikke i sig selv en fejl hos Skattestyrelsen.
Differencen er **oplyst minus beregnet**, uden tolerance.

Ved `GrønCheckUfuldstændig`, `GrønCheckUgyldigtInput` eller
`GrønCheckUdenForDækning` er `beregning` og `difference_øre` begge `null`.
Læs alle `uafklaret`-punkter og alle `forbehold`, også når en CLI-kørsel lykkes.
Intet beregningsbeløb er ikke det samme som ingen ret til kompensation.

For et fiktivt barnløst menneske født i 1950, almindeligt helår 2025, fuld
skattepligt, ingen forskerordning, ingen ægtefælle og ingen kapitalindkomst:

| Personlig indkomst efter AM | Pensionistbeløb | Tillæg | Samlet kredit |
| --- | --- | --- | --- |
| 277.800 DKK | 1.005,00 DKK | 280,00 DKK | 1.285,00 DKK |
| 277.801 DKK | 1.005,00 DKK | 0,00 DKK | 1.005,00 DKK |
| 475.301 DKK | 1.004,93 DKK | 0,00 DKK | 1.004,93 DKK |

Kompensationen hører til **kreditterne efter KSL § 60**, ikke ligningsmæssige
fradrag. I den eksisterende Personskat-kontrakt hedder den
`energiafgiftskompensation_øre` i den eksakte kreditliste. Kopiér aldrig
beløbet ind oven i en allerede medregnet kredit; dette modul foretager ingen
automatisk overførsel. KSL § 62 har desuden egne regler om, hvilken del af
tilbagebetalingen der kan indgå i godtgørelsesgrundlaget.

## Lovgrundlag og projektion

[Kompensationsloven, LBK 1306/2022](https://www.retsinformation.dk/eli/lta/2022/1306),
§§ 1–5, er grundlaget. Pensionistbeløbet er 875 DKK i 2023/2026, 1.000 DKK i
2024 efter [lov 96/2025 § 3](https://www.retsinformation.dk/eli/lta/2025/96)
og 1.005 DKK i 2025 efter
[lov 1779/2025 § 5](https://www.retsinformation.dk/eli/lta/2025/1779).
Det ekstra lavindkomsttillæg er 280 DKK og bortfalder først **over** grænsen.
Aftrapning sker med 7,5 %. Børnegrænsens pensionisttillæg forbliver 11.667 DKK,
også i 2024/2025, hvor det derfor ikke er lig pensionistbeløbet divideret med
7,5 %.

| År | Aftrapningsgrænse | Lavindkomsttillæggets grænse |
| --- | --- | --- |
| 2023 | 441.900 DKK | 258.300 DKK |
| 2024 | 457.500 DKK | 267.400 DKK |
| 2025 | 475.300 DKK | 277.800 DKK |
| 2026 | 498.200 DKK | 291.100 DKK |

Grænserne er kontrolleret 25. september 2026 mod
[ministeriets 2023–2024-reguleringstabel](https://svmn.dk/tal-og-metode/satser/regulering-af-beloebsgraenser/beloebsgraenser-i-skattelovgivningen-der-reguleres-efter-personskattelovens-20-2023-2024)
og [Skattestyrelsens 2025–2026-vejledning](https://skat.dk/borger/fradrag/groen-check).
Kapitalindkomstens grundbeløb genbruges fra modellens nationale årsparametre.
Fra 2026 henvises til mellemskat efter
[lov 1642/2025 §§ 3 og 10](https://www.retsinformation.dk/eli/lta/2025/1642),
ikke den nye, højere topskattegrænse.

For børn er enkeltbeløbet 120 DKK, højst 240 DKK; en hel ydelse giver dobbelt
beløb og forhøjer loftet med 120 DKK pr. dobbeltbarn, højst 240 DKK ekstra.
En blandet liste med én hel og to halve ydelser har derfor loft 360 DKK, ikke
480 DKK. Årets fødsler efter 1. januar og børn, der fylder 18 i året, tæller
ikke. Korte udlandsophold og kvalificerende uddannelsesophold har § 1, stk. 3's
undtagelse. Henvisningen til serviceloven ændres til barnets lov fra 2024 efter
[lov 753/2023 § 38](https://www.retsinformation.dk/eli/lta/2023/753).

Fra 2026 giver `GrønCheckRestEfterYdelseTilBarnet` også dobbelt enkeltbeløb:
forældremyndighedsindehaveren skal have ret til resten, når Udbetaling Danmark
har afgjort, at en del udbetales til barnet. Det følger af
[lov 1642/2025 § 3, nr. 4, og § 10](https://www.retsinformation.dk/eli/lta/2025/1642)
og forklares i [L 22, bemærkningerne til § 3, nr. 4](https://www.retsinformation.dk/eli/ft/202512L00022).
Får barnet hele ydelsen, vælges ingen ydelse for forælderen — ikke hel ydelse.

Loftets henvisning til § 5, 2. pkt., er ikke ændret til også at nævne det nye
3. pkt. Modellen afgør ikke denne fortolkning. Den beregner beløbet både uden
og med loftsforhøjelse for rest-modtagerens børn og kræver **samme beløb før
indkomstaftrapning**. Ellers tilbageholdes beregningen. Eksempler uden andre
berettigede børn:

| Ydelsesret i 2026 | Uden/med den omtvistede loftsforhøjelse | Resultat før aftrapning |
| --- | --- | --- |
| Resten for ét barn | 240 / 240 DKK | 240 DKK |
| Resten for ét barn og halv ydelse for et andet | 240 / 360 DKK | Tilbageholdt |
| Resten for to børn | 240 / 480 DKK | Tilbageholdt |
| Hel ydelse for to børn og resten for et tredje | 480 / 480 DKK | 480 DKK |

Dette er en afgrænset modeludvidelse, ikke en afklaring af loftet eller en
eksternt observeret 2026-beregning. Ukendte børnefakta tilbageholder stadig
beløbet. Rest-alternativet tilbageholdes for 2023–2025. Typer og felter er
uændrede; tidligere tilbageholdte, nu loftsuafhængige 2026-sager kan få beløb.
Generér en frisk kontraktskabelon ved overgang til den opdaterede model.

Loven giver procentreglen. Den administrative øreprojektion er en særskilt
implementeringsbeslutning: restbeløbet efter aftrapning afrundes til nærmeste
øre, halv op. Beregningen bevarer halve øre i heltal frem til projektionen;
den afrunder ikke først reduktionen eller hvert barn særskilt.

### Uafhængige 2025-observationer

Følgende fiktive input blev kørt i
[SKATs anonyme årsberegner 2025](https://www.tastselv.skat.dk/borger/beregn2025/profil.do)
25. september 2026. Ingen private oplysninger, MitID eller eksisterende session
blev anvendt. Alle er enlige helårspersoner, København, ingen kirkeskat,
indkomst i rubrik 16, øvrige indkomst-/fradrags-/betalingsfelter tomme, med de
udtrykkelige undtagelser nedenfor. De er delbeløbsobservationer, ikke bevis for
hele Personskats overensstemmelse.

| Fiktivt tilfælde | Observeret kredit | Permanent kontrol |
| --- | --- | --- |
| Født 1950, indkomst 277.800 | 100.500 + 28.000 øre | `skat-2025-supplement-equality` |
| Samme, indkomst 277.801 | 100.500 øre | `skat-2025-supplement-above` |
| Samme, indkomst 475.301 | 100.493 øre | `skat-2025-first-krone-phaseout` |
| Samme, indkomst 475.302 | 100.485 øre | `skat-2025-second-krone-phaseout` |
| Født 1990, indkomst 475.301, halv ydelse for ét barn | 11.993 øre | `skat-2025-child-half` |
| Født 1950, indkomst 486.968, halv ydelse for ét barn | 12.990 + 11.993 øre | `skat-2025-overlapping-phaseouts` |
| Født 1950, indkomst 277.800, renteindtægt 52.401 i rubrik 31 | 100.500 øre, intet tillæg | `skat-2025-capital-removes-supplement` |

Kontrollerne i [green_check.test.mjs](../../tests/green_check.test.mjs) kalder
den faktiske Futuruna-kontrakt. Øvrige testcases er lov-/modelgrænser, ikke
yderligere eksterne observationer. Kør uden nyt build:

```sh
FUTURUNA_MODEL_TEST_RUNA="$PWD/target/release/runa" node --test tests/green_check.test.mjs
```

### Kendte grænser — ingen opdigtet nulret

- Ukendte fakta eller ufuldstændig børneliste tilbageholder beregningen.
- Delår, dødsår, migration, grænsegængere og ægteskab/samliv kun en del af året
  er ikke dækket. Det er ikke en påstand om, at disse personer ingen ret har.
- Ægtefæller: når mindst én har nettokapitalindkomst over årets individuelle
  grundbeløb, tilbageholdes beløbet (`td-5f8fb4`). Eksempel: egne/personens
  ægtefælles personlige indkomster 475.300/200.000 og kapitalindkomster
  0/110.000 DKK gav personens kredit 1.005 DKK i den anonyme 2025-beregner;
  ved 110.000/0 gav den 0 DKK. Den fælles kapitalregel og
  [de oprindelige lovbemærkninger](https://www.retsinformation.dk/eli/ft/200812L00198)
  peger på fælles overskydende kapital hos den højeste personlige indkomst
  (her 5.200 DKK og 615 DKK resterende kredit i begge tilfælde). Vi har ikke
  forklaret forskellen og ændrer ikke lovmodellen for at få et match.
  En yderligere kontrol med 60.000/0 DKK kapital og samme personlige indkomster
  gav 435 DKK, selv om den fælles kapital er under 104.800 DKK. Kvitteringen
  bekræfter gift-profil og begge indkomsttal. Derfor er fælles kapital under
  dobbeltgrænsen ikke nok til at ophæve sikkerhedsgrænsen. Ved 60.000/60.000 DKK
  kapital gav beregneren også 435 DKK; fællesreglen ville her give 0 DKK.
  [Ministeriets historiske oversigt fra 2022](https://svmn.dk/tal-og-metode/satser/skattehistorik/groen-check-en-historisk-oversigt)
  beskriver også den fælles kapitalregel; den er ikke en kilde til de aktuelle
  satser. [Beregnerens begrænsningsside](https://info.skat.dk/SKAT.aspx?oID=170595&layout=2503),
  kontrolleret 25. september 2026, forklarer ikke denne forskel. De nye
  regressioner sikrer fortsat tilbageholdelse, ikke et match med 435 DKK.
- Når ydelsen delvis udbetales til barnet selv, er samspillet mellem den nye
  § 5, 3. pkt., og loftets henvisning til 2. pkt. uafklaret (`td-6d49ab`).
  Fra 2026 beregnes kun de ovenfor beskrevne loftsuafhængige tilfælde.
  Vælg det særskilte alternativ; ommærk ikke forløbet som en almindelig hel
  ydelse for at få et beløb. En oplyst rapportkredit vælger ikke fortolkningen.
- Historisk og universel administrativ øreoverensstemmelse er ikke bevist.
  Personskats eksisterende afregnings-/renteregler genprøves ikke af modulet.
