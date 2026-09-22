# Hvad ændrer sig, hvis jeg indbetaler mere på pension?

Start med dit spørgsmål, ikke med hele skattearbejdsbogen. Futuruna beregner
ud fra regler og oplyste fakta; en AI kan hjælpe med interviewet og forklare
resultatet, men afgør ikke skatten. Modellen er forskningssoftware, ikke
individuel skatte- eller pensionsrådgivning. `schema`, `template` og `call` er
Preview.

## Vælg den første opgave

- **»Stemmer min årsopgørelse?«** Brug
  [afstemning af årsopgørelsen](aarsopgoerelse-afstemning.md). Du kan få en
  betinget afstemning uden din ægtefælles årsopgørelse. De nødvendige,
  beregnede ægtefællebeløb er betingelser, ikke bekræftede fakta.
- **»Hvad hvis jeg indbetaler mere eller mindre på pension?«** Sammenlign
  samme års grundlag med én tydeligt beskrevet ændring som nedenfor.
- **»Er der fradrag, jeg bør undersøge?«** Start med tjeklisten nedenfor.
  En relevant mulighed er ikke det samme som dokumenteret fradragsret.

Afklar først indkomståret. En gennemgang af årsopgørelsen for 2025 og en
planlagt indbetaling i 2026 er to forskellige opgaver. En hypotetisk ændring
af et afsluttet år betyder ikke, at betalingen kan foretages med tilbagevirkende
kraft. Gem personlige dokumenter, input og resultater uden for Git-projektet.

## Pension: stil kun de næste nødvendige spørgsmål

1. Mener du **indbetaling** eller **udbetaling**? Her starter vi med en ændring
   af årets indbetalinger. Mindre fremtidig indbetaling er ikke en hævning af
   allerede bundne pensionsmidler.
2. Hvilket år, hvilken fødselsdato og hvilken ordning: ratepension, livsvarig
   livrente eller noget andet? Brug ikke ratepensionsloftet på alle ordninger.
3. Er ordningen privat eller arbejdsgiveradministreret? En egenbetaling over
   lønnen er ikke nødvendigvis en privattegnet ordning. Oplys for hver ordning
   bruttoindbetaling og eventuelt indeholdt AM-bidrag særskilt.
4. Hvad er allerede betalt, hvad forventes betalt resten af året, og hvad skal
   ændres? Saml alle relevante ordninger. Beløbene i beregningen er årlige,
   ikke månedlige. Brug pensionsselskabets eller bankens oplysninger; årets
   indbetalinger er ikke nødvendigvis synlige i TastSelv endnu.
5. Har der været pensionsudbetalinger i år eller sidste år? De kan påvirke
   grundlaget for det ekstra pensionsfradrag. Ukendt er ikke nul.

For 2026 er det fælles loft for ratepension og ophørende livrente 68.700 kr.
Arbejdsgiverordningen har prioritet, så der kan være mindre plads til privat
indbetaling. Kontroller indbetalingernes fordeling hos udbyderen, før du
handler. [Skattestyrelsens vejledning om pensionsindbetalinger](https://skat.dk/borger/pension-og-efterloen/fradrag-for-indbetalinger-til-pension).

Det ekstra pensionsfradrag er et **yderligere ligningsmæssigt fradrag**, ikke
en tilsvarende udbetaling eller procentvis skatterabat. Dets sats afhænger af
afstanden til folkepensionsalderen, og grundlaget har sit eget loft.
[Skattestyrelsens vejledning om ekstra pensionsfradrag](https://skat.dk/borger/fradrag/ekstra-pensionsfradrag).

## Beregn før og efter fra samme grundlag

Når de nødvendige kildefakta foreligger, brug den eksisterende
`beregn_personskat` i [personskat.calculate.runa](personskat.calculate.runa).
Der skal ikke skrives en ny skatteformel eller startes en Explore-strøm.

Fra projektets rod, med `PRIVATE_WORK_DIR` erstattet af din private mappe:

```sh
runa template examples/danish-income-tax/personskat.calculate.runa --format json --output PRIVATE_WORK_DIR/pension-cases.json
# Udfyld og gennemgå fakta før beregningen.
runa call examples/danish-income-tax/personskat.calculate.runa --input PRIVATE_WORK_DIR/pension-cases.json --output PRIVATE_WORK_DIR/pension-results.json
```

En genereret skabelon er **ikke et udfyldt menneske**. Nulbeløb, tomme lister
og standardvalg kræver stadig bekræftelse, og fødselsdatoen skal være rigtig.
Bevar en liste over ukendte oplysninger. Lad være med at vælge »ingen udgifter«
eller »ingen ægtefælle« for at få beregningen til at køre.

Gem den udfyldte, uændrede grundsag som `før`. Kopiér den til en sag med et
nyt `case_id`, og ændr kun de aftalte kildefakta:

- Private indbetalinger ligger i
  `lønmodtager.pension.pbl18_indbetalinger`. Bevar ordningens identifikation,
  kilde, betalingsår, øvrige indbetalinger og begrænsninger. Ændr det valgte
  betalingsbeløb, ikke et beregnet fradrag.
- Bevar løn, kommune, øvrige indkomster, udgifter og husholdningsfakta, medmindre
  den faktiske ændring også påvirker dem. En omlægning mellem løn og
  arbejdsgiverpension kræver sit eget sammenhængende før/efter-grundlag.
- Kontroller forskellen mellem inputtene før kørsel. Det skal fremgå, hvilke
  fakta er dokumenteret, og hvilke fremtidige beløb er scenarieantagelser.

Læs [resultatets vurdering](personskat-validity.md) for **begge** sager.
Hvis en af dem mangler `vurdering.slutskat_til_sammenligning_øre`, må der ikke
beregnes en skatteforskel. Heller ikke ved at erstatte `null` med nul.
`BeregnetMedForbehold` tillader en sammenligning af den modellerede del; det
beviser ikke, at alle relevante forhold er dækket.

Et brugbart svar viser:

| Beløb | Hvor kommer det fra? |
| --- | --- |
| Ændret indbetaling | Forskellen mellem de aftalte input, med år og ordning. |
| Tilladt privat ratefradrag og uudnyttet/afskåret beløb | `pension.pbl18_årsresultat`, som anvender de fælles årsgrænser. |
| Ekstra pensionsfradrag | `skat.ekstra_pensionsfradrag_kroner`. |
| Mindre modelleret skat | Før-skat minus efter-skat, fra begge gyldige sammenligningsbeløb i øre. |
| Færre frie midler efter skat | Merindbetaling minus mindre skat; kun ved uændret øvrig indkomst og betalinger. |

»Mindre skat« er ikke nødvendigvis en større kontant tilbagebetaling på
årsopgørelsen. Forudbetalt skat og tidligere udbetalinger påvirker afregningen.
Sammenligningen værdisætter heller ikke fremtidig skat, pensionsafkast,
omkostninger, forsikringsdækning, binding eller indkomstafhængige ydelser.

Hvis ægtefællefakta mangler, må de nødvendige overførsler fra en betinget
rapportafstemning ikke låses som kendte fakta i en ny pensionsberegning.
En ændring kan påvirke overførslerne. Vis i stedet den afgrænsede
fradragsberegning eller tydeligt betingede scenarier, og sig hvad der mangler
for at beregne skattevirkningen.

## Prøv et helt fiktivt eksempel

[pension-demo.mjs](pension-demo.mjs) kører otte små sager gennem den kanoniske
Futuruna-beregning. JavaScript opretter kun fiktive input, kontrollerer
resultater og trækker før/efter-beløb fra hinanden; skattereglerne ligger i
Futuruna. Eksemplet læser ingen personlige filer og indlæser ingen LLM.

Hvis Node.js allerede er installeret, kør fra projektets rod:

```sh
node examples/danish-income-tax/pension-demo.mjs ./target/release/runa
```

Angiv en aktuel `runa`-binær, der understøtter modellens beregningskontrakt.
Scriptet kræver Node.js 18 eller nyere, men Node er ikke nødvendigt for den
almindelige `runa template`/`runa call`-arbejdsgang. Installer ikke ekstra
software blot for dette valgfrie eksempel uden brugerens accept.

Eksemplets person er født 1. januar 1990, har 600.000 kr. i kontant bruttoløn
i 2026, bor i København, betaler ikke kirkeskat og har ingen ægtefælle,
ejendom, anden indkomst, pensionsudbetalinger eller andre fradragsudgifter.
Alle tomme grene er udtrykkelige **fiktive fravalg**, ikke rigtige personers
manglende oplysninger.

Scriptet viser mindre/mere privat indbetaling, en separat fast
arbejdsgiverordning og en sag med uoplyste udgifter. Det kører med én arbejder
og gemmer input, fulde resultater, kontraktfingeraftryk og en kort opsummering
i en ny midlertidig mappe uden for projektet. Det ændrer ingen eksisterende
filer. Den sidste sag skal have manglende sammenligningsbeløb, ikke nul skat.

Det observerede resultat med modellen den 22. september 2026:

| Ændring af årets private indbetaling | Ændring i modelleret skat | Ændring i frie midler efter skat |
| --- | ---: | ---: |
| 40.000 → 30.000 kr., uden arbejdsgiverordning | 3.820,68 kr. mere skat | 6.179,32 kr. mere |
| 40.000 → 50.000 kr., uden arbejdsgiverordning | 3.820,68 kr. mindre skat | 6.179,32 kr. mindre |
| 20.000 → 22.700 kr., med den faste arbejdsgiverordning | 1.031,59 kr. mindre skat | 1.668,41 kr. mindre |
| 22.700 → 22.701 kr., med den faste arbejdsgiverordning | Uændret | 1,00 kr. mindre |

Arbejdsgiverordningen i de to sidste rækker har 50.000 kr. brutto og 4.000 kr.
i oplyst AM-bidrag. Den forbruger 46.000 kr. af rateloftet, så modellen
beregner plads til 22.700 kr. i privat fradrag. Rækkerne sammenligner ikke
privat- og arbejdsgiverordninger som alternative lønpakker. Tallene er
eksempelresultater, ikke en generel skattesats eller personlig anbefaling.

## Hvilke fradrag er værd at undersøge?

Brug listen som interview, ikke som en påstand om fradragsret. Den er ikke
udtømmende. Begynd med udgifter, der faktisk er afholdt eller planlagt, og
kontroller om de allerede er medtaget. Køb ikke noget alene fordi ordet
»fradrag« optræder. Se også
[Skattestyrelsens fradragsoversigt](https://skat.dk/borger/fradrag).

| Mulighed | Næste relevante oplysninger |
| --- | --- |
| Befordring | Faktiske fremmødedage, daglig tur/retur-afstand, arbejdssteder, betalt transport og godtgørelser. Ikke automatisk 220 dage. |
| Fagforening og A-kasse | Årets indbetalinger opdelt efter art, udbyder og allerede indberettede beløb. |
| Renter | Årsoversigt, faktisk renteudgift og egen hæftelse/andel; ikke lånets afdrag. |
| Service eller grønt håndværk | Arbejdets art/dato, faktura, løn/materialer, betaling, bolig og eventuel fordeling. Start gerne med den [lille fakturaberegning](boligjob.md). |
| Gaver | Modtager, godkendelsesgrundlag, kvittering og indberetning. |
| Enlig forsørger | Oplysninger om ekstra børnetilskud og relevante kvartaler, ikke alene civilstand. Se [kildemodellen](ligningsloven-par9j-enlig.md). |
| Arbejdsrejser eller andre arbejdsudgifter | Arbejdssted, periode, udgiftstype, bilag og arbejdsgiverens betaling/godtgørelse; undersøg den specifikke regel. |

Afslut hver mulighed med én af fire konklusioner: **dokumenteret og beregnet**,
**allerede medtaget**, **kræver flere oplysninger**, eller **ikke omfattet af
den undersøgte regel/model**. Angiv det konkrete næste skridt: eksempelvis
hent en årsoversigt, få løndelen af fakturaen specificeret, eller afklar
indbetalingerne med pensionsselskabet. Udfør ikke ændringer i TastSelv eller
pensionsaftaler på brugerens vegne uden særskilt tilladelse.

Officielle vejledninger ovenfor kontrolleret 22. september 2026. Kodens
juridiske kildeankre findes blandt andet i
[pensionsbeskatningsloven.runa](pensionsbeskatningsloven.runa) og
[ligningsloven_fradrag.runa](ligningsloven_fradrag.runa).
