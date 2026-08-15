# Personskatteloven i Futuruna

Denne side er på dansk, fordi loven og dens retskilder er danske. Siden er
projektets ene webviste overblik. Lovtekst, regler, scenarier og audits ligger i
selve Futuruna-projektet under `examples/danish-income-tax/`.

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
beløb, datoer, ejerforhold, dispositioner og dokumenterede valg. Borgeren eller
en AI skal ikke selv konkludere, hvilken paragraf eller skatteart der gælder.
Det udleder reglerne.

Regelgrafen fører fakta gennem de juridiske mellemresultater og frem til en
ørenøjagtig slutskat og, når de nødvendige forudbetalinger er oplyst, en
årsopgørelse. Ufuldstændige eller modstridende fakta fejler lukket i stedet for
at blive udfyldt med en skjult skattemæssig antagelse.

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

Der er bevidst ingen automatisk PDF-importør. Et menneske eller en AI
transskriberer kildefakta; myndighedens beregnede resultat bruges kun som en
uafhængig kontrol. Derefter kan den samme udfyldte sag køres igen, auditeres og
forklares ud fra lovkoden.

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

Den første publicerbare Personskat-model består af 352 Futuruna-filer, heraf
185 scenariefiler og 38 auditfiler. Den kanoniske kontrakt dækker både
hovedperson og ægtefælle, relationelle kildefakta og den samlede
slutopgørelse. De kendte beløbsmæssige huller i det nuværende korpus er
implementeret, og publiceringskonformiteten er verificeret. Det løbende arbejde
efter en udgivelse er kildevedligeholdelse samt udvidelse til nye eller endnu
ikke modellerede retsforhold.

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
