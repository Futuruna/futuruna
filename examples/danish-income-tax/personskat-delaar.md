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
valgte private mappe. Fra projektets rod:

```sh
"$RUNA_BIN" schema examples/danish-income-tax/personskat-par14.calculate.runa --entry beregn_personskat_delår --format compact-json --output PRIVATE_WORK_DIR/delaar-schema.json
"$RUNA_BIN" template examples/danish-income-tax/personskat-par14.calculate.runa --entry beregn_personskat_delår --format json --output PRIVATE_WORK_DIR/delaar-cases.json
# Udfyld og gennemgå kildefakta, før der beregnes.
FUTURUNA_CALCULATION_JOBS=1 "$RUNA_BIN" call examples/danish-income-tax/personskat-par14.calculate.runa --entry beregn_personskat_delår --input PRIVATE_WORK_DIR/delaar-cases.json --output PRIVATE_WORK_DIR/delaar-results.json
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
  helårsindkomst må manglende `faktisk_helårsbeløb_kroner` ikke blive til nul.
- `helårsgrundlag`: afledte identificerede kilder eller et gennemgået fuldt
  Personskat-grundlag. Genbrug ikke delårsbeløbene som hele årets fakta uden
  grundlag, og gæt ikke manglende ægtefælleoplysninger.

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

Et fiktivt tilflytningsforløb i 2025 med løn 200.000/400.000 kr., egen
kapitalindkomst +20.000/+40.000 kr. og ægtefællens kapitalindkomst
−10.000/−15.000 kr. i henholdsvis delårs-/helårsgrundlaget illustrerer fejlen:
bundskattebrøken er 194.000/393.000 efter modregning, ikke 204.000/408.000.
Den rettede modelskat er 72.286,83 kr. mod tidligere 72.547,66 kr.
[Kontrollen](../../tests/personskat_partyear_capital.test.mjs) angiver resten
af de fiktive fakta og kontrollerer også § 11-grundlag samt to uændrede
kontrolforløb. Dette er en kildebaseret modelkontrol, ikke en uafhængig
delårsberegning fra SKAT.

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
  arbejdsudlejebeskatning. Betalingsafregningen skal vurderes særskilt.

`samlet_modeldækning_bekræftet` er fortsat `false`. Alle generelle og
delårsspecifikke forbehold skal bevares. En vellykket CLI-kørsel eller denne
vurdering autentificerer ikke dokumenterne og beviser ikke fuld lovdækning.
Den almindelige `personskat-resultat.mjs`-viser understøtter ikke denne
separate resultatkontrakt; AI'en skal læse det gemte delårs-JSON direkte.

Et **fiktivt modelkontroltilfælde**, kørt den 25. september 2026: fuld
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

## Eksisterende input

Vurderingen og inputvejledningen ændrer kontraktens fingeraftryk.
Generér en frisk delårsskabelon og overfør de samme gennemgåede kildefakta;
ret ikke kun `schema_hash`. Vurderingsgrænsen alene ændrede ikke skatteformler
eller rå beløb. Den efterfølgende kapitalrettelse ovenfor ændrer derimod
berørte delårsresultater og § 11-forklaringer. Aktierettelsen ændrer også
berørt aktieskat, og modstridende ægtefællegrundlag kan nu afvises.
Genberegn gemte resultater uden
at rette kildebeløbene. Inputtyper og satser er uændrede. Tidligere ugyldige
nulbeløb bliver ikke godkendte resultater ved migreringen.
