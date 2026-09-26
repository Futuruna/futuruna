# Arbejdsgiverpension og ATP: ekstern kontrol, 2025

Den 25. september 2026 blev seks **fiktive** profiler beregnet gennem
[SKATs anonyme årsberegner for 2025](https://www.tastselv.skat.dk/borger/beregn2025/profil.do).
To profiler matcher modellen frem til samlet skat i øre. Fire profiler med
arbejdsgiveradministreret ratepension gør **ikke**. Afvigelserne er ikke
afrundingsforskelle og er ikke løst ved at ændre input eller skatteformler.
To yderligere indtastninger blev afvist af formularens beløbsgrænse og gav
ingen skattespecifikation; de er ikke numeriske sammenligninger.

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
Samme lovs § 9 J omfatter relevante arbejdsgiverindbetalinger i grundlaget
for beskæftigelsesfradrag. Den nye lavtlønsprofil undersøger også denne del,
som de første rateprofiler ikke kunne skelne, fordi de nåede fradragsloftet.

Der var ingen tilgængelig browserforbindelse ved denne kontrol. Friske,
anonyme HTTP-sessioner fulgte den offentlige formulars faktiske felter og
handlinger: `profil.do` → `indberet.do` → `kvit.do` → `spec.do`.
Specifikationen blev åbnet med formularens `person=hop`. Tilbagefunktionen
bekræftede de indsendte beløb uændret. Ingen login, CPR-numre eller private
dokumenter indgik. HTML, sessionsoplysninger og forsøgsprogrammet opbevares
uden for repoet; regressionen nedenfor kører offline.

Den undersøgte `doSubmitBeregn()` sender til `kvit.do`; dens
`check_send_side()` kontrollerer sidens indlæsning og ændrer ikke pensionsbeløb.
Der blev ikke fundet en pensionsomfordeling i de undersøgte submit- og
valideringsfunktioner. Det er en begrænset inspektion, ikke bevis for, at
hele browserforløbet er ækvivalent med HTTP-forsøget. En HTTP-succes eller
forsøgsprogrammets afslutning uden fejl er heller ikke bevis for et
beregningsresultat: de to beløbsafvisninger nedenfor blev identificeret
ved læsning af den returnerede fejlside.

En særskilt kontrol den **26. september 2026** undersøgte formularens
paneltilstand. De viste JavaScript-handlinger sætter `diverseKnap=true`, når
pensionspanelet åbnes, og `persmaKnap=true`, når lønpanelet åbnes. Et nyt
anonymt HTTP-forløb satte begge felter til `true`, men beholdt lavtlønsprofilens
fakta: løn 100.000 kr., `PRA=46000`, ingen ATP eller privat pension. Resultatet
var fortsat beskæftigelsesfradrag 12.300 kr., ingen ekstra pensionsfradragslinje,
skattepligtig indkomst 79.700 kr. og beregnet skat 19.455,54 kr. Tilbagefunktionen
bevarede både beløbene og de to `true`-værdier. Åbne paneler fjernede altså ikke
afvigelsen i dette forsøg; kontrollen fastslår ikke alle browserens handlinger
eller serverens interne feltfortolkning. Det er samme økonomiske profil, ikke
en syvende uafhængig skatteprofil eller et nyt match.

Det er **ikke** en ny afprøvning af den grafiske browsers samlede forløb.
Beregnerens versionsnummer blev ikke registreret. Årsagen til afvigelserne,
herunder den præcise feltfortolkning og forskelle fra en virkelig
årsopgørelse, er fortsat åben (`td-0e5d15`).

## Fiktive kildefakta og felter

Alle profiler: 2025, født 1. januar 1990, enlig, København (101), ingen
kirkeskat, børn, virksomhed, ejendom eller udenlandske forhold. Fuld dansk
skattepligt og DBO-hjemsted hele året. Ingen privat pension bortset fra den
udtrykkelige kombinationsprofil nedenfor; ingen udbetalinger fra pension i
2024/2025, andre indkomster eller fradrag. Beløbsfelter uden for
profilen var blanke. Det udtrykkelige fravær gælder kun disse opdigtede
profiler; ukendte forhold i virkelige dokumenter er ikke nul.

Løn er allerede efter medarbejderens bidrag til arbejdsgiverpension/ATP,
men før løn-AM og skat. Hvor pension forekommer, er 50.000 kr. brutto og
4.000 kr. indeholdt AM selvstændigt valgte kildefakta; netto er 46.000 kr.
ATP er særskilt 3.000 kr. brutto og 2.760 kr. dokumenteret netto. Beløbene
er illustrative, ikke en udledning af en bestemt ATP-sats. Ratepensionen
har almindelig bortseelsesret og er under årsgrænsen; livrenteprofilen er
en **anden** ordning, ikke en omklassifikation for at få et match.

| Profil | Løn, rubrik 11 | Arbejdsgiverrate efter AM (`PRA`) | Øvrige arbejdsgiverordninger (`PRATPA`) | Privat rate, rubrik 21 |
| --- | ---: | ---: | ---: | ---: |
| ATP alene, lav løn | 100000 | blank | 2760 | blank |
| Livsvarig livrente og ATP | 600000 | blank | 48760 | blank |
| Ratepension alene | 600000 | 46000 | blank | blank |
| Ratepension og ATP, som bilagsdemoen | 600000 | 46000 | 2760 | blank |
| Ratepension, lav løn | 100000 | 46000 | blank | blank |
| Arbejdsgiver- og privat rate, ved fælles loft | 600000 | 46000 | blank | 19500 |

Kombinationsprofilens private betaling på 19.500 kr. er en særskilt
opdigtet egenindbetaling uden AM. Den er ikke en medarbejderandel af de
50.000 kr. i arbejdsgiverordningen. Sammen med arbejdsgiverens netto på
46.000 kr. når den 2025-loftet på 65.500 kr., men overskrider det ikke.

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
| Ratepension, lav løn | 12300 | 0 | 0 | 79700 | 1872950 | 1945554 |
| Arbejdsgiver- og privat rate, ved fælles loft | 55600 | 2900 | 2340 | 471660 | 11084010 | 20447019 |

De to første profiler matcher den kanoniske model. ATP-profilens lave løn
holder beskæftigelsesfradraget under loftet og undersøger dermed mere end
§ 9 L alene. Det er ikke bevis for korrekt behandling af alle ATP-typer.

## De fire afvigelser er åbne

| Profil | Model: ekstra pensionsfradrag | Offentlig formular: ekstra pensionsfradrag | Model: samlet skat, øre | Offentlig formular: samlet skat, øre |
| --- | ---: | ---: | ---: | ---: |
| Ratepension alene | 5520 | 0 | 21064734 | 21194454 |
| Ratepension og ATP | 5852 | 332 | 21056932 | 21186652 |
| Ratepension, lav løn | 5520 | 0 | 1671309 | 1945554 |
| Arbejdsgiver- og privat rate, ved fælles loft | 7860 | 2340 | 20317299 | 20447019 |

Alle fire har en forskel på 5.520 kr. i ekstra pensionsfradrag. Ved løn
600.000 kr. er skatteforskellen 1.297,20 kr., aritmetisk svarende til denne
§ 9 L-del. Lavtlønsprofilen har derudover beskæftigelsesfradrag 18.450 kr.
i modellen mod formularens 12.300 kr. Modellen bruger arbejdsgrundlag
150.000 kr. fra løn 100.000 og dokumenteret bruttopension 50.000; formularens
resultat svarer her til kun lønnen. Den samlede skatteforskel er derfor
2.742,45 kr. Begge fradrag ligger under beskæftigelsesfradragets loft.

Kombinationsprofilen fratrækker privat rate 19.500 kr. i personlig indkomst
i både model og formular. Formularens ekstra pensionsfradrag på 2.340 kr.
svarer til 12 % af den private del alene; modellens 7.860 kr. svarer til
12 % af 65.500 kr. Dette beskriver de observerede beløb, ikke beregnerens
interne implementering eller en forklaring på dens feltfortolkning.
Man må **ikke** flytte ratebidraget til `PRATPA`, ændre dokumenterede fakta
eller bruge en tolerance for at erklære konformitet.

Den kanoniske `vurdering.forbehold` og den danske resultatviser oplyser nu
denne begrænsning. Gyldighed, inputtyper og skatteformler er uændrede.
Et beløb mærket »beregnet med forbehold« er ikke en godkendt årsopgørelse,
og en forskel her er ikke en anbefaling om at rette sin indberetning.

## Formularafvisning er ikke et skatteresultat

To yderligere indtastninger havde samme grundprofil og løn 600.000 kr.,
men overskred formularens fælles grænse:

| Forsøg | `PRA` efter AM | Privat rate, rubrik 21 | Returneret status |
| --- | ---: | ---: | --- |
| Arbejdsgiverrate over loft | 69000 | blank | Fejlside: summen overstiger 65.500 kr. |
| Arbejdsgiver- og privat rate over loft | 46000 | 40000 | Samme fejlside |

`PRATPA` var blankt. Ingen af forsøgene gav en skattespecifikation.
Der er derfor **intet observeret skattebeløb**, heller ikke nul, og ingen
ekstern verifikation af modellens behandling af overskydende indbetalinger.
De afviste profilbeløb bruges ikke som kildefakta til de seks beregnede
profiler ovenfor.

Afvisningerne viser, at serveren modtager og bruger `PRA` i kontrollen af
det fælles loft. Sammen med de genviste input indsnævrer det fejlsøgningen:
en generelt tabt indtastning forklarer ikke forløbet. Det afgør ikke,
hvorfor beløbet ikke ses i de to undersøgte fradrag. Grafisk reproduktion
og dokumentation af feltfortolkningen mangler fortsat; ingen ændring af
lovmodellen er begrundet alene af disse observationer.

## Reproduktion

[Regressionen](../../tests/tax_employer_pension_conformance.test.mjs) bruger
faste eksterne observationer, ikke netværk eller forventninger udledt af
Futurunas output. Den skelner udtrykkeligt mellem **to match og fire åbne
afvigelser**. En syvende, rent modelbaseret kontrol bevarer ukendt ATP som
ugyldigt grundlag uden sammenligningsbeløb. Den danske visning skal bevare
forbeholdet om både pensions- og beskæftigelsesfradrag, også i den blandede
batch. Formularafvisningerne er dokumenterede observationer ovenfor, ikke
numeriske cases eller nye HTTP-kald i regressionen.

```sh
FUTURUNA_MODEL_TEST_RUNA="$RUNA_BIN" node --test tests/tax_employer_pension_conformance.test.mjs
```

En bestået regression betyder, at disse forventninger og synlige
begrænsninger bevares. Den betyder ikke seks konforme profiler, fuld
Personskat-dækning, korrekt AI-læsning af vilkårlige bilag eller verificeret
officiel adfærd i andre år.
