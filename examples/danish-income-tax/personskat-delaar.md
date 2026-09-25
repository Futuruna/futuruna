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

Den tilføjede vurdering og inputvejledning ændrer kontraktens fingeraftryk.
Generér en frisk delårsskabelon og overfør de samme gennemgåede kildefakta;
ret ikke kun `schema_hash`. De eksisterende skatteformler, gyldighedsvilkår
og rå beløb er uændrede. Tidligere ugyldige nulbeløb bliver ikke godkendte
resultater ved migreringen.
