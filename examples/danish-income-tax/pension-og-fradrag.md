# Hvad ændrer sig, hvis jeg indbetaler mere på pension?

Start med dit spørgsmål, ikke med hele skattearbejdsbogen. Futuruna beregner
ud fra regler og oplyste fakta; en AI kan hjælpe med interviewet og forklare
resultatet, men afgør ikke skatten. Modellen er forskningssoftware, ikke
individuel skatte- eller pensionsrådgivning. `schema`, `template` og `call` er
Preview.

Før beregningen skal den valgte compiler bestå det lille
[kompatibilitetstjek](../../website/public/ai-setup.md#tax-audit-runtime-check).
Den oprindelige `v0.2.0`-download mangler senere sikkerhedsrettelser; samme
versionsnummer er ikke nok. Tjekket bruger kun fiktive data. Fortsæt ikke med
personlige beregninger, hvis det fejler.

Bevar den afprøvede binærs absolutte sti i `RUNA_BIN`, som i opsætningsguiden,
og brug samme sti i kommandoerne nedenfor. En anden `runa` på din PATH er
ikke nødvendigvis den binær, der bestod tjekket.

## Vælg den første opgave

- **»Stemmer min årsopgørelse?«** Brug
  [afstemning af årsopgørelsen](aarsopgoerelse-afstemning.md). Du kan få en
  betinget afstemning uden din ægtefælles årsopgørelse. De nødvendige,
  beregnede ægtefællebeløb er betingelser, ikke bekræftede fakta.
- **»Hvad hvis jeg indbetaler mere eller mindre på pension?«** Sammenlign
  samme års grundlag med én tydeligt beskrevet ændring som nedenfor.
- **»Er der fradrag, jeg bør undersøge?«** Start med tjeklisten nedenfor.
  En relevant mulighed er ikke det samme som dokumenteret fradragsret.
- **»Stemmer mit beskæftigelsesfradrag ved udenlandsk arbejde?«** Brug den
  [afgrænsede indkomstfordeling](beskaeftigelsesfradrag.md). Den håndterer blandede
  kilder og bruges også i den samlede Personskat-beregning. Den enkle sag
  kræver et dokumenteret nej til mindst én betingelse for alle relevante
  ansættelser; skabelonens udgangspunkt er uoplyst.

Afklar først indkomståret. En gennemgang af årsopgørelsen for 2025 og en
planlagt indbetaling i 2026 er to forskellige opgaver. En hypotetisk ændring
af et afsluttet år betyder ikke, at betalingen kan foretages med tilbagevirkende
kraft. Gem personlige dokumenter, input og resultater uden for Git-projektet.

Afklar også, om dansk skattepligt gjaldt hele året. Færre lønmåneder er ikke
i sig selv delårsskattepligt. Ved indtræden eller ophør skal
[delårsindgangen](personskat-delaar.md) og dens endelige vurdering bruges,
når modellen dækker forløbet; den almindelige beregning og de fiktive
helårseksempler nedenfor foretager ikke automatisk PSL § 14-omregning.

### Skattekommunen er ikke altid din nuværende bopæl

`lønmodtager.kommune` betyder **skattekommunen for indkomståret**. Hovedreglen
bruger bopælen den 5. september året før, men skattesatsen fra selve indkomståret.
Ved eksempelvis tilflytning fra udlandet, flere bopæle, institutionsophold eller
skibsophold skal særreglerne afklares; modellen udleder ikke valget fra din
adresse. Det gælder også ægtefællens kommune, som ikke må gættes ud fra din.
[Kommuneskattelovens § 2](https://www.retsinformation.dk/eli/lta/2019/935).

Et fiktivt eksempel: Du boede i København den **5. september 2025** og flyttede
senere til Ballerup. Under hovedreglen er skattekommunen for 2026 København,
selv om du nu bor i Ballerup. Med 400.000 kr. i **skattepligtig indkomst**, ikke
bruttoløn, giver kommuneskatten før personfradrag 93.560 kr. Forkert valg af
Ballerup ville give 102.000 kr. — en forskel på 8.440 kr. Dette er kun én
skattekomponent, ikke forskellen i samlet slutskat. Satserne er henholdsvis
23,39 % og 25,50 % i [ministeriets 2026-tabel](https://svmn.dk/tal-og-metode/satser/oversigt-over-kommuneskatter).
Det [eksekverbare eksempel](kommuneskattelov-skattekommune.scenario.runa)
kontrollerer kommunevalg, årssats og beløb.

I den kompakte rapportafstemning oplyses derimod **rapportens** skattekommune.
Et aritmetisk match beviser ikke, at rapportens kommunevalg er korrekt. Hvis
rapporten og dine bopælsfakta strider mod hinanden, skal forskellen undersøges;
skift ikke kommunen blot for at få tallene til at passe.

Feltstier og skatteformler er uændrede. Nye feltforklaringer ændrer kontraktens
fingeraftryk: genopret en skabelon med den valgte compiler, og overfør de
kontrollerede fakta. Ret ikke kun `schema_hash` i gamle input eller resultater.

### Fødselsdato er ikke kun et pensionsfelt

Oplys den faktiske fødselsdato under `lønmodtager.pension.fødselsdato`, også
hvis der hverken er pensionsindbetalinger eller -udbetalinger. Den bruges også
til AM-bidrag. Fra 2026 er satsen 0 % til og med det indkomstår, hvor man fylder
17. Fra begyndelsen af det år, hvor man fylder 18, er satsen 8 % — ikke først
fra fødselsdagen. Reglen gælder ikke bagud for 2025.
[Lov nr. 96 af 4. februar 2025, § 1 og § 7, stk. 4](https://www.retsinformation.dk/eli/lta/2025/96/pdf).

For et fiktivt, almindeligt løngrundlag på 100.000 kr., uden andre indkomster,
er indkomsten **før pensionsfradrag** derfor:

| Indkomstår | Alder ved årets udgang | AM-bidrag | Indkomst efter AM, før pensionsfradrag |
| --- | --- | --- | --- |
| 2025 | 17 | 8.000 kr. | 92.000 kr. |
| 2026 | 17 | 0 kr. | 100.000 kr. |
| 2026 | 18 | 8.000 kr. | 92.000 kr. |

Nul AM-bidrag betyder ikke automatisk nul indkomstskat. Løngrundlaget indgår
fortsat i beskæftigelsesfradraget; satsen på bidraget og grundlaget for fradraget
er forskellige ting. Den foreløbige pensionsberegning og den endelige
skatteberegning skal anvende samme oplyste fødselsdato, ikke en voksen
standardperson. Manglende dato er uafklaret, ikke en grund til at vælge 1990.
[Lovforarbejderne, bemærkninger til § 1](https://www.retsinformation.dk/eli/ft/202412L00117),
[Skattestyrelsens guide til første job](https://skat.dk/borger/unge-og-studerende/job/foerste-job).

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

Små forskelle kan skyldes afrunding. Modellen oprunder nu det ekstra
pensionsfradrag til hele kroner efter modregning og loft, i overensstemmelse
med [otte fiktive observationer hos SKAT for 2025/2026](skatdk-pensionsfradrag-ekstern.md).
Ved et grundlag på 40.001 kr. og 12 % bliver fradraget 4.801 kr., ikke
4.800 kr. Genberegn tidligere resultater fra de samme fakta; en resterende
forskel er ikke i sig selv bevis for en fejl i din årsopgørelse.

### Løn og arbejdsgiverpension: brug ikke samme beløb to gange

`lønmodtager.bruttoløn_kroner` er løngrundlaget før AM-bidrag og skat,
ikke lønpakken inklusive pension. Hold arbejdsgiveradministrerede bidrag,
også din egen andel over lønnen, adskilt fra lønnen. Private indbetalinger
skal derimod ikke trækkes fra lønfeltet. Lønoplysningerne er efter fradrag
af ATP og eget arbejdsgiveradministreret pensionsbidrag.
[Skattestyrelsens forklaring af AM-bidrag](https://skat.dk/borger/am-bidrag).

Samme definition gælder for ægtefællens lønfelt. Kontraktens spørgsmål og hjælp
kommer fra én typet metadata-definition for begge personer. Hvis lønlinjen
allerede er efter egen ATP og bortseelsesberettiget pensionsandel, må du ikke
trække dem fra endnu en gang. Brug heller ikke bankens nettoudbetaling eller
årsopgørelsens samlede personlige indkomst som almindelig løn.

Et rubriknummer er ikke en fuldstændig fordeling: lønindberetningens felt 13
kan også indeholde fx skattepligtige personalegoder og godtgørelser. Hvis et
beløb oplyses gennem en særskilt modelgren, må det ikke samtidig tælles med i
det almindelige lønfelt; bevar dokumenteret opdeling, og udelad ikke et beløb
uden at føre det til den relevante gren. Private pensionsbetalinger holdes
adskilt fra denne afgrænsning. Afklar manglende detaljer frem for at tilpasse
lønnen, så beregnet skat passer.
[eIndkomstvejledningen 8.2, felt 13](https://info.skat.dk/data.aspx?oid=2233519).

Metadataændringen ændrer kontraktens fingeraftryk, men ikke lønfeltets sti,
datatype eller skatteformler. Generér en ny skabelon og overfør gennemgåede
fakta; redigér ikke det gamle fingeraftryk. Ægtefællens tidligere kortere
felttekst er ikke grundlag for at bruge en anden løndefinition.

En almindelig arbejdsgiverbetalt livsvarig livrente indgår **før AM-bidrag**
i grundlaget for beskæftigelses- og jobfradrag. Det ekstra pensionsfradrag
bruger derimod bidraget **efter indeholdt AM-bidrag**. Derfor skal både
bruttoindbetaling og faktisk indeholdt bidrag oplyses; et nettobeløb divideret
med 0,92 er ikke dokumentation for bruttoindbetalingen.
[LL §§ 9 J–9 L](https://www.lovtidende.dk/api/pdf/250970),
[AMBL § 2, stk. 1, nr. 4](https://www.retsinformation.dk/eli/lta/2020/121).

Et fiktivt 2026-eksempel: 200.000 kr. i løngrundlag, 50.000 kr. på en
almindelig arbejdsgiverlivrente, 4.000 kr. indeholdt pensions-AM, født i 1990,
ingen øvrige indkomster, pensionsudbetalinger eller ATP:

| Fradrag | Anvendt grundlag | Beregnet fradrag |
| --- | --- | --- |
| Beskæftigelsesfradrag | 250.000 kr. før AM | 31.875 kr. |
| Jobfradrag | 250.000 − 235.200 kr. | 666 kr. |
| Ekstra pensionsfradrag | 50.000 − 4.000 kr. | 5.520 kr. |

Satserne og jobfradragets bundgrænse fremgår af
[Skattestyrelsens 2026-satser](https://skat.dk/borger/fradrag/arbejdsrelaterede-fradrag/beskaeftigelses-og-jobfradrag).
Beløbene i sidste kolonne er fradrag, ikke sparet skat. Indeholdt pensions-AM
opkræves ikke igen som løn-AM. En privat livrente og en arbejdsgiverbetaling
henført til et andet år må ikke lægges til årets løngrundlag på denne måde.

Den kanoniske model medtager almindelig arbejdsgiverlivrente fra både
`pbl18_indbetalinger` og den særskilte § 19-kildevej nedenfor. Det afledte beløb kan
følges under `pension.lønmodtager_pensionsfradrag` i feltet
`øvrigt_arbejdsmarkedsbidragsgrundlag_med_indeholdt_bidrag_kroner`; det kan
også indeholde andre særskilt modellerede bidragsgrundlag. Tidligere resultater
kan undervurdere fradragene. Beregn berørte sager igen fra kildefakta med en
ny skabelon; ret ikke blot slutskatten eller en gammel kontrakthash.

Eksemplet har udtrykkeligt ingen ATP. Har du ATP, skal den oplyses særskilt
som beskrevet nedenfor, ikke presses ind som almindelig livrente for at få
tallene til at stemme. Særlige pensionsordninger kræver deres eget grundlag.
Arbejdsgiverbetalinger over den almindelige rategrænse behandles nedenfor.

### ATP tæller også, selv om du ikke har firmapension

Årets ATP skal afklares under `lønmodtager.pension.atp`. Skabelonen starter
med `AtpUoplyst`. Vælg `IngenAtpIndbetalinger` kun efter gennemgang; ellers
brug `OplysteAtpIndbetalinger` med kildehenvisning, år og poster og bekræft
først fuldstændighed, når alle kilder er gennemgået. Det gælder også ved
flere arbejdsgivere eller en del af året på offentlige ydelser.

Find oplysningerne i TastSelv under **Skatteoplysninger → Arbejdsmarkedets
Tillægspension (ATP)** og i ATPs oversigt. Det er indbetalingernes kildebeløb,
du skal bruge, ikke årsopgørelsens beregnede ekstra pensionsfradrag.
[Skattestyrelsens ATP-vejledning](https://skat.dk/borger/pension-og-efterloen/fradrag-for-indbetalinger-til-pension).

Ved arbejdsgiverindberettet ATP er bruttobeløbet i eIndkomst felt 46
**lønmodtagerens og arbejdsgiverens bidrag i alt**. Dit eget ATP-løntræk er
ikke hele bidraget. Gæt ikke totalen ud fra løn, timer eller din egen andel.
Obligatorisk pensionsopsparing på overførselsindkomst har et særskilt felt 47.
[eIndkomstvejledningen, afsnit 8.2](https://info.skat.dk/data.aspx?oid=2233519).

| Dokumenteret kilde | Beskæftigelses-/jobfradragets grundlag | Ekstra pensionsfradrags grundlag |
| --- | --- | --- |
| Arbejdsgiverindberettet ATP, PBL § 19, stk. 1 | Indberettet bidrag før ATPs AM | Indberettet bidrag efter AM |
| ATP på offentlige ydelser, PBL § 19, stk. 2 | Intet tillæg | Indberettet bidrag efter AM |
| SUPP indbetalt til ATP, PBL § 19, stk. 4 | Intet tillæg | Indberettet bidrag efter AM |
| Obligatorisk pension efter ATP-lovens § 17 s, fra 2020 | Intet tillæg | Hele bidraget; AM-fritaget |

Afgrænsningen følger [LL §§ 9 J–9 L](https://www.lovtidende.dk/api/pdf/250970),
[PBL § 19](https://www.retsinformation.dk/eli/lta/2024/1243/pdf) og
[AMBL § 3, nr. 5](https://www.retsinformation.dk/eli/lta/2020/121).
Senior- og enligforsørgerfradrag bruger også arbejdsfradragsgrundlaget, når
betingelserne for dem er opfyldt. ATP forbruger ikke ratepensionsloftet.

Ved [udlandsundtagelsen](beskaeftigelsesfradrag.md) følger relevante
ansættelsesrelaterede pensions- og ATP-bidrag den pågældende kilde i
arbejdsfradragsgrundlaget. Det ekstra pensionsfradrags eget grundlag ændres
ikke af denne undtagelse. Opdater fordelingen, hvis et pensionsscenarie ændrer
grundlaget; en gammel sum må ikke bruges som genvej til et sammenligneligt
skattebeløb.

Oplys brutto og dokumenteret netto særskilt i hele kroner fra årets
beregningsgrundlag. Bevar originale kildebeløb med eventuelle øre; rund ikke
hver måned på egen hånd for at konstruere en årsindberetning. Der udledes
ikke automatisk et nettobeløb med 8 % AM: blandt andet blev AM-reglerne for
unge ændret fra 2026. `null` betyder stadig ukendt, og sammenligningsbeløbet
tilbageholdes, hvis nødvendige oplysninger mangler. Et forkert netto på
obligatorisk AM-fritaget pension afvises også.

Løn og offentlige ydelser oplyses som deres særskilte indkomstgrundlag.
ATP lægges **ikke** tilbage som skattepligtig løn, får ikke et ekstra privat
§ 18-fradrag og pålægges ikke endnu et AM-bidrag. Vis indberetningerne og de
to forskellige tillæg i `pension.atp_resultat`. Det ekstra fradrag kan fortsat
begrænses af loftet eller pensionsudbetalinger; tillægget er ikke sparet skat.

Et kørt, fiktivt 2026-eksempel isolerer forskellen: 200.000 kr. i løngrundlag,
født i 1990, København, ingen kirkeskat, ægtefælle, andre indkomster,
pensionsindbetalinger eller -udbetalinger. Med 5.000 kr. arbejdsgiverindberettet
ATP før AM og 4.600 kr. efter AM bliver beskæftigelsesfradraget 26.137 kr.
og det ekstra pensionsfradrag 552 kr. Modelleret skat er 55.742,04 kr., mod
56.020,15 kr. i kontrollen uden ATP: en forskel på 278,11 kr. ATP-beløbet er
en testværdi, ikke en standardsats eller en valgfri pensionsindbetaling.
Løn-AM er stadig 16.000 kr.; den allerede indeholdte ATP-AM opkræves ikke igen.

Brug en stabil betalingsidentifikation. Samme betaling i ATP-listen og den
almindelige pensionsliste eller under arbejdsgiverydelser giver en fejl;
omdøb ikke dubletten for at omgå kontrollen. Den dedikerede ATP-gren omfatter
ikke selvstændiges private ATP-indbetalinger eller SUPP hos andre
pensionsudbydere. Vælg ikke en arbejdsgivergren for sådanne forhold for at
få et resultat; deres fradragsgrundlag skal behandles særskilt.

**Migration:** `atp` er et nyt påkrævet input. Generer en ny skabelon, overfør
gennemgåede fakta, og beregn igen. En gammel sag uden feltet beviser ikke,
at der ingen ATP var. Det gælder også en aktiv ægtefælle og delårsberegning.
De fiktive pensionsdemonstrationer angiver udtrykkeligt ingen ATP; dette er
ikke et forslag til standardvalg i en virkelig sag.

### Betalingsår og forfaldsår

En pensionsbetaling hører ikke nødvendigvis til det år, pengene blev trukket.
Bevar både aftalens forfaldsår og bilagets faktiske betalingsår. For almindelige
forsikringspræmier kan rettidig betaling senest 1. april året efter forfald
henføre fradraget til forfaldsåret. Fristen flyttes kun til førstkommende
bankdag, **hvis 1. april er en banklukkedag**. Senere betaling følger
betalingsåret. Bankbaseret rateopsparing følger normalt betalingsåret.
Se [PBL § 18, stk. 1, og § 19, stk. 1](https://www.retsinformation.dk/eli/lta/2024/1243).

Den juridiske vejledning fastholder betalingsåret for almindelige
arbejdsgiverbidrag til pensionsordninger i pengeinstitutter. Den beskriver også
en særskilt godkendelse af henføring af løntilbageholdte bidrag til året før.
En sådan godkendelse må ikke erstattes af et gættet forfaldsår eller ændret
ordningstype; denne kontrol implementerer ikke godkendelsesforløbet.
[C.A.10.2.2.3.1, bidrag til pensionsordning i pengeinstitut](https://info.skat.dk/data.aspx?oid=2048283).

Input under `pbl18_indbetalinger.betaling` kontrolleres nu for årstalsmodstrid:

- Betaling i eller før forfaldsåret kan ikke samtidig være efter fristen.
- Betaling mindst to år efter forfaldsåret kan ikke være før fristen.
- Betaling i det efterfølgende år kan være rettidig eller sen; dokumentation
  for betalingsdagen og fristen afgør svaret. Årstallet alene er utilstrækkeligt.

`Pbl18Par15AIndbetalingsår` og `Pbl18Par15AAfståelsesår` er kun til
§ 15 A-ordninger. De er ikke en generel valgmulighed for almindelig ratepension,
som bruger `Pbl18IkkePar15APlacering`. Et fiktivt eksempel med 40.000 kr.
forsikringspræmie, forfald i 2024 og rettidig betaling i 2025 blev tidligere
accepteret med fradrag i 2025 ved et forkert § 15 A-valg. Det sænkede modellens
2025-skat med 15.332 kr. Modstridende valg tilbageholder nu sammenligningsbeløbet;
kildetal og årstal omskrives ikke. Det gælder også en beregnet ægtefælle.

Ukendt betalingsfrist må ikke blive `false`. Afklar bilaget før den uafhængige
beregning, hvis den påkrævede boolske oplysning ikke kan besvares. Kontrollen
beviser ikke en betalingsdato, en ordningsklassifikation eller særlig
godkendelse. [Tilbagebetaling og genindbetaling efter § 22 E](personskat-pensionskorrektion.md)
har nu særskilte dokumenterede fakta og beregnede korrektionsfrister; et ja/nej
alene er utilstrækkeligt. En post uden fradrag i det aktuelle år er ikke i sig
selv ugyldig.

**Migration:** Generer frisk schema/skabelon, da de fælles spørgsmål og
kildespor ændrer kontrakthashen. Gennemgå tidligere gemte tidsoplysninger ud
fra bilag og beregn igen; skift ikke blot et årstal eller en variant for at
få et resultat. [Regressionen](../../tests/personskat_pension_payment_year.test.mjs)
kontrollerer fiktive årsberegninger og fælles hovedperson/ægtefællevejledning,
ikke en ny uafhængig SKAT-observation.

### Hvis arbejdsgiveren har indbetalt over rategrænsen

En rubrik 347-linje kan også indeholde **gruppeliv**, ikke kun rateoverskud.
For lønfinansieret gruppeliv bruges dokumenteret brutto til arbejdsfradrag
og netto til personlig indkomst, uden nyt AM eller ekstra pensionsfradrag.
Se [gruppelivsguiden og skabelonændringen](personskat-gruppeliv.md), før en
samlet pensions-/forsikringsoversigt fordeles på flere inputposter.

Den fælles grænse for almindelig ratepension og ophørende livrente gælder
**efter indeholdt AM-bidrag**, på tværs af ordninger. Arbejdsgiverbidrag har
prioritet over private bidrag. Overskud fra arbejdsgiverordningen er personlig
indkomst; det giver ikke ekstra pensionsfradrag. Skattestyrelsen henfører
det til rubrik 347, hvor der ikke skal betales AM igen.
[Vejledningen om rategrænsen og prioritet](https://info.skat.dk/data.aspx?oid=2048285),
[Skattestyrelsens pensionsguide](https://skat.dk/borger/pension-og-efterloen/fradrag-for-indbetalinger-til-pension),
[oplysningsskemaets indkomstgrupper](https://skat.dk/media/s3hff0yh/04003-plus-04068-2025-t.pdf).

For en fiktiv voksen i 2026 med 100.000 kr. i arbejdsgiverbetalt ratepension
og 8.000 kr. i dokumenteret pensions-AM viser
`pension.arbejdsgiver_rate_resultat`:

| Størrelse | Beløb |
| --- | --- |
| Indbetaling efter AM | 92.000 kr. |
| Fælles rategrænse og bortseelsesberettiget beløb | 68.700 kr. |
| Overskud til personlig indkomst uden nyt AM | 23.300 kr. |

Ved 200.000 kr. i øvrigt løngrundlag bliver den personlige indkomst før
private pensionsfradrag 184.000 + 23.300 = 207.300 kr. Ved 12 %-satsen
er det ekstra pensionsfradrag 8.244 kr. Med dette arbejdsgiverbidrag er der
ingen resterende fradragsplads til privat ratepension. Modellen tilføjer selv
overskuddet fra de samordnede pensionsbetalinger; indtast ikke de samme
23.300 kr. igen som anden indkomst eller som en rubrik 347-samlepost.

Beskæftigelses- og jobfradrag omfatter både den bortseelsesberettigede
pension og det skattepligtige arbejdsvederlag. Her tælles det dokumenterede
bruttobeløb derfor én gang, også over grænsen. At begrænse dette samlede
grundlag til 68.700 kr. ville være en anden fejl.
[Lovforarbejderne til LL § 9 J, til § 1, nr. 1 og 2](https://www.retsinformation.dk/eli/ft/201712L00238).

Kontakt udbyderen om en eventuel overførsel til livrente. Modellen vælger
ikke en overførsel for dig. En almindelig tilbagebetaling af rateoverskud
giver ikke i sig selv et nyt fradrag i indbetalingsåret. En overførsel kan
ændre indberetningen; brug da dokumenterede, korrigerede oplysninger.
[Skattestyrelsens beskrivelse af valgmulighederne](https://skat.dk/borger/pension-og-efterloen/fradrag-for-indbetalinger-til-pension).

### To inputveje, én betaling

En almindelig arbejdsgiverpension kan oplyses i pensionslisten
`lønmodtager.pension.pbl18_indbetalinger` eller i varianten
`ArbejdsgiveradministreretPensionEfterPbl19` under
`lønmodtager.personlig_indkomst.ordinære_forhold.arbejdsgiverydelser`.
Den korte § 19-post kræver år, betalingsidentifikation, arbejdsgiverrelation,
ordningstype samt dokumenterede beløb før og efter indeholdt AM.
Brug pensionsudbyderens oplysninger — ikke et fradragstal fra årsopgørelsen.

Modellen samler almindelige ratebidrag og ophørende livrenter fra begge
lister **før** den fælles årsgrænse og arbejdsgiverprioriteten anvendes.
Almindelige livsvarige livrenter fra begge veje indgår i de relevante
fradragsgrundlag. Den opfinder ikke betalingsdatoer eller særlige
ordningsvilkår for at omdanne en kort § 19-post til en detaljeret § 18-post.

Det fiktive 200.000/50.000 kr.-eksempel ovenfor giver i København uden
kirkeskat **53.082,13 kr. i modelleret slutskat** gennem begge inputveje.
Med 50.000 kr. i arbejdsgiverbetalt ratepension (46.000 kr. efter AM) og
22.701 kr. i privat ratepension giver begge veje 22.700 kr. i privat fradrag,
1 kr. uden fradrag og **44.409,18 kr. i modelleret slutskat**.
Det er afgrænsede testresultater, ikke en godkendelse af en virkelig årsopgørelse.

Hver betaling må kun forekomme én gang. Samme betalingsidentifikation i
begge lister giver en fejl og intet sammenligningsbeløb. Fjern den dobbelte
registrering; løs ikke fejlen ved blot at omdøbe den. To reelt forskellige
betalinger må gerne have samme beløb. Modellen kan ikke genkende én betaling,
som fejlagtigt har fået to forskellige identifikationer: gennemgå derfor
kildernes overlap. Et samlet skattepligtigt beløb efter §§ 19/56 er heller
ikke dokumentation for en ny pensionsindbetaling eller et ekstra fradrag.

Under `pension.arbejdsgiverydelser_resultat` ses de oprindelige § 19-poster,
kontroller og supplerende brutto-/nettogrundlag. `arbejdsgiver_rate_resultat`
viser det **fælles** årsresultat. `pbl18_årsresultat.arbejdsgivergrundlag`
omfatter også de supplerende ratebidrag, selv om de oprindelige § 18-inputrækker
bevares uændret. Et kendt nettobeløb gør ikke et ukendt bruttobeløb kendt.
Bevar `null`, hvis dokumentationen mangler; beregningen tilbageholder da
sammenligningsbeløbet og angiver den manglende inputsti. Det gælder også en
beregnet ægtefælle og ved delårsberegning.

Den korte § 19-gren dækker ikke særlige ordninger efter §§ 15, 15 A og 15 B
eller almindelige gamle kapitalordninger ud fra kapitel 1-klassifikationen
alene. De kræver deres detaljerede regelspecifikke grundlag. De udtrykkeligt
skattepligtige § 19-undtagelser, fx aldersopsparing, bevarer deres behandling
som indkomst uden nyt AM; de bliver ikke ekstra pensionsfradrag.

**Migration:** Det afledte felt i `LønmodtagerPensionsfradrag` er omdøbt fra
`pbl19_rate_ophørende_bortseelsesret_før_am_kroner` til
`pbl19_rate_ophørende_arbejdsindkomst_før_am_kroner`, fordi hele grundlaget
ikke nødvendigvis har bortseelsesret. Efter-AM-feltet for bortseelsesret
indeholder nu kun det tilladte beløb. Ældre beregninger med rateoverskud skal
køres igen. Generer en ny skabelon og overfør gennemgåede kildefakta; ændr
ikke gamle kontrakthashes. Håndskrevne lavniveau- og delårsinput skal holde
rateoverskud adskilt fra bruttoløn og samtidig medtage det i øvrig personlig
indkomst uden nyt AM.

I den enkelte § 19-post er feltet
`bortseelsesberettiget_efter_indeholdt_arbejdsmarkedsbidrag_kroner` desuden
omdøbt til `bortseelsesgrundlag_før_årsgrænser_efter_am_kroner`. Det er en
foreløbig postklassifikation, **ikke** det endeligt tilladte årsbeløb. Brug
`pension.arbejdsgiver_rate_resultat.bortseelsesberettiget_efter_am_kroner`
for den samlede almindelige ratepensions bortseelsesret. Tidligere resultater
fra den alternative § 19-gren kan mangle fradrag eller fælles begrænsninger;
kør dem igen med den rettede model. De eksisterende selvstændige funktioner
`pbl18_årsresultat` og `pbl19_rate_årsresultat` bevarer deres argumenter; deres
interne `ÅrsSag`-konstruktører har fået to supplerende beløbsargumenter
(brutto og indeholdt AM). Foretræk funktionerne ved direkte lavniveaukald.

### Hvis du også får pension udbetalt

Indhent udbetalingernes beløb, år og art fra pensionsudbyderen. Efter LL § 9 L,
stk. 2, er det **årets** relevante skattepligtige udbetalinger, der reducerer
grundlaget for ekstra pensionsfradrag — men kun hvis der også var relevante
udbetalinger i det foregående år. Sidste års beløb er en betingelse, ikke det
beløb, som skal trækkes fra. Det behøver ikke være samme ordning eller udbyder.
Visse udbetalinger er undtaget, blandt andet bestemte invaliditets- og
efterladtepensioner. Undtagne udbetalinger sidste år udløser heller ikke i sig
selv modregning af almindelige udbetalinger i år.
[Lovforarbejderne til LL § 9 L, bemærkninger til nr. 6 og 7](https://www.retsinformation.dk/api/pdf/201576).

Et afgrænset 2026-eksempel med 50.000 kr. i fradragsberettiget indbetaling og
12 %-satsen viser forskellen (beløbene er fradrag, **ikke sparet skat**):

| Ikke-undtaget udbetaling sidste år | Ikke-undtaget udbetaling i år | Ekstra pensionsfradrag |
| --- | --- | --- |
| 40.000 kr. | 0 kr. | 6.000 kr. |
| 0 kr. | 30.000 kr. | 6.000 kr. |
| 10.000 kr. | 30.000 kr. | 2.400 kr. |

Årets skattepligtige pensionsudbetaling er fortsat indkomst, selv om den ikke
reducerer det ekstra pensionsfradrag. Indtast ikke samme sportspensionsrate
både under § 15 B og som øvrig § 20-udbetaling.

I `pension.udbetalingsresultat` vises årets ikke-undtagne beløb som
`samlet_modregningspligtig_pbl20_i_året_kroner` og det faktisk anvendte beløb
som `modregnet_efter_ligningslov9l_i_året_kroner`. De tidligere års felter
bevarer deres historiske betydning; `pbl20_udbetaling_status` i det afledte
pensionsfradragsinput beskriver det foregående års betingelse. Læs altid
gyldighedsvurderingen før beløbene bruges.

### Når pensionsoplysninger mangler

En tom udbetalingsliste er ikke en bekræftelse på, at du ikke har modtaget
pension. Under `lønmodtager.pension.udbetalingsoplysninger` starter begge
fuldstændighedsmarkeringer som `false`:

- `for_året_komplette`: Bekræft først, når årets relevante udbetalinger er
  gennemgået. Det omfatter også sportspension og eventuelle feriemidler efter
  PBL § 14 B i pensionsgrundlaget. En gennemgået tom liste kan bekræftes;
  en uoplyst liste kan ikke.
- `for_foregående_år_komplette`: Bekræft kun, hvis sidste års relevante
  § 20-udbetalinger og deres undtagelser er afklaret. Lad markeringen være
  `false`, hvis historikken stadig er ukendt eller ufuldstændig.

Futuruna kræver ikke automatisk endnu en årsopgørelse. I
`pension.oplysningsstatus` vises, om sidste års historik stadig kan ændre
det ekstra fradrag. Et dokumenteret ikke-undtaget beløb sidste år kan være
nok til at fastslå betingelsen. Historikken kan også være uden betydning,
fx ved ingen aktuelle modregningsrelevante udbetalinger, intet
fradragsberettiget indbetalingsgrundlag eller samme fradrag med og uden
modregning på grund af loftet. Det sidste kontrolleres med selve LL § 9 L-
reglen, inklusive loft og afrunding — ikke med en AI-vurdering.

`foregående_oplysninger_komplette` forbliver da `false`, selv om
`foregående_oplysninger_tilstrækkelige` er `true`. Det betyder **tilstrækkeligt
til denne beregning**, ikke at manglende historik er blevet til kendte nuller.
Hvis oplysningerne stadig er nødvendige, viser `vurdering.fejl` den konkrete
inputsti, og sammenligningsbeløbet er `null`. Samme regler gælder for en
beregnet ægtefælle. Den betingede rapportafstemning kræver fortsat ikke
ægtefællens dokumenter.

Modellen blev rettet 22. september 2026: tidligere blev sidste års beløb
fejlagtigt modregnet. Beregn berørte sager igen fra dokumenterede fakta med
den rettede model. Kontrakten har fået sporingsfelter og de eksplicitte
fuldstændighedsmarkeringer; gamle input kræver derfor gennemgang og migrering:
generer en ny skabelon med `template`, overfør kun gennemgåede inputfakta og
kør `call` igen. Overskriv ikke gamle resultater eller kontrakthashes.

## Beregn før og efter fra samme grundlag

Når de nødvendige kildefakta foreligger, brug den eksisterende
`beregn_personskat` i [personskat.calculate.runa](personskat.calculate.runa).
Der skal ikke skrives en ny skatteformel eller startes en Explore-strøm.

Fra projektets rod, med `PRIVATE_WORK_DIR` erstattet af din private mappe:

```sh
"$RUNA_BIN" template examples/danish-income-tax/personskat.calculate.runa --format json --output PRIVATE_WORK_DIR/pension-cases.json
# Udfyld og gennemgå fakta før beregningen.
FUTURUNA_CALCULATION_JOBS=1 "$RUNA_BIN" call examples/danish-income-tax/personskat.calculate.runa --input PRIVATE_WORK_DIR/pension-cases.json --output PRIVATE_WORK_DIR/pension-results.json
```

Én worker holder hukommelsesforbruget nede, også når de fire ægtefællesager
nedenfor samles i samme batch. Det ændrer ikke beregningsreglerne.

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

Du kan læse begge sagers gemte output med den
[lokale danske resultatvisning](personskat-validity.md#læs-dit-gemte-resultat-på-dansk)
(kræver allerede installeret Node.js 18 eller nyere):

```sh
node examples/danish-income-tax/personskat-resultat.mjs PRIVATE_WORK_DIR/pension-results.json
```

Den viser status, udvalgte beløb og alle vurderingens kontroller/forbehold,
men beregner ikke en før/efter-forskel eller rådighedsbeløb for dig.

Et brugbart svar viser:

| Beløb | Hvor kommer det fra? |
| --- | --- |
| Ændret indbetaling | Forskellen mellem de aftalte input, med år og ordning. |
| Tilladt privat ratefradrag og uudnyttet/afskåret beløb | `pension.pbl18_årsresultat`, som anvender de fælles årsgrænser. |
| Ekstra pensionsfradrag | `skat.ekstra_pensionsfradrag_kroner`. |
| Mindre modelleret skat for hovedpersonen | Før-skat minus efter-skat, fra begge gyldige sammenligningsbeløb i øre. Beløbet er ikke begge ægtefællers samlede skat. |
| Færre frie midler efter skat | Merindbetaling minus mindre skat; kun ved uændret øvrig indkomst og betalinger. Angiv om resultatet gælder personen eller husstanden; se nedenfor. |

»Mindre skat« er ikke nødvendigvis en større kontant tilbagebetaling på
årsopgørelsen. Forudbetalt skat og tidligere udbetalinger påvirker afregningen.
Sammenligningen værdisætter heller ikke fremtidig skat, pensionsafkast,
omkostninger, forsikringsdækning, binding eller indkomstafhængige ydelser.

### Din skat og husstandens skat er ikke det samme

`vurdering.slutskat_til_sammenligning_øre` gælder **hovedpersonen i den
pågældende sag**, også når ægtefællefakta indgår. En privat pensionsbetaling
kan ændre, hvor meget uudnyttet personfradrag der overføres, og dermed
ægtefællens skat. Nul ændring i din egen skat betyder derfor ikke nødvendigvis
nul skattevirkning for husstanden. Overførsel af uudnyttet skatteværdi af
personfradrag er et særskilt sambeskatningselement, som kræver samliv ved årets
udløb; se [Den juridiske vejledning C.A.8.2, PSL §§ 9–10](https://info.skat.dk/data.aspx?oid=1976883).

Hvis spørgsmålet gælder **begge personers økonomi**, og de nødvendige fakta
foreligger, skal der være fire sammenhængende beregninger:

| Sag | Hovedperson | Ægtefællens fakta |
| --- | --- | --- |
| A før | A før ændringen | B før ændringen |
| A efter | A med ændret pensionsbetaling | B efter ændringen |
| B før | B før ændringen | A før ændringen |
| B efter | B efter ændringen | A med ændret pensionsbetaling |

Samme persons fakta skal stemme på tværs af hovedperson/ægtefælle. Også
**B efter** skal medtage A's ændrede betaling, selv når B's løn og egne
pensionsbetalinger er uændrede. Beregn begge før/efter-forskelle fra de fire
gyldige sammenligningsbeløb. Husstandens skattebesparelse er summen af
forskellene; ved uændret øvrig økonomi er faldet i husstandens frie midler
merindbetalingen minus denne sum. Mangler blot ét nødvendigt resultat,
tilbageholdes husstandssammenligningen — `null` må ikke blive til nul.

Dette er **ikke** en generel instruktion om blindt at bytte to JSON-felter.
Tabsfremførsel, udenlandsk lempelse, ejerskab, fælles fordelinger og
betalingsafregning kan have personbundne roller uden for de otte fælles
personfaktafelter. Gennemgå disse kilder og modellens dækning for hver
orientering. Brug ikke afrundede delbeløb fra `ægtefælle.skat` som erstatning
for en gyldig, eksakt beregning med B som hovedperson.

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
node examples/danish-income-tax/pension-demo.mjs "$RUNA_BIN"
```

Brug den samme `runa`-binær, der bestod kompatibilitetstjekket.
Scriptet kræver Node.js 18 eller nyere, men Node er ikke nødvendigt for den
almindelige `runa template`/`runa call`-arbejdsgang. Installer ikke ekstra
software blot for dette valgfrie eksempel uden brugerens accept.

Eksemplets person er født 1. januar 1990, har 600.000 kr. i kontant bruttoløn
i 2026, bor i København, betaler ikke kirkeskat og har ingen ægtefælle,
ejendom, anden indkomst, pensionsudbetalinger eller andre fradragsudgifter.
Personen er også udtrykkeligt DBO-hjemmehørende i Danmark hele året. Det er
en særskilt fiktiv oplysning, ikke noget scriptet udleder af adressen.
De øvrige fravalg er udtrykkelige **fiktive forudsætninger**. De to øvrige
udlandsbetingelser står som uoplyste, fordi de ikke kan ændre denne afgrænsning.
Brug ikke eksemplets forudsætninger som standardværdier for en rigtig person.

Scriptet viser mindre/mere privat indbetaling, en separat fast
arbejdsgiverordning og en sag med uoplyste udgifter. Det kører med én arbejder
og gemmer input, fulde resultater, kontraktfingeraftryk og en kort opsummering
i en ny midlertidig mappe uden for projektet. Det ændrer ingen eksisterende
filer. Den sidste sag skal have manglende sammenligningsbeløb, ikke nul skat.

Terminalen viser danske kronebeløb og retningen: mere/mindre indbetalt,
mere/mindre skat og flere/færre frie midler. `summary.json` bevarer de
maskinlæsbare ørebeløb, gyldighed og forbehold. En uafklaret sag giver ingen
sammenligning, ikke en skatteændring på nul.

Det observerede resultat, genkørt med modellen den 25. september 2026:

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

### Et fiktivt par: uændret egen skat, men mindre skat hos ægtefællen

Det separate [pension-par-demo.mjs](pension-par-demo.mjs) gennemfører de fire
kanoniske beregninger ovenfor for ét fast fiktivt par. Fra projektets rod,
med den samme verificerede compiler og allerede installeret Node.js 18+:

```sh
node examples/danish-income-tax/pension-par-demo.mjs "$RUNA_BIN"
```

Begge er født 1. januar 1990, fuldt skattepligtige og DBO-hjemmehørende i
Danmark hele 2025, har skattekommune København, ingen kirkeskat og er gift
og samlevende ved årets udgang. A har 50.000 kr. i løngrundlag og ændrer
en privat ratebetaling fra 0 til 10.000 kr. B har uændret løngrundlag
600.000 kr. og ingen pensionsbetalinger. Ingen har ATP, andre indkomster,
fradragsudgifter, ejendom, pensionsudbetalinger, fremførte tab eller
udenlandsk lempelse. Årets og sidste års udbetalingshistorik er bekræftet tom.
Disse er udtrykkeligt **opdigtede fakta**, ikke standardvalg for rigtige brugere.

Observeret kanonisk modeloutput den 25. september 2026:

| Modelleret skat inklusive løn-AM | Før | Efter | Mindre skat |
| --- | ---: | ---: | ---: |
| A | 4.000,00 kr. | 4.000,00 kr. | 0,00 kr. |
| B | 208.510,73 kr. | 204.677,73 kr. | 3.833,00 kr. |
| Begge tilsammen | 212.510,73 kr. | 208.677,73 kr. | 3.833,00 kr. |

Husstandens frie midler falder derfor med **6.167,00 kr.** ved den ekstra
indbetaling på 10.000 kr. Et svar baseret alene på A's skat ville overse
ægtefællevirkningen. Beløbet beskriver årets modellerede skat og private
betaling, ikke en lovet ændring af tilbagebetaling eller samlet pensionsafkast.
De fire resultater er `BeregnetMedForbehold`, ikke uafhængigt verificerede
matches med SKAT eller bevis for fuld lovdækning.

Scriptet kører med én worker og gemmer `cases.json`, uændret `results.json`,
`summary.json`, den korte `sammenligning.txt` samt `resultat.txt` med alle
returnerede vurderinger, kontroller og forbehold i en ny midlertidig mappe.
Det genbruger konstruktion fra det fiktive bilagseksempel, men **ikke** dets
kildelog eller enlige profils eksterne forventninger. Det er ikke en
importør eller en generel husstandsberegner til personlige sager. Ingen
skatteformler er duplikeret i scriptet.

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
| Renter | Årsoversigt, faktisk renteudgift og egen hæftelse/andel; ikke lånets afdrag. Se [input og ekstern kontrol](skatdk-rentefradrag-ekstern.md). |
| Service eller grønt håndværk | Arbejdets art/dato, faktura, løn/materialer, betaling, bolig og eventuel fordeling. Start gerne med den [lille fakturaberegning](boligjob.md). |
| Gaver | Modtager, godkendelsesgrundlag, kvittering og indberetning. Bindende løbende ydelser kræver også aftalereference og årets samlede forpligtelse; [delbetalinger deler aftaleloftet](personskat-gaver.md). |
| Børne- eller ægtefællebidrag | Aftale/afgørelse, faktisk betaling, forfald og rette modtager. [Barnets indkomst og forælderens fradrag](personskat-underholdsbidrag.md) tilhører forskellige opgørelser. |
| Enlig forsørger | Oplysninger om ekstra børnetilskud og relevante kvartaler, ikke alene civilstand. Se [kildemodellen](ligningsloven-par9j-enlig.md). |
| Arbejdsrejser eller andre arbejdsudgifter | Arbejdssted, periode, udgiftstype, bilag og arbejdsgiverens betaling/godtgørelse; undersøg den specifikke regel. |

For faglige kontingenter beskriver `skatteyderstatus` **dig som skatteyder**,
ikke den, der betaler regningen. Arbejdsgiverbetaling gør ikke en lønmodtager
til `Ll13JuridiskPerson`; den status afvises ved aktive kontingenter i en
personlig beregning. Den almindelige lønmodtagergrænse er 6.000 kr. i 2023
og 7.000 kr. fra 2024. A-kassebidrag skal ikke lægges ind under det loft;
de har deres egen regel og inputgren. Ret ikke status for at få et større
fradrag. [SKATs juridiske vejledning](https://info.skat.dk/data.aspx?oid=2061770).

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
