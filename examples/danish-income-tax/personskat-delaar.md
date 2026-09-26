# Skattepligtig en del af året: vælg den rigtige beregning

Spørg først, om **dansk skattepligt** indtrådte eller ophørte i indkomståret.
At begynde et job i juli eller kun få løn i seks måneder betyder ikke i sig
selv delårsskattepligt. En flyttedato er heller ikke alene dokumentation for
den juridiske skattepligtsperiode. Afklar kildefakta og fuld/begrænset
skattepligt, før der vælges beregningsvej — også for en ægtefælle.

Den almindelige `beregn_personskat` modtager ikke skattepligtsperiodens datoer
og anvender ikke automatisk PSL § 14. Et gyldigt resultat dér beviser derfor
ikke, at det er den rigtige indgang til en til- eller fraflytningssag. Mangler
den nødvendige afklaring, skal den uafhængige skattesammenligning afvente den;
[betinget rapportafstemning](aarsopgoerelse-afstemning.md) kan stadig undersøge
snævrere spørgsmål uden at bekræfte periodisering eller fuld retlig korrekthed.

## Reglen og modellens indgang

Ved fuld skattepligt en del af året er udgangspunktet helårsomregning og
efterfølgende forholdsmæssig nedsættelse af den beregnede skat. Der findes et
valg af faktisk helårsindkomst efter PSL § 14, stk. 2. Dette valg kræver også
oplysninger om indkomst uden for den danske skattepligtsperiode. Omregningen
skal give et retvisende helårsgrundlag; alle beløb skal ikke mekanisk ganges
med samme dagbrøk. Se [Den juridiske vejledning C.F.1.6.2.1](https://info.skat.dk/data.aspx?oid=1977388)
og [indtægts- og udgiftstyper](https://info.skat.dk/data.aspx?oid=1977389).

Futurunas særskilte indgang er `beregn_personskat_delår` i
[personskat-par14.calculate.runa](personskat-par14.calculate.runa). Den
indeholder kildehenvisninger, årsbetingelser og særskilt klassifikation af
løbende beløb, engangsbeløb og dokumenterede retvisende helårsbeløb.
Begrænset skattepligt kræver sit relevante personfradragsgrundlag og valg;
brug ikke kategorien fuld skattepligt for at få beregningen til at køre.

Indgangen er en forskningsmodel med Preview-kontrakt, ikke individuel
skatterådgivning eller en automatisk afgørelse af skattepligt. Den modellerer
én sammenhængende periode med indtræden eller ophør og den relevante
årsbetingelse. Flere skattepligtsperioder eller skift mellem fuld og begrænset
skattepligt må ikke presses ind i én periode. Ægtefælle-/underskudsforløb kan
kræve et særskilt dokumenteret helårsgrundlag; en gyldig simpel lønsag beviser
ikke dækning af sådanne sammensatte sager.

Kirkemedlemskab en del af året er ikke det samme som delårsskattepligt.
Denne indgang tilføjer ingen medlemsperioder; PSL § 14 må ikke bruges til
at efterligne en ind-/udmeldelse. Afklar også kirkeskatten for året og
en eventuel ægtefælle; se [kirkeskattegrænsen](personskat-validity.md#kirkeskat-gælder-indkomståret-ikke-status-i-dag).
`KirkeskatUoplyst` og `KirkeskatEnDelAfÅret` i Personskat-grundlaget
tilbageholder også den yderste delårssammenligning. Det gælder både
delårsgrundlaget og et særskilt dokumenteret helårsgrundlag.

## Fra gennemgåede fakta til input

Brug den compiler, der bestod [runtime-tjekket](../../website/public/ai-setup.md#tax-audit-runtime-check),
og gem personlige filer uden for projektet. Erstat `PRIVATE_WORK_DIR` med den
valgte private mappe. Visningskommandoen er valgfri og kræver allerede
installeret Node.js 18 eller nyere. Fra projektets rod:

```sh
"$RUNA_BIN" schema examples/danish-income-tax/personskat-par14.calculate.runa --entry beregn_personskat_delår --format compact-json --output PRIVATE_WORK_DIR/delaar-schema.json
"$RUNA_BIN" template examples/danish-income-tax/personskat-par14.calculate.runa --entry beregn_personskat_delår --format json --output PRIVATE_WORK_DIR/delaar-cases.json
# Udfyld og gennemgå kildefakta, før der beregnes.
FUTURUNA_CALCULATION_JOBS=1 "$RUNA_BIN" call examples/danish-income-tax/personskat-par14.calculate.runa --entry beregn_personskat_delår --input PRIVATE_WORK_DIR/delaar-cases.json --output PRIVATE_WORK_DIR/delaar-results.json
node examples/danish-income-tax/personskat-resultat.mjs PRIVATE_WORK_DIR/delaar-results.json
```

Skabelonværdier er ikke personfakta. Følg de genererede spørgsmål og bevar
kildereferencer og uafklarede forhold:

- `personskat`: kildegrundlaget for den danske skattepligtsperiode. En
  fuld kalenderårsindkomst må ikke samtidig genbruges som periodens løn.
- `skattepligtsændring` og `skattepligtsperiode`: det afklarede juridiske
  forløb, ikke ansættelsesdatoer eller rapportens udskriftsdato.
- `kilder`: identificerede delårsbeløb med deres relevante beregningsfelt
  og kildeunderbyggede omregningsmetode. Afledte fradrags-/pensionsfelter skal
  afstemme modellen; beløb må ikke flyttes for at frembringe en ønsket skat.
- `valg_afgivet_ved_oplysninger` og `omvalg_dato`: dokumenterede valg, ikke
  en antagelse om hvilken metode der giver lavest skat. Ved faktisk
  helårsindkomst må ukendte beløb ikke blive til nul. Kun for aktieindkomst
  kan det gentagne årsbeløb udelades, fordi kildebeløbet bevares uændret.
- `helårsgrundlag`: afledte identificerede kilder eller et gennemgået fuldt
  Personskat-grundlag. Genbrug ikke delårsbeløbene som hele årets fakta uden
  grundlag, og gæt ikke manglende ægtefælleoplysninger.

## Samme person og fødselsdato i begge grundlag

PSL § 14 omregner indkomst for en person, ikke personens fødselsdato. Ved
`DokumenteretHelårsPersonskat` skal hovedpersonens dato under
`lønmodtager.pension.fødselsdato` derfor være ens i delårs- og helårsgrundlaget.
Det gælder hele datoen, også dag og måned, selv om forskellen ikke ændrer
skatten i det konkrete år. En aktiv ægtefælles dato skal tilsvarende stemme
med **den samme ægtefælles** dato i det andet grundlag. Ægtefællerne skal ikke
have samme dato som hinanden, og deres indkomstbeløb kan være forskellige
mellem delårs- og helårsgrundlag.

Dette er en konsistenskontrol ved sammensætningen af de to beregninger efter
[§ 14](https://www.retsinformation.dk/eli/lta/2021/1284/pdf) og
[helårsvejledningen](https://info.skat.dk/data.aspx?oid=1977388), ikke en ny
skatteregel eller kontrol af identitet. Modstridende datoer tilbageholder
den yderste sammenligning med en forklaring under `helårsgrundlag`.
Futuruna retter ikke datoerne og vælger ikke den skattemæssigt gunstigste.
Afklar person og dato mod kilden; to ens, men forkerte datoer kan stadig
bestå denne kontrol.

[Regressionskontrollen](../../tests/personskat_partyear_birth_dates.test.mjs)
indeholder fiktive konsistente og modstridende grundlag samt kontrol af
feltvejledningen. Den er ikke en uafhængig SKAT-beregning eller et facit
for virkelige bilag.

## Samme årsforhold for ægtefællen

I `DokumenteretHelårsPersonskat` skal den tilsvarende ægtefælles
`lønmodtager.kommune`, `lønmodtager.kirkeskat` og
`lønmodtager.personfradrag_alder_status` også stemme med delårsgrundlaget.
Helårsomregningen ændrer indkomstbeløb, ikke disse årsoplysninger.
Sammenlign samme person mellem grundlagene; ægtefællerne kan have forskellige
skattekommuner og kirkeskatteforhold fra hinanden.

Kommune er årets **skattekommune**, ikke nødvendigvis den nuværende bopæl.
Kommuneskattelovens [§ 2](https://www.retsinformation.dk/eli/lta/2019/935/pdf)
har regler om kalenderårets skattekommune, herunder ved skattepligtens indtræden.
Denne kontrol afgør ikke skattekommunen ud fra en flyttehistorik. Kirkeskat
er den samme oplysning om hele indkomståret i begge grundlag; en faktisk
[medlemsændring](https://www.borger.dk/kultur-og-fritid/medlemskab-af-folkekirken)
må ikke skjules ved at vælge to forskellige helårsstatusser. Medlemsperioder
er fortsat uden for denne beregningsvej.

Modstridende årsforhold tilbageholder sammenligningen under `helårsgrundlag`,
mens kilderne bevares. Kontrollen udleder ikke alder/civilstand, autentificerer ikke
bilag og beviser ikke, at to ens oplysninger er korrekte. Afklar kilderne;
kopiér ikke en værdi blot for at bestå. De
[fiktive kontroltilfælde](../../tests/personskat_partyear_spouse_settings.test.mjs)
holder konsistens mellem samme persons grundlag adskilt fra tilladte
forskelle mellem ægtefællerne.

## Ægtefællers kapitalindkomst i de to grundlag

Bevar hver persons egen kapitalindkomst i både delårsgrundlaget og det
dokumenterede helårsgrundlag. Indtast ikke et allerede ægtefællemodregnet
beløb som egne renter. Modellen skal selv anvende PSL § 6, stk. 3, før
bundskatten og dens delårs-/helårsbrøk beregnes. Den anden persons negative
kapitalindkomst kan være forskellig i de to grundlag; genbrug ikke automatisk
delårsbeløbet i helårsberegningen, og udled ikke beløbet af den forventede skat.

Tilsvarende skal § 11-grundlaget bevare den relevante ægtefælles positive
kapitalindkomst og den fælles beløbsgrænse. De viste beregnede nedslag er ikke
altid de anvendte nedslag: egen skat kan begrænse anvendelsen, og et uudnyttet
nedslag kan komme fra ægtefællen. Sammenlign derfor ikke de to felter som en
generel lighedskontrol.

Dette følger modellens sammensætning af
[PSL §§ 6, 11 og 14](https://www.retsinformation.dk/eli/lta/2021/1284)
med særskilt kildeunderbygget helårsomregning. Det er ikke en automatisk
afgørelse af skattemæssigt samliv eller dokumentation for alle parforløb.
Ægtefællefakta kræver fortsat det dokumenterede helårsgrundlag; den afledte
enkeltpersonsvej gætter dem ikke.

[Kontrollen](../../tests/personskat_partyear_capital.test.mjs) angiver fiktive
delårs-/helårsfakta og kontrollerer bundskattebrøk, § 11-grundlag samt
enkeltpersons- og samlivsgrænser. Det er en kildebaseret modelkontrol,
ikke en uafhængig delårsberegning fra SKAT.

## Aktieindkomst: kildebeløb og skattegrundlag er forskellige

`Par14Aktieindkomst` i kildelisten skal afstemme personens egen beregnede
aktieindkomst **før PSL § 8 a-ægtefællemodregning**. Er egen aktieindkomst
100.000 kr. og ægtefællens −40.000 kr., er kildens beløb stadig 100.000 kr.,
selv om det positive skattegrundlag efter modregning er 60.000 kr. Registrér
ægtefællens egne dokumenterede aktiefakta særskilt; opfind ikke et tab ud fra
skatten på årsopgørelsen. Kildeartsbegrænsede tab efter ABL § 13 A er ikke
automatisk negativ aktieindkomst efter PSL § 8 a.

Skatteberegningen bruger derefter hvert grundlags kanoniske ægtefællemodregning
og overførsel af uudnyttet grundbeløb efter
[PSL § 8 a, stk. 4 og 6](https://www.retsinformation.dk/eli/lta/2021/1284).
Eksempelvis giver 100.000 kr. positiv aktieindkomst og en kvalificerende
ægtefælle uden aktieindkomst 27.000 kr. aktieskat i 2025, før eventuelle andre
reduktioner og kreditter. Se [SKATs satser og tabsvejledning](https://skat.dk/borger/aktier-og-andre-vaerdipapirer/skat-af-aktier).
Aktieindkomst, der er et engangsbeløb i skattepligtsperioden, indgår uændret i
helårsgrundlaget; opskalér ikke automatisk efter antal dage. Se
[C.F.1.6.2.2](https://info.skat.dk/data.aspx?oid=1977389).

### Aktieindkomst skal ikke omregnes som løn

PSL § 14, stk. 1–3, omhandler almindelig skattepligtig indkomst, personlig
indkomst og kapitalindkomst. Egen aktieindkomst er en særskilt indkomstform
efter § 4 a og må ikke få et højere hypotetisk årsbeløb via dagomregning.
Hver `Par14Aktieindkomst`-kilde skal derfor bevare sit delårsbeløb i det
valgte beregningsgrundlag. Brug `Par14UændretEngangsbeløb`; et dokumenteret
helårsbeløb kan også bruges, når det er samme beløb. Kontrollen gælder hver
kilde, så modsatrettede omregninger ikke kan skjule hinanden i en sum.

Ved valget efter § 14, stk. 2, kan `faktisk_helårsbeløb_kroner` være `null`
for `Par14Aktieindkomst`: modellen bruger det allerede oplyste delårsbeløb
uændret. Oplyses et årsbeløb, skal det være samme beløb; et udtrykkeligt nul
er ikke en tom værdi. For alle andre kildearter er det faktiske årsbeløb
fortsat påkrævet. Dette er ikke en antagelse om ukendt indkomst, og det
tilføjer ikke udenlandske aktieindtægter uden for skattepligtsperioden.
Valget af faktisk indkomst udvider ikke i sig selv den danske skattepligt
for aktier. Er sådanne beløb faktisk relevante, skal deres skattepligt og
kilder afklares særskilt; de må ikke presses ind i denne omregningsrække.
Bevar dokumenterne og ret ikke faktiske beløb for at få input accepteret.
En uforenelig omregning tilbageholder sammenligningen med en forklaring
under `kilder`; modellen retter ikke automatisk input.

[Regressionskontrollen](../../tests/personskat_partyear_share_conversion.test.mjs)
beskriver alle fiktive fakta og afprøver afledte og dokumenterede grundlag,
valg af faktisk indkomst samt modsatrettede kildeomregninger. Det er ikke
en uafhængigt observeret SKAT-beregning.

For et dokumenteret helårsgrundlag sammenlignes den rekonstruerede lave
aktieskat også med det kanoniske helårsresultats egen aktieskat. Det er en
intern sammensætningskontrol, ikke et uafhængigt administrativt facit.
Dette afgør ikke alle ægtefælleforløb, udenlandske aktiers skattepligt eller
fraflytningsbeskatning.

Samliv **ved indkomstårets udløb** er én og samme oplysning i delårs- og
helårsgrundlaget. Modstridende ja/nej, en ægtefælle der mangler i det ene
grundlag, eller en aktiv ægtefælles forkerte skatteår afviser sammenligningen.
Det forbyder ikke ændret civilstand i løbet af året: feltet spørger ikke om
samliv på periodens første dag. Kontrollen beviser heller ikke personernes
identitet eller de juridiske betingelser for skattemæssigt samliv.

[Den fokuserede regression](../../tests/personskat_partyear_shares.test.mjs)
omfatter to grundbeløbsoverførsler, delvis og fuld modregning af et dokumenteret
unoteret aktietab, to kontrolforløb og fire modstridende ægtefællegrundlag.
Det er kildebaserede modelkontroller, ikke uafhængige administrative delårsresultater.

## Læs den yderste vurdering

Brug **resultatets egen** `vurdering.slutskat_til_sammenligning_øre`.
`delårsresultat.vurdering` tilhører et indlejret mellemtrin; dets gyldighed
eller beløb er ikke den endelige delårskonklusion. Heller ikke
`helårsberegning` er beløbet, der skal sammenlignes med delårsskatten.

- `UgyldigtBeregningsgrundlag`: sammenligningsbeløbet er `null`. Læs
  `vurdering.fejl` med inputstier og forklaringer. Øvrige beløb er kun
  diagnostik, også når `slutskat_efter_par14_øre` viser nul.
- `BeregnetMedForbehold`: de eksisterende delårskontroller er bestået.
  Sammenligningsbeløbet svarer til `slutskat_efter_par14_øre`; det er ikke
  restskat, tilbagebetaling eller det særskilte beløb inklusive endelig
  arbejdsudlejebeskatning. En valgt betalingsafregning skal også bestå
  kontrollerne mod den endelige delårsskat. Uden valgt afregning er et beløb
  til udbetaling eller opkrævning ikke fastlagt.

`samlet_modeldækning_bekræftet` er fortsat `false`. Alle generelle og
delårsspecifikke forbehold skal bevares. En vellykket CLI-kørsel eller denne
vurdering autentificerer ikke dokumenterne og beviser ikke fuld lovdækning.
`personskat-resultat.mjs` understøtter det gemte delårsresultat og bruger kun
den yderste vurdering til sammenligningsbeløbet. Ved et gyldigt grundlag vises
også skattepligtsdage, valgt metode og udvalgte indkomstgrundlag i perioden
og årsberegningen. Disse grundlag er mellemregninger, ikke årsopgørelsens
endelige rubrikbeløb. Indlejrede skattetotaler vises ikke som slutskat.
Ugyldige sager viser kontroller og forbehold, men ingen af disse beløb.
Viseren genberegner ikke skatten, afklarer ikke skattepligt og ændrer ingen
filer; den fulde detailberegning og betalingsafregning skal læses i JSON.
Se [visningens grænser og exitkoder](personskat-validity.md#læs-dit-gemte-resultat-på-dansk).

Et **fiktivt modelkontroltilfælde**: fuld
skattepligt 1. januar–30. juni 2025, løn 300.000 kr. i perioden, født
1. januar 1990, København, ingen kirkeskat, ægtefælle, ATP, pension,
anden indkomst, fradragsudgifter, ejendom eller fremførte tab. Den fiktive
løns løbende helårsbehandling er udtrykkeligt oplyst, og udbetalingshistorik
er gennemgået tom. Resultatet viser **103.831,01 kr.** efter § 14, mens det
indlejrede almindelige resultat viser **94.331,44 kr.** At vælge det forkerte
niveau ville her ændre det viste beløb med 9.499,57 kr.

Ændres kildens delårsbeløb fejlagtigt til 299.999 kr., mens Personskat-lønnen
stadig er 300.000 kr., er delårsresultatet ugyldigt. Det indlejrede almindelige
resultat er fortsat beregneligt, men den yderste vurdering tilbageholder
sammenligningen. [Regressionskontrollen](../../tests/personskat_partyear_validity.test.mjs)
dækker denne forskel. Tallene er modelobservationer, ikke en uafhængig
kontrol mod SKAT eller et eksempel, der må genbruges som personlige fakta.

## Afregning efter den endelige delårsskat

`vurdering.kontrolgrundlag.beregning` indeholder delårets kilde- og
beregningskontroller. `afregning` indeholder kontrollen af den afsluttende
betalingsafregning. `kontroller` og `fejl` bevarer det samlede overblik.
`input_gyldigt` og den yderste vurdering kræver begge et gyldigt
beregningsgrundlag og en gyldig valgt afregning.

Et ordinært mellemtrin kan have en anden betalingsretning end den endelige
delårsberegning. Dets afregningsfejl må derfor hverken afvise et gyldigt
delårsresultat eller erstatte den endelige kontrol. Kildekontrollerne fra både
perioden og det dokumenterede helårsgrundlag bevares derimod, også når deres
sti hedder `årsopgørelse`.

Et særskilt fiktivt kontroltilfælde bruger perioden ovenfor, men **dokumenteret
helårsløn på 600.000 kr.**, ikke den løbende dagfaktor. Modellens slutskat er
103.752,24 kr. Med fiktivt indeholdt skat/AM på 100.000 kr. er det ordinære
mellemtrin en tilbagebetaling, mens det endelige resultat har 3.752,24 kr. i
restskat før tillæg. En valgt restskatafregning for 2025 accepteres; en
tilbagebetalingsinstruks eller afregningsår 2024 tilbageholder sammenligningen.
Den rå skat ændres ikke for at få instrukserne til at passe.

[Regressionen](../../tests/personskat_settlement_stage.test.mjs) dækker også
ugyldige betalingsfakta, en gyldig tilbagebetaling, nulbalance uden
betalingsinstruks og kildefejl i begge grundlag. Det er kontrol af den
eksisterende KSL-models sammensætning efter § 14. Modregning og sondringen
mellem restskat og overskydende skat følger de særskilte trin i
[KSL §§ 60–62](https://www.retsinformation.dk/eli/lta/2024/460/pdf).
Kontrollen er ikke ny dokumentkontrol, individuel rådgivning eller fuld
dækning af alle afregningsregler og år.

## Eksisterende input

Generér en frisk delårsskabelon fra den valgte modelversion og overfør
gennemgåede kildefakta; ret ikke `schema_hash` for at omgå en kontraktforskel.
Genberegn resultaterne uden at ændre kildebeløb for at opnå accept. De aktuelle
[kompatibilitetsnoter](../../docs/compatibility-guides/0.2.x.md) beskriver
migration, når en gemt kontrakt ikke længere passer. Et ugyldigt nulbeløb
bliver ikke et godkendt resultat ved migreringen.
