# Ekstra pensionsfradrag: ekstern kontrol af afrunding

Den 25. september 2026 viste otte **fiktive** beregninger hos Skattestyrelsen,
at det ekstra pensionsfradrag oprundes til hele kroner i de undersøgte
2025- og 2026-tilfælde. Futuruna afrundede tidligere til nærmeste krone.
Et privat ratebidrag på 40.001 kr. gav derfor 4.800 kr. i modellen, men
4.801 kr. hos SKAT. Den forskel blev reproduceret gennem `runa call` før
rettelsen, ikke kun i en isoleret procentformel.

Modellen bruger nu oprunding af det **samlede grundlag efter modregning og
loft**. Den afrunder ikke hver pensionspost for sig. Satser, pensionsalder,
fradragsret og lofter ændres ikke.

## Kilder og fiktive fakta

- [Ligningslovens § 9 L](https://www.retsinformation.dk/eli/lta/2025/1500)
  fastlægger grundlag, modregning, satser og henvisningen til årets regulering.
  Lovcitatet i [modellen](ligningsloven_fradrag.runa) er bevaret.
- [SKATs vejledning om ekstra pensionsfradrag](https://skat.dk/borger/fradrag/ekstra-pensionsfradrag)
  beskriver 12/32-procentsatserne og grundlaget. Den er ikke her anvendt som
  dokumentation for en lovbestemt helkroneafrunding.
- [Den anonyme årsberegner for 2025](https://www.tastselv.skat.dk/borger/beregn2025/profil.do):
  indtast løn i rubrik 11 og privat ratebidrag i rubrik 21. Åbn resultatets
  specifikation for fradrag og skattekomponenter. Versionsnummer blev ikke
  registreret.
- [Den anonyme forskudsberegner for 2026](https://www.tastselv.skat.dk/fskbrgn2/Skprofil.aspx?indkomstaar=2026):
  samme fakta i felt 201 og 416. Observeret profilversion **26.3.5.1**.
  Dette er en forskudsberegning, ikke en færdig årsopgørelse for 2026.

Fælles profil: enlig, født 1. januar i tabellens år, København (101), ingen
kirkeskat, selvstændig virksomhed, ejerbolig, børn eller enligforsørgertillæg.
Årsløn 600.000 kr. før AM. Fuldt skattepligtig og DBO-hjemmehørende i Danmark
hele året, ingen udlandsudelukkelse. Privat rateopsparing betalt i samme år,
ingen arbejdsgiverpension, ATP, pensionsudbetalinger i året eller året før,
andre indkomster eller fradrag. Ingen alderspensionsudbetaling blev valgt.

De øvrige valgfrie beløbsfelter blev efterladt blanke i denne afgrænsede,
fiktive profil. Det er **ikke** en anvisning på at behandle ukendte oplysninger
i en rigtig sag som nul. Alle personfakta er valgt uafhængigt af resultatet;
intet fradrag er indtastet baglæns for at få skatten til at stemme.
Friske anonyme sessioner blev brugt uden login, CPR-numre eller private
dokumenter. Cookies, sessionsnøgler og rå HTML indgår ikke i repoets testdata.

## Aflæste resultater

Indbetaling, fradrag og indkomst er i **kroner**; skattekolonnerne i **øre**.
Kommuneskat er før personfradrag. Samlet skat inkluderer AM, men ikke renter,
restskattetillæg eller modregning af forskudsbetalinger.

| År | Fødselsår | Privat ratebidrag | Ekstra pensionsfradrag | Skattepligtig indkomst | Kommuneskat | Samlet skat |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 2025 | 1990 | 1 | 1 | 493498 | 11597203 | 21194394 |
| 2025 | 1990 | 40000 | 4800 | 448700 | 10544450 | 19661254 |
| 2025 | 1990 | 40001 | 4801 | 448698 | 10544403 | 19661194 |
| 2025 | 1990 | 40004 | 4801 | 448695 | 10544332 | 19661087 |
| 2025 | 1990 | 40005 | 4801 | 448694 | 10544309 | 19661052 |
| 2025 | 1960 | 40001 | 12801 | 440698 | 10356403 | 19473194 |
| 2026 | 1990 | 40001 | 4801 | 440798 | 10310265 | 19344232 |
| 2026 | 1960 | 40001 | 12801 | 426698 | 9980466 | 19014433 |

Personlig indkomst er i alle otte tilfælde 552.000 kr. efter AM minus det
private ratebidrag. Beskæftigelsesfradrag/jobfradrag er 55.600/2.900 kr. i
2025 og 63.300/3.100 kr. i 2026. Personen født i 1960 har desuden 6.100 kr.
i seniorbeskæftigelsesfradrag i 2026. Det er et andet fradrag end § 9 L.

40.001 × 12 % er 4.800,12, og 40.001 × 32 % er 12.800,32. Begge
observationer adskiller oprunding fra nærmeste krone. Kontrollen med 40.000
viser, at et helt produkt ikke forhøjes. Indkomst- og skattekolonnerne viser,
at forskellen indgår i beregningen og ikke blot er visningsafrunding.

## Dækning og reproduktion

[§ 9 L-scenariet](ligningsloven-par9l.scenario.runa) indeholder alle otte
aflæste fradrag. Det kontrollerer også seks **afledte modelkanter**: nul,
samlet afrunding af flere bidrag, modregning før afrunding, intet negativt
fradrag og årets loft på og lige over grænsen. Disse seks er ikke yderligere
observationer hos SKAT. Eksisterende scenarier for pensionsalder, ugyldig dato,
historiske satser, udbetalingsundtagelse og kombination med seniorfradrag
bevares.

Den [kanoniske integrationstest](../../tests/personskat_pension_timing.rs)
kontrollerer fem af tilfældene frem til samlet skat i øre: 2025-bidraget på
1 kr. samt 40.001 kr. ved begge aldersprofiler i begge år. De skal både
have gyldige kontroller og bevare den udtrykkelige begrænsning af samlet
modeldækning. Tests bruger faste forventninger fra observationerne og kører
offline. Native/interpreter-paritet gælder § 9 L-scenariet, ikke hele
Personskats native kodegenerering.

Brug samme afprøvede compiler som i
[tax-audit-opsætningen](../../website/public/ai-setup.md#tax-audit-runtime-check):

```sh
"$RUNA_BIN" examples/danish-income-tax/ligningsloven-par9l.scenario.runa
"$RUNA_BIN" run examples/danish-income-tax/ligningsloven-par9l.scenario.runa
```

Dette er konformitet med **observeret offentlig beregnerpraksis**, ikke et
fund af en generel lovbestemmelse om afrunding. § 9 L-satserne giver eksakte
øre ved modellens hele kronegrundlag; oprunding via øre og direkte oprunding
giver derfor samme resultat her. Det gælder ikke vilkårlige andre satser.
Anvendelsen i ældre år er en udtrykkelig modelantagelse, ikke uafhængigt
verificeret historisk praksis eller en påstået lovændring i 2025.

Private ratebidrag validerer ikke alle pensionsordninger, delår eller en
hel personlig årsopgørelse. De to kendte 2026-afvigelser for **arbejdsfradrag**
i [den særskilte kontrol](skatdk-fradrag-oere-ekstern.md) er ikke løst her.

Inputtyper ændres ikke. Genberegn berørte resultater fra de samme kildefakta;
ret ikke indbetalinger eller andre fakta for at udligne en forskel. Et
fradrag kan stige med én krone, hvilket ikke er det samme som én krone mindre
skat. Modellen er forskningssoftware, ikke individuel skatterådgivning.
