# Stemmer mit beskæftigelsesfradrag ved udenlandsk arbejde?

Den [afgrænsede beregning](beskaeftigelsesfradrag.calculate.runa) kontrollerer
almindeligt beskæftigelsesfradrag og jobfradrag for 2023–2026. Den viser, hvilke
grundlagsbeløb der bevares eller udelukkes, og giver ingen sammenlignelige
fradragsbeløb, hvis nødvendige fakta er uafklarede. Det er en forskningsmodel,
ikke individuel skatterådgivning. `schema`, `template` og `call` er Preview.

Den samlede [Personskat-beregning](personskat.calculate.runa) bruger samme
fordeling og beregner selv grundlaget ud fra indkomst og pension. Her påvirker
afgrænsningen også senior- og enligforsørgertillæg, når deres betingelser er
opfyldt. Den afgrænsede beregning ovenfor viser derimod kun de to fradrag, ikke
skat, udenlandsk skattenedslag eller værdien af et ekstra fradrag.

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

## I den samlede Personskat-beregning

Det nye felt er `lønmodtager.ligningsfradrag.arbejdsfradrag_udland`.
Skabelonen starter som `ArbejdsfradragUdlandUoplyst`, ikke som et stiltiende nej.

- Hvis mindst én af de tre betingelser kan afkræftes for **alle relevante
  ansættelser og perioder**, vælg `IngenUdlandsudelukkelseIFællesForhold`.
  Oplys det dokumenterede nej og kildereferencen; andre uafklarede forhold kan
  blive stående som `null`. Kommunen eller en dansk adresse er ikke i sig selv
  dokumentation for DBO-hjemsted.
- Ellers vælg `FordeltArbejdsfradragsgrundlag`, og oplys fordelingen som
  ovenfor. Dens sum skal stemme med Personskats afledte grundlag **før**
  udlandsundtagelsen. Tre ja-værdier i den fælles gren accepteres ikke som
  grundlag for at udelukke hele årets indkomst.
- Kontroller `arbejdsfradrag_udland.grundlag` og `kontroller` i resultatet og
  derefter [den samlede vurdering](personskat-validity.md). En aktiv ægtefælle
  har samme kontrol i sit eget grundlag. Uoplyste eller modstridende forhold
  tilbageholder `vurdering.slutskat_til_sammenligning_øre`.

Afgrænsningen ændrer ikke AM-bidrag, personlig indkomst eller LL § 9 L's
pensionsgrundlag. Et fiktivt 2026-grundlag på 600.000 kr., hvor 300.000 kr.
udelukkes, giver i den samlede beregning de samme 38.250 kr. og 2.916 kr. som
ovenfor; et berettiget senior- eller enligforsørgertillæg bruger også de
bevarede 300.000 kr. Det er ikke en påstand om fuld dækning af en udlandssag:
skattepligt, DBO-lempelse og andre indkomstforhold kræver deres egne fakta.

Ved pensions- og lønscenarier skal fordelingen følge den ændrede kilde.
Genbrug ikke en gammel fordeling efter ændring af grundlaget: en forkert sum
bliver afvist, ikke automatisk rettet. Generér nye JSON/XLSX-skabeloner efter
denne kontraktændring; ændr ikke gamle schemafingeraftryk. Repoets fiktive
scenariehjælpere er ikke dokumentation for en rigtig persons udlandsforhold.

## Når skattepligten kun gælder en del af året

I [delårsberegningen](personskat-par14.calculate.runa) følger det udelukkede
grundlag med til helårsberegningen. Tilføj én kilde med beregningsfelt
`Par14Ll9jUdelukketUdenlandskAnsættelsesindkomst` for hver udelukket
fordelingspost. Brug samme `identifikation` og `delårsbeløb_kroner` som posten.
Det er en afgrænsning, ikke endnu en løn eller et ekstra fradrag.

Omregningsmetoden skal passe til netop denne kildes grundlag: løbende beløb,
engangsbeløb eller et dokumenteret retvisende helårsbeløb. Fradraget beregnes
igen af det bevarede helårsgrundlag; det omregnes ikke blot efter dage.
Ved valg af faktisk helårsindkomst skal også det faktiske udelukkede
helårsbeløb oplyses. Brug `DokumenteretHelårsPersonskat`, når ændrede
forhold eller sammensatte årsforløb kræver en fuld opgørelse; dens udelukkelse
skal stemme med kildebeløbene.
[Den juridiske vejledning, C.F.1.6.2.1](https://info.skat.dk/data.aspx?oid=1977388).

En kilde, som kun forekommer uden for delårsperioden, har nul i
`delårsbeløb_kroner`. Identifikationen og helårsbeløbet skal da kunne genfindes
i `DokumenteretHelårsPersonskat`; opfind ikke et ansættelsesforhold i delåret.

Se `input_gyldigt`, `kilder_gyldige`, `kilder_afstemt_med_personskat` og
`helårsgrundlag_gyldigt` før brug af delårsresultatet. Manglende afgrænsning,
forkert kilde eller negativt udelukket beløb afvises. Modellen kontrollerer
identitet og beløb, men kan ikke bekræfte dokumenternes sandhed eller vælge
en juridisk retvisende omregningsmetode uden fakta.

Den tekniske indgangsgrænse er 1.000 poster med højst +/- 1 mia. kr. pr. post,
ikke et lovbestemt loft. Både fordelingsmodulet og det viste fradragseksempel
er kontrolleret med native kørsel og giver samme output som den fortolkede
kørsel. Eksemplet kan også køres med:

```sh
./target/release/runa run examples/danish-income-tax/beskaeftigelsesfradrag.scenario.runa
```

Denne kontrol gælder den afgrænsede arbejdsfradragsberegning, ikke native
kørsel af hele Personskat.

### Hvis du bruger lovmodulets parameterfunktioner direkte

Årstabellerne har eksplicitte `*_opslag`-funktioner, som returnerer `Some(...)`
ved et dækket opslag og `None`, når den nødvendige parameter mangler.
De eksisterende værdifunktioner beholder navn og værdi for dækkede opslag,
men stopper ved manglende parametre; fravær bliver ikke til en nulsats.
Brug opslaget, hvis dit program selv skal håndtere et udækket år.
En kendt pensionsprocent er ikke nok til et fuldt pensionsfradrag: uden årets
reguleringstal kan modellen ikke fastlægge det regulerede loft. Dette udvider
ikke auditberegningens understøttede skatteår.
