# Ekstern kontrol af arbejdsfradragenes hele kroner

Den 24. september 2026 blev 11 **fiktive** lønmodtagerberegninger kontrolleret
mod Skattestyrelsens offentlige, anonyme beregnere. De viser højere hele
fradragsbeløb end Futurunas tidligere afkortning til kroner. En
[efterfølgende kontrol af øretrin og tillæg](skatdk-fradrag-oere-ekstern.md)
præciserer forklaringen: modellen afkorter nu til hele øre før oprunding til
kroner. De 11 observationer nedenfor stemmer fortsat; direkte matematisk
oprunding fra procentproduktet er ikke længere modellens konvention.

Det [eksekverbare scenarie](skatdk-arbejdsfradrag-ekstern.scenario.runa) gemmer
aflæste forventninger og kontrollerer de almindelige fradrag. Den kanoniske
[integrationstest](../../tests/personskat_pension_timing.rs) kontrollerer syv
af tilfældene helt frem til skat i øre gennem `runa call`. Begge tests kører
offline; de henter ikke nye tal fra nettet.

## Kilder og reproduktion

- [Beregn din skat for 2025](https://skat.dk/borger/aarsopgoerelse/beregn-din-skat)
  henviser til [den anonyme årsberegner](https://www.tastselv.skat.dk/borger/beregn2025/profil.do).
  Brug resultatets »Specifikation af beregnet skat« for de enkelte fradrag og
  skattekomponenter. Ingen versionsbetegnelse blev registreret for denne beregner.
- [Beregn skat og skattekort 2026](https://www.tastselv.skat.dk/fskbrgn2/Skprofil.aspx?indkomstaar=2026),
  observeret profilversion **26.3.5.1**, viser komponenterne i resultattabellen.
- [SKATs fradragsvejledning](https://skat.dk/borger/fradrag/arbejdsrelaterede-fradrag/beskaeftigelses-og-jobfradrag)
  fastlægger de anvendte satser, indkomstgrænser og lofter. Sidens afrundede
  indkomsttal for fuldt fradrag er ikke brugt til at udlede helkroneprojektionen.

Fælles profil: enlig, født 01.01.1990, København (101), ingen kirkeskat,
ingen selvstændig virksomhed, ejerbolig, børn eller enligforsørgertillæg.
Fiktiv fuldt skattepligtig lønmodtager hele året, ingen udlandsudelukkelse,
ATP, pensionsindbetalinger/-udbetalinger, andre indkomster eller fradrag.
Kun løn udfyldes: rubrik 11 i 2025 og felt 201 i 2026. De øvrige valgfrie
beløbsfelter efterlades blanke i denne bevidst afgrænsede profil. Det er ikke
en anvisning på at behandle manglende oplysninger i en rigtig sag som nul.
Indtastning af nul i visse valgfrie felter kan aktivere særlige beregningsgrene.

Der blev anvendt friske anonyme sessioner og almindelige formularindsendelser,
ikke login, CPR-numre eller personlige dokumenter. Formularernes sessionsnøgler,
cookies og rå HTML er ikke en del af repoets testdata.

## Aflæste resultater

Alle indkomster og fradrag er i kroner; skattekolonnerne er i øre.
Kommuneskat er før skatteværdien af personfradraget. Samlet skat inkluderer AM,
men ikke renter, tillæg eller modregning af forskudsbetalinger.

| År | Løn | Beskæftigelsesfradrag | Jobfradrag | Personlig indkomst | Skattepligtig indkomst | Kommuneskat | Samlet skat |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 2025 | 100001 | 12301 | 0 | 92001 | 79700 | 1872950 | 1945566 |
| 2025 | 224501 | 27614 | 1 | 206541 | 178926 | 4204761 | 6649002 |
| 2025 | 288922 | 35538 | 2899 | 265809 | 227372 | 5343242 | 9014592 |
| 2025 | 288923 | 35538 | 2900 | 265810 | 227372 | 5343242 | 9014604 |
| 2026 | 100000 | 12750 | 0 | 92000 | 79250 | 1853657 | 1843437 |
| 2026 | 100001 | 12751 | 0 | 92001 | 79250 | 1853657 | 1843449 |
| 2026 | 235201 | 29989 | 1 | 216385 | 186395 | 4359779 | 6925022 |

Fire yderligere 2025-aflæsninger af ligningsmæssige fradrag i alt:
100000 → 12300; 100004 → 12301; 100005 → 12301; 100500 → 12362.
Her er jobfradrag og alle andre ligningsmæssige fradrag nul ifølge den fiktive
profils forudsætninger. For 100004 viser resultatet også personlig indkomst
92004 og skattepligtig indkomst 79703. Det eksakte produkt 100004 × 12,30 %
er 12300,492; både nedrunding og afrunding til nærmeste krone ville give 12300.

## Hvad kontrollen fastslår — og ikke fastslår

Oprunding efter afkortning til hele øre stemmer med disse observationer i **2025 og 2026**,
inklusive et helt produkt, små positive brøkdele, jobfradragets start og
2025-jobfradragets loft. Skattekomponenterne viser, at de hele fradragsbeløb
faktisk indgår i beregningsgrundlaget; forskellen er ikke blot visning.

Det er en kildebelagt modelkorrektion mod offentlig beregneradfærd, ikke et
fund af en generel lovbestemmelse om alle fradrags afrunding. Modellen anvender
samme oprunding for almindeligt § 9 J/§ 9 K i sine ældre år som en udtrykkelig
**modelantagelse**. Historisk administrativ konformitet er ikke verificeret.
Der indføres ikke en opdigtet lovændring i 2025. Satser, lofter, betingelser og
fordeling af udenlandsk arbejdsindkomst ændres ikke.

Seniorfradrag og enligforsørgertillæg er undersøgt særskilt i
[den efterfølgende kontrol](skatdk-fradrag-oere-ekstern.md), som også
dokumenterer **tre åbne 2026-afvigelser på én krone**. Ekstra pensionsfradrags
afrunding er efterfølgende undersøgt i [otte særskilte pensionscases](skatdk-pensionsfradrag-ekstern.md).
Kontrollen er heller ikke fuld validering af en
personlig årsopgørelse eller af delårsberegning.
[DJV's delårseksempel C.F.1.6.2.1](https://info.skat.dk/data.aspx?oid=1977388)
har afrundede grundlag og omregning; det kan ikke læses som en isoleret
helkroneafrundingsregel. En resterende forskel må undersøges, ikke udlignes
ved at ændre brugerens fakta.

Genberegn tidligere resultater efter modelopdateringen. API og inputtyper
ændres ikke; ændret fradragsafrunding kan flytte et fradrag med én krone og
dermed ændre den beregnede skat. Brug den afprøvede compiler fra
[tax-audit-opsætningen](../../website/public/ai-setup.md#tax-audit-runtime-check):

```sh
"$RUNA_BIN" examples/danish-income-tax/skatdk-arbejdsfradrag-ekstern.scenario.runa
```

Modellen er forskningssoftware, ikke individuel skatterådgivning.

Native/interpreter-paritet kontrolleres for fradragsscenariet. Hele Personskats
native kodegenerering har fortsat kendte begrænsninger; ovenstående kontrol af
samlet skat gælder beregningsinterfacet `runa call`, ikke fuld native kørsel.
