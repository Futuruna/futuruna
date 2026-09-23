# Stemmer mit beskæftigelsesfradrag ved udenlandsk arbejde?

Den [afgrænsede beregning](beskaeftigelsesfradrag.calculate.runa) kontrollerer
almindeligt beskæftigelsesfradrag og jobfradrag for 2023–2026. Den viser, hvilke
grundlagsbeløb der bevares eller udelukkes, og giver ingen sammenlignelige
fradragsbeløb, hvis nødvendige fakta er uafklarede. Det er en forskningsmodel,
ikke individuel skatterådgivning. `schema`, `template` og `call` er Preview.

Den samlede Personskat-beregning er **endnu ikke tilsluttet denne fordeling**.
Brug ikke dens uændrede slutskat som en fuld audit af en sag, hvor undtagelsen
kan gælde. Denne beregning er heller ikke en beregning af skat, udenlandsk
skattenedslag eller værdien af et ekstra fradrag.

## Hvorfor ikke bare »udenlandsk indkomst: ja/nej«?

LL § 9 J, stk. 1, kræver en bestemt kombination: DBO-hjemsted i en fremmed
stat, Grønland eller Færøerne og bidragspligtig indkomst fra arbejde udført
i udlandet for en udenlandsk arbejdsgiver. Lovforarbejderne præciserer, at
undtagelsen gælder den berørte indkomst. En udenlandsk arbejdsgiver alene
udelukker derfor ikke hele årets fradrag.
[Ligningsloven, §§ 9 J–9 K](https://www.lovtidende.dk/api/pdf/250970),
[L 238, 2017–18, bemærkninger til § 1, nr. 3, side 15–16](https://www.ft.dk/ripdf/samling/20171/lovforslag/l238/20171_l238_som_fremsat.pdf).

Oplys de tre forhold for **samme kilde, periode og beløb**. Et dokumenteret nej
til én betingelse afkræfter undtagelsen; ellers må manglende oplysninger stå som
`null`. Bopælsadresse eller statsborgerskab fastslår ikke automatisk DBO-hjemsted.
Modellen træffer ikke selv en afgørelse om hjemsted.

## Se et lille eksempel først

Brug en compiler, der består det aktuelle
[tax-audit-kompatibilitetstjek](../../website/public/ai-setup.md#tax-audit-runtime-check),
ikke alene det oprindelige download med versionsnummer 0.2.0. Kør fra repoets rod:

```sh
./target/release/runa examples/danish-income-tax/beskaeftigelsesfradrag.scenario.runa
```

Det [fiktive eksempel](beskaeftigelsesfradrag.scenario.runa) giver:

| Oplyst § 9 J-grundlag i 2026 | Almindeligt beskæftigelsesfradrag | Jobfradrag |
| --- | ---: | ---: |
| 600.000 kr., ingen del udelukket | 63.300 kr. | 3.100 kr. |
| 300.000 kr. bevaret, 300.000 kr. udelukket | 38.250 kr. | 2.916 kr. |
| Samme beløb, men nødvendigt DBO-hjemsted uafklaret | intet sammenligneligt beløb | intet sammenligneligt beløb |

Fradragene er ikke skattebesparelser. Modellen genbruger de eksisterende
årssatser og lofter i [ligningsloven_fradrag.runa](ligningsloven_fradrag.runa).
Den præcise administrative afrunding til hele kroner er fortsat ikke
uafhængigt verificeret; dette arbejde ændrer ikke den eksisterende projektion.

## Oplys egne fakta uden at gætte

1. Opgør årets § 9 J-grundlag **før denne udlandsundtagelse** ud fra relevante
   indkomst- og pensionsoplysninger. Det er ikke nødvendigvis bruttolønnen og
   slet ikke nettoløn, betalt AM eller årsopgørelsens fradrag. Bevar `null`, hvis
   grundlaget mangler; udled det ikke baglæns af det fradrag, du vil kontrollere.
2. Fordel hele grundlaget på dokumenterede kilder og perioder. Relevante
   ansættelsesrelaterede pensionsbidrag følger ansættelsen. En ATP- eller
   pensionspost er ikke automatisk »ikke ansættelsesindkomst«.
3. Del en kilde, når forholdene ændrer sig. Oplys dagnummer i indkomståret,
   begge endepunkter inklusive: 1 er 1. januar; året slutter på 365, eller 366
   i 2024. Samme kildes perioder må ikke overlappe. Der beregnes ingen
   automatisk beløbsfordeling efter dage.
4. Bevar negative øvrige grundlagsdele, fx et virksomhedsunderskud. De må
   ikke slettes, blot fordi en anden kilde er positiv. Negative
   ansættelsesbeløb kræver derimod korrektionstilknytning, som denne første
   model ikke understøtter.
5. Bekræft først fuldstændighed efter gennemgang af alle kilder. Posternes sum
   skal stemme med den selvstændige grundlagsopgørelse. Kontrollerne bekræfter
   ikke dokumenternes sandhed og kan ikke opdage en dublet, der omdøbes som
   en ny kilde.

Gem dokumenter, input og resultater uden for checkout. Fra repoets rod kan du
generere en JSON-skabelon; vælg et nyt filnavn i en privat mappe, du allerede har:

```sh
./target/release/runa template examples/danish-income-tax/beskaeftigelsesfradrag.calculate.runa --format json --output /absolut/privat/mappe/arbejdsfradrag.json
./target/release/runa call examples/danish-income-tax/beskaeftigelsesfradrag.calculate.runa --input /absolut/privat/mappe/arbejdsfradrag.json --output /absolut/privat/mappe/arbejdsfradrag-resultat.json
```

Skabelonen starter uafklaret. Udfyld og gennemgå den **før** `call`; ret ikke
schemafingeraftrykket. `schema` viser danske spørgsmål og kildehenvisninger til
felterne. Et vellykket `call` kan godt indeholde ugyldige modelgrundlag: kontroller
`afgrænsning.alle_input_gyldige`, de enkelte `kontroller` og
`fradrag_til_sammenligning`. `null` er ikke et nulfradrag.

Enligforsørger- og seniorfradrag er separate tillæg og er ikke med i de to
viste fradragsbeløb. LL § 9 L ekstra pensionsfradrag bruger sit eget grundlag;
det må ikke sættes til nul på grund af § 9 J-undtagelsen.
[Ligningsloven, §§ 9 J–9 L](https://www.lovtidende.dk/api/pdf/250970).

Den tekniske indgangsgrænse er 1.000 poster med højst +/- 1 mia. kr. pr. post,
ikke et lovbestemt loft. Fordelingsmodulet er også kontrolleret med native
kørsel. Den samlede fradragsberegnings importerede lovmodul har fortsat en
native-begrænsning; brug den viste fortolkede kørsel eller `runa call`.
