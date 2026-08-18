# Læserens blik: 490 indkomstklinter

Dette dokument fastholder læserens vej gennem [Jeg fandt 490 tilfælde, hvor én
ekstra krone gjorde dig fattigere](personskat-income-cliffs.md). Artiklen skal
kunne læses af en nysgerrig dansker, som aldrig har set Futuruna-kode eller den
kodede personskattemodel før.

## Læseren

Læseren kommer med fire spørgsmål:

1. Hvor ofte kostede én ekstra krone mere, end den gav?
2. Hvordan kan så få linjer kode stille det spørgsmål til en hel lovmodel?
3. Hvor store var tabene, og hvordan fordelte de sig?
4. Hvilken regel skaber resultatet, og hvor langt rækker konklusionen?

Læseren skal have svaret, programmet og grafen tidligt. Juraen og
afgrænsningen skal derefter forklare resultatet uden at sløre det.

## Læserens rejse

| Afsnit | Læseren spørger | Afsnittet leverer | Læseren ved bagefter |
|---|---|---|---|
| Åbningen | Hvor mange gange gik regnestykket baglæns? | Spørgsmål, metode og hovedtal | Alle 490 undersøgte kombinationer gav et tab på 69,23–170,02 kr. |
| Spørgsmålet, skrevet som kode | Hvordan kan programmet spørge sådan? | Det korte filter, `?`-kontrollen og de 980 beregninger | Programmet genbruger den eksisterende lovmodel og beholder de tilfælde, hvor beløbet efter skat falder |
| Hvad kostede den ekstra krone? | Hvordan fordeler tabene sig? | Et histogram med fire ikke-overlappende intervaller og en præcis datatabel | Søjlerne er 0, 245, 16 og 229; transportprofilerne danner to tydelige grupper |
| Det dyreste tilfælde | Hvordan ser det største tab ud i kroner og øre? | Læsø-regnestykket før og efter | Én ekstra lønkrone giver 171,02 kr. mere i beregnet skat og 170,02 kr. mindre tilbage |
| Hvorfor sker det? | Hvilken regel skaber klinten? | § 9 C, hele tusinder og de to transportgrene | Lavindkomsttillægget falder i 50 trin, og et eksisterende fradrag kan derfor blive mindre på én gang |
| Hvad undersøgte programmet? | Hvad betyder de 490? | 392 kommunale sammenligninger og 98 ekstra trin | De 490 er kombinationer af skatteprofil og indkomsttrin, ikke 490 forskellige regler eller tilfældige lønstigninger |
| Sådan vender Futuruna loven på vrangen | Hvad er den nye arbejdsmåde? | Beregningens retning før og efter | Den samme lovmodel kan besvare et helt søgekort ved at ændre spørgsmålet, ikke selve loven |
| Hvad resultatet dækker | Gælder det alle danskere? | Faste fakta, geografisk afgrænsning og resultatmål | Resultatet gælder de 490 standardiserede modeltilfælde og er ikke en befolkningsmåling eller individuel rådgivning |
| Kør søgningen selv | Kan jeg efterprøve det? | Kildekode, kommandoer og målt tid | Programmet kan læses og køres; den målte søgning tog 4 minutter og 23 sekunder |
| Hvad spørger vi om næste gang? | Hvad åbner metoden for? | En fremadrettet tanke og kontakt | Samspillet mellem regler er næste oplagte sted at lede |

## Begreber i den rækkefølge, de bruges

- **Indkomstklint:** Et punkt, hvor en højere indkomst før skat giver et lavere
  beløb efter skat.
- **Tilbage efter skat:** Bruttoårsløn minus den personskat, modellen beregner,
  opgjort i øre.
- **Histogram:** Fire søjler, hvor hvert af de 490 resultater tælles én gang
  efter tabets størrelse.
- **Lavindkomsttillæg:** Et tillæg til kørselsfradraget; ikke en kontant
  udbetaling på samme beløb.
- **Indkomsttrin:** Et skift, når indkomsten over grænsen når endnu et helt
  tusinde kroner.
- **Skatteprofil:** De faste oplysninger, som kommunen, kirkeskatten og
  transportafstanden udgør i søgningen.

Tekniske ord fra programudviklingen skal ikke bruges som erstatning for disse
begreber. Artiklen siger *tilfælde*, *konkret eksempel*, *det største tab* og
*den fulde personskatteberegning*.

## Grafens læsekontrakt

Histogrammet viser fire forskellige intervaller:

| Tab efter den ekstra krone | Antal |
|---|---:|
| Under 50 kr. | 0 |
| 50–99,99 kr. | 245 |
| 100–149,99 kr. | 16 |
| 150–199,99 kr. | 229 |

Intervallerne overlapper ikke, og summen er 490. x-aksen er tabets størrelse;
y-aksen er antallet af sammenligninger. Grafen må ikke læses som en opgørelse
over danskere. Den viser de skatteprofiler og indkomsttrin, programmet blev
bedt om at undersøge.

## Kilder, model og resultat

Læseren skal kunne skelne mellem tre ting:

1. De officielle kilder fastlægger 2026-grænserne, satserne,
   kommunesatserne og nedtrapningen i hele tusinder.
2. Futuruna-modellen omsætter disse regler og de faste oplysninger til en
   personskatteberegning før og efter hvert trin.
3. Søgeprogrammet tæller de 490 resultater, fordeler dem i histogrammet og
   finder det mindste og største tab.

Den landsdækkende del dækker første trin for de valgte akser. Kun profilerne for
København og Læsø følger alle 50 trin. Transportafstandene er standardiserede
modeloplysninger, ikke beskrivelser af virkelige pendlere.

## Tone

- Svar før forbehold.
- Skriv i første person, når forfatteren fortæller, hvad han gjorde og fandt.
- Lad tallene gøre opdagelsen interessant; undgå salgssprog.
- Forklar hvert fagord dér, hvor læseren får brug for det.
- Brug et kort forbehold, når det ændrer betydningen af resultatet; undgå en
  katalogtekst over alt, programmet ikke undersøger.
- Slut med det næste spørgsmål, ikke med et resumé.
