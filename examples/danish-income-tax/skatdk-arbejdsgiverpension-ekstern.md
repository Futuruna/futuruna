# Arbejdsgiverpension og ATP: ekstern kontrol, 2025

Den 25. september 2026 blev fire **fiktive** profiler sendt gennem
[SKATs anonyme årsberegner for 2025](https://www.tastselv.skat.dk/borger/beregn2025/profil.do).
To profiler matcher modellen frem til samlet skat i øre. To profiler med
arbejdsgiveradministreret ratepension gør **ikke**. Afvigelserne er ikke
afrundingsforskelle og er ikke løst ved at ændre input eller skatteformler.

Dette er observationer af den offentlige beregner, ikke personlige
årsopgørelser eller dokumentation for, at Skattestyrelsens faktiske
årsopgørelser har samme adfærd. De typede beregninger er Preview, og modellen
er forskningssoftware, ikke individuel rådgivning.

## Kilder, metode og afgrænsning

[LL § 9 L](https://www.retsinformation.dk/eli/lta/2025/1500) og
[Den juridiske vejledning om ekstra pensionsfradrag](https://info.skat.dk/data.aspx?oid=2273726)
omfatter relevante pensionsindbetalinger med fradrags- eller bortseelsesret.
Modellens bevarede lovcitat skelner mellem arbejdsgiverrate og øvrige
arbejdsgiverordninger; netto efter indeholdt AM indgår i § 9 L-grundlaget.
[SKATs borgervejledning](https://skat.dk/borger/fradrag/ekstra-pensionsfradrag)
beskriver også ekstra fradrag for både egne og arbejdsgiverens indbetalinger.
De kilder giver ikke grundlag for at fjerne det almindelige ratebidrag fra
modellens ekstra pensionsfradrag for at efterligne nedenstående observationer.

Der var ingen tilgængelig browserforbindelse ved denne kontrol. Friske,
anonyme HTTP-sessioner fulgte den offentlige formulars faktiske felter og
handlinger: `profil.do` → `indberet.do` → `kvit.do` → `spec.do`.
Specifikationen blev åbnet med formularens `person=hop`. Tilbagefunktionen
bekræftede de indsendte beløb uændret. Ingen login, CPR-numre eller private
dokumenter indgik. HTML, sessionsoplysninger og forsøgsprogrammet opbevares
uden for repoet; regressionen nedenfor kører offline.

Det er **ikke** en ny afprøvning af den grafiske browsers samlede forløb.
Beregnerens versionsnummer blev ikke registreret. Årsagen til afvigelserne,
herunder den præcise feltfortolkning og forskelle fra en virkelig
årsopgørelse, er fortsat åben (`td-0e5d15`).

## Fiktive kildefakta og felter

Alle profiler: 2025, født 1. januar 1990, enlig, København (101), ingen
kirkeskat, børn, virksomhed, ejendom eller udenlandske forhold. Fuld dansk
skattepligt og DBO-hjemsted hele året. Ingen privat pension, udbetalinger fra
pension i 2024/2025, andre indkomster eller fradrag. Beløbsfelter uden for
profilen var blanke. Det udtrykkelige fravær gælder kun disse opdigtede
profiler; ukendte forhold i virkelige dokumenter er ikke nul.

Løn er allerede efter medarbejderens bidrag til arbejdsgiverpension/ATP,
men før løn-AM og skat. Hvor pension forekommer, er 50.000 kr. brutto og
4.000 kr. indeholdt AM selvstændigt valgte kildefakta; netto er 46.000 kr.
ATP er særskilt 3.000 kr. brutto og 2.760 kr. dokumenteret netto. Beløbene
er illustrative, ikke en udledning af en bestemt ATP-sats. Ratepensionen
har almindelig bortseelsesret og er under årsgrænsen; livrenteprofilen er
en **anden** ordning, ikke en omklassifikation for at få et match.

| Profil | Løn, rubrik 11 | Arbejdsgiverrate efter AM (`PRA`) | Øvrige arbejdsgiverordninger (`PRATPA`) |
| --- | ---: | ---: | ---: |
| ATP alene, lav løn | 100000 | blank | 2760 |
| Livsvarig livrente og ATP | 600000 | blank | 48760 |
| Ratepension alene | 600000 | 46000 | blank |
| Ratepension og ATP, som bilagsdemoen | 600000 | 46000 | 2760 |

[Hjælpen til ratefeltet](https://info.skat.dk/data.aspx?oid=1942575) angiver
bidraget inklusive arbejdsgiverandel efter indeholdt AM. Den nævner også en
ældre beløbsgrænse; årets grænse aflæses ikke derfra.
[Hjælpen til øvrige ordninger](https://info.skat.dk/data.aspx?oid=2347777)
giver kun en kort feltbeskrivelse. Derfor bevares feltnavne, opdeling og
usikkerheden frem for at påstå fuldstændigt dokumenteret feltsemantik.

## Aflæste specifikationer

Fradrag/indkomst er **kroner**; skattekolonner er **øre**. Kommuneskat er før
personfradrag. Samlet skat inkluderer løn-AM; den omfatter ikke ATP's eller
pensionsinstituttets allerede indeholdte AM, betalingsafregning eller
restskattetillæg. Resultatets specifikation bruges, ikke oversigtens
afrundede helkronebeløb.

| Profil | Beskæftigelsesfradrag | Jobfradrag | Ekstra pensionsfradrag | Skattepligtig indkomst | Kommuneskat | Samlet skat |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| ATP alene, lav løn | 12669 | 0 | 332 | 78999 | 1856476 | 1929080 |
| Livsvarig livrente og ATP | 55600 | 2900 | 5852 | 487648 | 11459728 | 21056932 |
| Ratepension alene | 55600 | 2900 | 0 | 493500 | 11597250 | 21194454 |
| Ratepension og ATP | 55600 | 2900 | 332 | 493168 | 11589448 | 21186652 |

De to første profiler matcher den kanoniske model. ATP-profilens lave løn
holder beskæftigelsesfradraget under loftet og undersøger dermed mere end
§ 9 L alene. Det er ikke bevis for korrekt behandling af alle ATP-typer.

## De to afvigelser er åbne

| Profil | Model: ekstra pensionsfradrag | Offentlig formular: ekstra pensionsfradrag | Model: samlet skat, øre | Offentlig formular: samlet skat, øre |
| --- | ---: | ---: | ---: | ---: |
| Ratepension alene | 5520 | 0 | 21064734 | 21194454 |
| Ratepension og ATP | 5852 | 332 | 21056932 | 21186652 |

I begge tilfælde er forskellen 5.520 kr. i fradrag og 1.297,20 kr. i skat.
Det stemmer aritmetisk med forskellen fra ratebidragets § 9 L-del i disse
profiler, men identificerer ikke årsagen i den offentlige beregner.
Man må **ikke** flytte ratebidraget til `PRATPA`, ændre dokumenterede fakta
eller bruge en tolerance for at erklære konformitet.

Den kanoniske `vurdering.forbehold` og den danske resultatviser oplyser nu
denne begrænsning. Gyldighed, inputtyper og skatteformler er uændrede.
Et beløb mærket »beregnet med forbehold« er ikke en godkendt årsopgørelse,
og en forskel her er ikke en anbefaling om at rette sin indberetning.

## Reproduktion

[Regressionen](../../tests/tax_employer_pension_conformance.test.mjs) bruger
faste eksterne observationer, ikke netværk eller forventninger udledt af
Futurunas output. Den skelner udtrykkeligt mellem **to match og to åbne
afvigelser**. En femte, rent modelbaseret kontrol bevarer ukendt ATP som
ugyldigt grundlag uden sammenligningsbeløb. Den danske visning skal bevare
det nye forbehold, også i den blandede batch.

```sh
FUTURUNA_MODEL_TEST_RUNA="$RUNA_BIN" node --test tests/tax_employer_pension_conformance.test.mjs
```

En bestået regression betyder, at disse forventninger og synlige
begrænsninger bevares. Den betyder ikke fire konforme profiler, fuld
Personskat-dækning, korrekt AI-læsning af vilkårlige bilag eller verificeret
officiel adfærd i andre år.
