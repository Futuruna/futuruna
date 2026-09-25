# Personskatteloven i Futuruna

Denne side er på dansk, fordi loven og dens retskilder er danske. Siden er
projektets ene webviste overblik. Lovtekst, regler, scenarier og audits ligger i
selve Futuruna-projektet under `examples/danish-income-tax/`.

## Begynd med dit eget spørgsmål

Du kan starte med »stemmer min årsopgørelse?«, »hvad ændrer en større eller
mindre pensionsindbetaling?« eller »hvilke fradrag bør jeg undersøge?«.
[Pension og fradrag](https://github.com/Futuruna/futuruna/blob/main/examples/danish-income-tax/pension-og-fradrag.md)
viser de relevante spørgsmål, en konkret før/efter-beregning og hvilke bilag
der er nødvendige. Du behøver ikke begynde med hele skattearbejdsbogen.

Har du både løn og dagpenge, viser [dagpenge og kørselsfradrag](https://github.com/Futuruna/futuruna/blob/main/examples/danish-income-tax/personskat-dagpenge.md)
en lille beregning med dokumenterede indkomstarter og synlige ukendte forhold.
For studerende adskiller [SU og studiejob](https://github.com/Futuruna/futuruna/blob/main/examples/danish-income-tax/personskat-su.md)
stipendium, lån og løn, så SU ikke får lønnens AM-bidrag og arbejdsfradrag.
For folkepensionister viser [folkepension og tillæg](https://github.com/Futuruna/futuruna/blob/main/examples/danish-income-tax/personskat-folkepension.md),
hvilke dækkede udbetalinger der indgår i skatten, og hvilke tillæg der er skattefri.
[Førtids-, senior- og tidlig pension](https://github.com/Futuruna/futuruna/blob/main/examples/danish-income-tax/personskat-socialpension.md)
har en særskilt indgang med opdeling af gamle førtidspensionsreglers skattepligtige og skattefri dele.

Mangler din ægtefælles oplysninger, kan du stadig
[afstemme din egen årsopgørelse betinget](https://github.com/Futuruna/futuruna/blob/main/examples/danish-income-tax/aarsopgoerelse-afstemning.md).
Resultatet viser blandt andet, hvilke overførsler der skulle være til stede,
men beviser ikke de ukendte forhold. Modellerne er forskningssoftware, ikke
individuel skatterådgivning.

## Et samlet sprog til lov og ret

Futuruna lader den juridiske tekst og den eksekverbare model bo tæt på
hinanden. Hver lovsektion følger som udgangspunkt den samme struktur:

1. Den officielle danske lovtekst gengives ordret i en flerlinjekommentar.
2. En særskilt note tilføjes kun, når fortolkning eller kildehistorik kræver
   det.
3. De faktiske regler formuleres med tydelige navne og typede domæneobjekter.

Regelformen `|` bruges som det normale juridiske udsagn. `under` gør
betingelser synlige, og `exception` gør undtagelser synlige. Resultatet er
ikke en løs samling skatteformler, men en kæde af regler, hvor hvert
mellemresultat kan kontrolleres og genbruges.

Kilder er knyttet til de relevante kodeområder med typede metadata. En audit
kan derfor undersøge både resultatet, den anvendte regel og den retskilde, som
reglen bygger på.

## Personskatteloven og den samlede indkomstskat

Dansk personskat er ikke én formel. Personskatteloven er kernen, men en virkelig
beregning afhænger blandt andet af arbejdsmarkedsbidrag, kommunal skat,
kirkeskat, Kildeskatteloven, Ligningsloven, aktie- og kapitalindkomst,
ægtefælleregler, underskud, ejendomsskatter, pension og slutopgørelse.

Futurunas offentlige beregningsregel `beregn_personskat` samler disse dele i
én typet regelgraf. Inputtet består så vidt muligt af observerbare kildefakta:
beløb, datoer, ejerforhold, dispositioner og dokumenterede valg. Nogle grene
kræver også en eksplicit klassifikation, fx pensionsordningens eller en
udbetalings retlige type. Et menneske eller en AI skal kunne begrunde det valg
i kildematerialet; modellen udleder ikke alle klassifikationer fra en PDF.
Uafklarede forhold skal undersøges eller forblive ukendte, ikke gættes.

Regelgrafen fører fakta gennem de juridiske mellemresultater og frem til en
slutskat i eksakte øreenheder og, når de nødvendige forudbetalinger er oplyst,
en årsopgørelse. De modellerede inputkontroller kan tilbageholde
sammenligningsbeløbet ved ugyldige eller uafklarede fakta. De opdager ikke
automatisk enhver udeladt indkomst, forkert klassifikation eller usand oplysning.
Eksakte enheder beviser heller ikke, at alle myndighedens afrundinger er gengivet.

Korpusset omfatter også de dele af andre love, som den kanoniske beregning
afhænger af. Det betyder ikke, at dansk skatteret bliver statisk: nye
ændringslove, satser og praksis skal fortsat versionsbindes. Det betyder, at
den implementerede beregningsvej er samlet, sporbar og kan udvides uden en
separat beregningsmotor.

## Fra interview til deterministisk resultat

`@ calculate("Dansk personskat")` udstiller den typede
`PersonskatInput`-grænse. Ud fra den samme Futuruna-kode kan værktøjet:

1. generere en XLSX-arbejdsbog, et JSON-dokument eller en TOML-skabelon,
2. vise menneskelige danske etiketter, spørgsmål, hjælp, enheder og
   valgmuligheder,
3. placere gentagne oplysninger som børn, aktiver, indbetalinger og hændelser
   i relationelle tabeller,
4. validere den udfyldte kontrakt, og
5. beregne det fulde typede resultat deterministisk.

Det tilsigtede menneske-maskine-forløb er, at en AI interviewer borgeren og
udfylder arbejdsbogen. AI'en må gerne hjælpe med at læse dokumenter og stille
opfølgende spørgsmål, men den skal ikke gætte skatten. Futuruna ejer
valideringen, regelfølgen og beregningen.

Skabelonens nulbeløb, tomme lister og første valgmuligheder er pladsholdere,
ikke bekræftede fakta. Brug kontraktens spørgsmål, hjælp, enheder og
kildehenvisninger til at afklare de relevante felter. Kendt nul og ukendt er
forskellige ting; hvis feltet ikke kan udtrykke den aktuelle usikkerhed, må
en udfyldt skabelon ikke præsenteres som en uafhængig beregning af personens skat.

Der er bevidst ingen automatisk PDF-importør. Et menneske eller en AI
transskriberer kildefakta. I den uafhængige beregning bruges myndighedens
beregnede resultat kun til sammenligning. Den særskilte betingede afstemning
bruger derimod udtrykkeligt rapportens observationer og kan vise nødvendige
overførsler; de må ikke bagefter behandles som uafhængigt dokumenterede fakta.
Den samme udfyldte sag kan køres igen og forklares ud fra lovkoden og de
angivne forbehold.

## Eksempel: offentlig beregning fra Skattestyrelsen

Et kildebelagt scenarie bruger Skattestyrelsens offentlige 2026-beregner for en
enlig lønmodtager i København med 600.000 kr. i årsløn og uden kirkeskat.
Futuruna genberegner de offentliggjorte mellemresultater og rammer 48.000 kr. i
arbejdsmarkedsbidrag, 552.000 kr. i personlig indkomst og 208.726 kr. i samlet
skat inklusive arbejdsmarkedsbidrag efter afrunding.

Det samme scenarie beregner skattekortet til 36 procent og et månedsfradrag på
8.164 kr., svarende til den observerede offentlige beregning. Testen holder
kildens input og output adskilt, så myndighedens resultat ikke kan blive brugt
som genvej i selve lovberegningen.

## Audit: mere end 100 procent

Den samme kode kan undersøges som et regelsystem i stedet for kun at blive kørt
med én borgers fakta. En afgrænset audit gennemløber 8.064 kombinationer af
indkomst, kommune, kirkeskat, kapital- og aktieindkomst, ægtefælleforhold og
overført restskat.

Auditten fandt ingen konfiguration, hvor selve årets beregnede skat oversteg
100 procent af det positive indkomstgrundlag. Den fandt derimod mere end 200
konfigurationer, hvor årets samlede betalingsbelastning oversteg 100 procent.
Alle disse fund krævede overført restskat fra et tidligere år.

Det er den afgørende juridiske forskel: Fundene viser ikke en ordinær årlig
skattesats over 100 procent. De viser, at betaling af årets skat sammen med
gammel, endnu ikke betalt skat kan overstige årets aktuelle indkomst. Auditkoden
bevarer begge mål, så en dramatisk søgning ikke bliver til en forkert juridisk
konklusion.

Audits ligger i `.audit.runa`-filer, mens konkrete regressions- og
virkelighedssager ligger i `.scenario.runa`-filer. De kan blandt andet
kontrollere bevarelse af beløb, tidsmæssige lovskift, modstridende
klassifikationer og usædvanlige regelkaskader.

## Status

Den kanoniske kontrakt omfatter hovedperson og ægtefælle, relationelle
kildefakta og en slutopgørelse for dækkede forhold. Beregningsgrænsefladen
`@ calculate` samt `schema`, `template` og `call` er Preview, og skattemodellen
er forskningssoftware. De konkrete scenarier ovenfor er afgrænset evidens,
ikke verifikation af vilkårlige danske årsopgørelser.

Læs altid resultatets `vurdering`. `BeregnetMedForbehold` betyder, at de
opregnede kontroller bestod, ikke at alle forhold eller regler er dækket.
`UgyldigtBeregningsgrundlag` tilbageholder sammenligningsbeløbet; brug da ikke
de øvrige tal som en pålidelig skat. Et `BetingetAfstemt` rapportresultat er
heller ikke en uafhængig godkendelse.

Den vedligeholdte [status for skatteaudit](https://github.com/Futuruna/futuruna/blob/main/docs/tax-audit-readiness.md)
angiver verificerede forløb, afrundingsusikkerhed, kendte begrænsninger og
offentlig leveringsstatus. Rettelser i repoet er ikke i sig selv en ny offentlig
binær eller en opdateret hjemmeside. Brug den compiler, der består
[runtime-tjekket](https://github.com/Futuruna/futuruna/blob/main/website/public/ai-setup.md#tax-audit-runtime-check),
og generér input fra den samme modelversion.

Futuruna-koden er den autoritative projektflade. Denne side opsummerer
metoden og de verificerede resultater, men forsøger ikke at gengive hele
lovkorpusset som en webartikel.

## Kilder

Den aktuelle arbejdskilde for Personskattelovens konsoliderede tekst er
[LBK nr. 1284 af 14. juni 2021](https://www.retsinformation.dk/eli/lta/2021/1284)
med særskilt sporede ændringslove, ikrafttrædelser og årssatser.
[LBK nr. 799 af 7. august 2019](https://www.retsinformation.dk/eli/lta/2019/799)
bevares som historisk kildelinje.

Afhængige bestemmelser hentes fra deres officielle Retsinformation-kilder.
Satser og administrativ praksis bindes særskilt til Skatteforvaltningens eller
Skatteministeriets officielle materiale. Hver relevant lovtekst bevares
ordret ved reglerne, så kilde, oversættelse og beregning kan revideres samlet.

Projektet er forskning og software, ikke individuel skatterådgivning.
