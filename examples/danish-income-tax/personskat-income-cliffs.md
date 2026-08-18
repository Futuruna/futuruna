# Jeg fandt 490 tilfælde, hvor én ekstra krone gjorde dig fattigere

Jeg ville stille den danske skattelov et meget enkelt spørgsmål:

**Hvor mange gange kan du tjene én krone ekstra og alligevel have færre penge
tilbage efter skat?**

Så jeg skrev et lille Futuruna-program. Det tager en bruttoårsløn lige før og
lige efter et trin i loven, beregner den fulde personskat begge gange og gemmer
de tilfælde, hvor regnestykket går baglæns.

Programmet undersøgte **490 kombinationer af skatteprofil og indkomsttrin**.
Resultatet var **490 ud af 490**. I hvert eneste af de undersøgte tilfælde
kostede den ekstra krone mere, end den gav: Tabet var mellem **69,23 og 170,02
kr.**

Jeg kalder sådan et punkt en *indkomstklint*: Du tjener mere før skat, men har
mindre tilbage efter skat.

## Spørgsmålet, skrevet som kode

Kernen i søgningen er ganske kort:

```runa
= personskat_indkomstklinter = filter(
    personskat_ligningsfradrag_gyldige_indkomstovergange,
    |overgang: PersonskatIndkomstovergang|
        overgang.netto_efter_øre < overgang.netto_før_øre
)

| personskat_indkomstklint_alle_490_fundet:
    personskat_indkomstklint_fundantal
    -> personskat_indkomstklint_fundantal == 490

? personskat_indkomstklint_alle_490_fundet
```

Det er hele spørgsmålet: Behold de tilfælde, hvor der er færre øre tilbage
efter lønstigningen end før, og kontrollér hvor mange der er.

Resten af programmet bygger de 490 sammenligninger og sender begge sider
gennem den samme fulde personskatteberegning. Hver sammenligning kræver én
beregning før og én efter — **980 fulde skatteberegninger i alt**. Det korte
søgeprogram genbruger altså den lovmodel, der allerede findes. Det skriver ikke
skatteloven en gang til.

## Hvad kostede den ekstra krone?

<figure class="income-cliff-histogram" aria-labelledby="income-cliff-histogram-caption">
  <figcaption id="income-cliff-histogram-caption">
    <strong>Alle 490 sammenligninger gik baglæns.</strong>
    Søjlerne viser, hvor mange tilfælde der gav et tab inden for hvert interval.
  </figcaption>
  <svg viewBox="0 0 720 430" aria-hidden="true" focusable="false">
    <g class="income-cliff-histogram-grid">
      <line x1="88" y1="340" x2="688" y2="340"></line>
      <line x1="88" y1="280" x2="688" y2="280"></line>
      <line x1="88" y1="220" x2="688" y2="220"></line>
      <line x1="88" y1="160" x2="688" y2="160"></line>
      <line x1="88" y1="100" x2="688" y2="100"></line>
      <line x1="88" y1="40" x2="688" y2="40"></line>
    </g>
    <g class="income-cliff-histogram-ticks">
      <text x="76" y="345">0</text>
      <text x="76" y="285">50</text>
      <text x="76" y="225">100</text>
      <text x="76" y="165">150</text>
      <text x="76" y="105">200</text>
      <text x="76" y="45">250</text>
    </g>
    <g class="income-cliff-histogram-bars">
      <rect x="112" y="340" width="112" height="0"></rect>
      <rect x="258" y="46" width="112" height="294"></rect>
      <rect x="404" y="321" width="112" height="19"></rect>
      <rect x="550" y="65" width="112" height="275"></rect>
    </g>
    <g class="income-cliff-histogram-values">
      <text x="168" y="326">0</text>
      <text x="314" y="34">245</text>
      <text x="460" y="309">16</text>
      <text x="606" y="53">229</text>
    </g>
    <g class="income-cliff-histogram-labels">
      <text x="168" y="367"><tspan x="168">Under</tspan><tspan x="168" dy="18">50 kr.</tspan></text>
      <text x="314" y="367"><tspan x="314">50–99,99</tspan><tspan x="314" dy="18">kr.</tspan></text>
      <text x="460" y="367"><tspan x="460">100–149,99</tspan><tspan x="460" dy="18">kr.</tspan></text>
      <text x="606" y="367"><tspan x="606">150–199,99</tspan><tspan x="606" dy="18">kr.</tspan></text>
    </g>
    <text class="income-cliff-histogram-x-title" x="388" y="420">Tab efter skat ved 1 kr. ekstra i bruttoårsløn</text>
    <text class="income-cliff-histogram-y-title" transform="translate(20 190) rotate(-90)">Antal sammenligninger</text>
  </svg>
  <div class="visually-hidden">
    <table>
      <caption>Dataene bag histogrammet</caption>
      <thead>
        <tr><th scope="col">Tab efter skat</th><th scope="col">Antal sammenligninger</th></tr>
      </thead>
      <tbody>
        <tr><td>Under 50 kr.</td><td>0</td></tr>
        <tr><td>50–99,99 kr.</td><td>245</td></tr>
        <tr><td>100–149,99 kr.</td><td>16</td></tr>
        <tr><td>150–199,99 kr.</td><td>229</td></tr>
      </tbody>
    </table>
  </div>
</figure>

Hver sammenligning er talt præcis én gang. De 245 resultater med 60 kilometers
daglig transport ligger alle mellem 69,23 og 90,52 kr. De 245 resultater med
130 kilometer ligger mellem 144,08 og 170,02 kr.; 16 af dem er under 150 kr.,
mens 229 er på mindst 150 kr.

Den tydelige todeling kommer altså især fra de to transportprofiler. Den er
ikke et billede af, hvor mange danskere der befinder sig hvert sted. Den viser
fordelingen i de 490 kombinationer, programmet blev bedt om at undersøge.

## Det dyreste tilfælde

Det største tab i søgningen var **170,02 kr.** Det samme maksimum optrådte 41
gange. Her er et af tilfældene, så regnestykket kan ses helt konkret:

- Kommune: **Læsø**
- Kirkeskat: **ja**
- Transport: **130 km i alt pr. arbejdsdag**, 203 arbejdsdage
- Bruttoårsløn: **342.499 → 342.500 kr.**

| Beløb | Før | Efter | Ændring |
|---|---:|---:|---:|
| Bruttoårsløn | 342.499,00 kr. | 342.500,00 kr. | **+1,00 kr.** |
| Lavindkomsttillæg til kørselsfradraget | 30.800 kr. | 30.184 kr. | **-616 kr.** |
| Beregnet personskat | 88.526,60 kr. | 88.697,62 kr. | **+171,02 kr.** |
| Tilbage efter skat | 253.972,40 kr. | 253.802,38 kr. | **-170,02 kr.** |

Du tjener **1 kr. mere**. Modellen beregner **171,02 kr. mere i skat**. Derfor
er der **170,02 kr. mindre tilbage**:

```text
+1,00 kr. i løn - 171,02 kr. i ekstra skat = -170,02 kr.
```

Her betyder *tilbage efter skat* helt konkret bruttoårsløn minus den personskat,
Futuruna-modellen beregner. Transportudgifter, boligstøtte, børneydelser,
forbrugsskatter og resten af husholdningsøkonomien er ikke med.

Københavnseksemplet ved det første trin kostede 69,47 kr. Det var altså hverken
en ener eller det største tilfælde.

## Hvorfor sker det?

Forklaringen ligger i lavindkomsttillægget til kørselsfradraget. I 2026 bliver
tillægget [trappet ned mellem 341.500 og 391.500
kr.](https://skat.dk/borger/fradrag/koerselsfradrag/koerselsfradrag-befordringsfradrag),
og det maksimale tillæg er 30.800 kr.

[Ligningslovens § 9 C, stk.
4](https://www.retsinformation.dk/eli/lta/2025/1500) reducerer tillæggets
procentsats med 1,28 procentpoint og maksimum med 2 procent for hver 1.000 kr.
over indkomstgrænsen. [Lov nr. 616 af 30. juni
2026](https://www.retsinformation.dk/eli/lta/2026/616) forhøjede
kilometersatserne for 2026 og fordoblede årets maksimumsbeløb. Grundsatserne
for 2026 står i
[bekendtgørelse nr. 1333 af 20. november
2025](https://www.retsinformation.dk/eli/lta/2025/1333).

Ordene *for hver 1.000 kr.* er afgørende. I et officielt [regneeksempel for en
ugift skatteyder](https://skm.dk/tal-og-metode/satser/skatte-og-afgiftsberegning/skatteberegningseksempel-for-en-ugift-skatteyder-i-2023)
opgør Skatteministeriet indkomsten over grænsen i hele antal tusinde kroner.
Futuruna-modellen bruger samme trinvise læsning.

I netop disse lønprofiler følger aftrapningsindkomsten bruttolønnen krone for
krone, fordi de andre indkomsttyper i beregningen er sat til nul.

Det giver 50 skarpe trin:

```text
342.499 → 342.500 kr.
343.499 → 343.500 kr.
...
391.499 → 391.500 kr.
```

Når lønnen krydser et trin med én krone, kan flere hundrede kroner af et
eksisterende fradrag forsvinde på én gang. Det betyder ikke, at hele
fradragsfaldet bliver trukket fra din konto. Fradraget bliver mindre, og den
fulde personskatteberegning afgør, hvad det koster efter skat.

Ved 60 kilometer i alt pr. arbejdsdag styres tillægget af en procentdel af det
almindelige kørselsfradrag. Ved 130 kilometer i alt pr. arbejdsdag har
tillægget nået sit maksimum, og hvert trin fjerner 616 kr. Det er
hovedforklaringen på de to grupper i histogrammet.

## Hvad undersøgte programmet?

De 490 er ikke 490 forskellige paragraffer. De er 490 kombinationer af et
relevant indkomsttrin og en fast skatteprofil.

Først undersøgte programmet det første trin i alle 98 kommuner, med og uden
kirkeskat og med 60 eller 130 kilometer i alt pr. arbejdsdag:

**98 kommuner × 2 kirkestatusser × 2 transportafstande = 392 sammenligninger.**

Derefter fulgte programmet de resterende 49 trin for to faste profiler,
København uden kirkeskat ved 60 kilometer og Læsø med kirkeskat ved 130
kilometer:

**49 trin × 2 profiler = 98 sammenligninger mere.**

| Del af søgningen | Sammenligninger | Personskatteberegninger | Resultat |
|---|---:|---:|---:|
| Første trin i alle 98 kommuner | 392 | 784 | 392 indkomstklinter |
| De resterende trin for København og Læsø | 98 | 196 | 98 indkomstklinter |
| **I alt** | **490** | **980** | **490 indkomstklinter** |

De første trin for København og Læsø var allerede med blandt de 392 og bliver
derfor ikke talt igen.

Programmet gik målrettet efter de grænser, hvor reglen skifter trin. Resultatet
betyder derfor ikke, at hver ekstra krone i hele indkomstintervallet gør dig
fattigere. Det betyder, at alle 490 undersøgte kombinationer ved de relevante
trin gjorde det.

## Sådan vender Futuruna loven på vrangen

En almindelig skatteberegner gør dette:

```text
én persons oplysninger → reglerne → ét skatteresultat
```

Søgeprogrammet gør dette:

```text
490 fastlagte tilfælde
→ beregn begge sider med de samme regler
→ behold dem, hvor beløbet efter skat falder
→ find det mindste og største tab
```

Der kræves ingen særlig søgesyntaks. Futuruna bruger lister til at bygge
tilfældene, den eksisterende personskattemodel til at beregne dem og et filter
til at beholde de steder, hvor resultatet går baglæns. `?` kontrollerer blandt
andet, at alle 490 er med, at ingen tælles dobbelt, og at histogrammets fire
søjler indeholder de rigtige antal.

Det korte program ændrer altså ikke loven. Det ændrer spørgsmålet, vi stiller
den. Computeren har heldigvis mere tålmodighed til 980 skatteberegninger, end
jeg har.

## Hvad resultatet dækker

Programmet bruger skatteåret 2026 og faste profiler: voksen, ugift, ingen
ægtefælle og ingen kapitalindkomst, aktieindkomst, pension, ejendomsskat,
udenlandske sociale bidrag eller særlige skatteordninger. Alle forhold er faste
bortset fra kommunen, kirkeskatten, transportafstanden og de to lønbeløb omkring
hvert trin.

Kommunesatserne kommer fra [Skatteministeriets officielle tabel for
2026](https://skm.dk/tal-og-metode/satser/oversigt-over-kommuneskatter).
Programmet udleder den forhøjede kørselsfradragssats fra de [25 kommuner, som
reglen
nævner](https://skat.dk/borger/fradrag/koerselsfradrag/yderligere-information-om-koerselsfradrag).
Reglen nævner også ti små øer særskilt. De er ikke med, fordi en kommunesats
ikke fortæller, om en person bor på en bestemt ø.

Transportafstandene er standardiserede beregningseksempler, ikke påstande om
rigtige pendlere i de nævnte kommuner. Særligt er 130 kilometer på Læsø et
standardiseret regneeksempel med lang transport, ikke en påstand om en
sandsynlig bilrute på øen.

Resultaterne kommer fra den nuværende Futuruna-model. De erstatter ikke en
officiel årsopgørelse eller individuel skatte- og juridisk rådgivning.

## Kør søgningen selv

Hele [programmet, der kortlægger
indkomstklinterne](https://github.com/Futuruna/futuruna/blob/main/examples/danish-income-tax/personskat-income-cliffs.audit.runa),
kan læses på GitHub.
Det bruger den fulde
[`personskat.calculate.runa`](https://github.com/Futuruna/futuruna/blob/main/examples/danish-income-tax/personskat.calculate.runa)-model.
I [arbejdsbogen om at undersøge
regler](https://github.com/Futuruna/futuruna/blob/main/examples/danish-income-tax/exploration-workbook.md)
kan du lære metoden og tilpasse den til et andet spørgsmål.

Fra en Futuruna-mappe:

```bash
runa check --frontend examples/danish-income-tax/personskat-income-cliffs.audit.runa
runa examples/danish-income-tax/personskat-income-cliffs.audit.runa
```

Søgningen udfører 980 fulde personskatteberegninger. På maskinen, hvor jeg
målte den, tog det **4 minutter og 23 sekunder**. Tiden vil variere fra maskine
til maskine.

## Hvad spørger vi loven om næste gang?

Det mest opsigtsvækkende for mig er ikke de 170,02 kr. Det er, at selve
søgningen er så lille. Når loven først er udtrykt som et program, behøver vi
ikke skrive en ny skatteberegner for hvert spørgsmål. Vi ændrer spørgsmålet.

En almindelig beregner giver én person ét svar. Her kunne få linjer kode stille
490 spørgsmål til den samme lovmodel og finde samtlige steder i søgningen, hvor
resultatet gik baglæns.

Den næste oplagte undersøgelse er samspillet mellem skatter og ydelser. Én
regel kan se rimelig ud alene. To regler, der rammer den samme krone samtidig,
er sandsynligvis dér, de virkelig mærkelige klinter gemmer sig.

Hvis du har en lov, kontrakt eller grænse, vi bør vende på vrangen, vil jeg
meget gerne høre om den på
[research@futuruna.com](mailto:research@futuruna.com).
