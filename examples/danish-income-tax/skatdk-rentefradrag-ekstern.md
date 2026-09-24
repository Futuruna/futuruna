# Renter: input og ekstern kontrol

Seks **fiktive** 2025-beregninger hos Skattestyrelsen blev kontrolleret den
25. september 2026. Futurunas kanoniske beregning stemte i alle seks tilfælde
til øren, også omkring 50.000 kr. i negativ nettokapitalindkomst og ved lav
indkomst. Der var **ikke behov for en formelrettelse**. Kontrollen er nu en
fast regressionstest.

## Hvilke beløb skal ind i modellen?

Begynd med årsoversigten og din egen andel. Banker og realkreditinstitutter
indberetter normalt renterne automatisk: rubrik 42 for bank mv. og rubrik 41
for realkredit. Kontroller, at samme udgift ikke tælles to gange. Private
långivere, restancer og udenlandske forhold kræver særskilt afklaring;
brug ikke bare lånets samlede ydelse som rentebeløb.
[SKATs rentevejledning](https://skat.dk/borger/fradrag/fradrag-for-renter).

I [Personskat-inputtet](personskat.calculate.runa) ligger beløbene under
`kapitalindkomst.renter`:

- `renteudgifter_kroner`: din samlede **fradragsberettigede** renteudgift
  for året, angivet positivt. Ikke afdrag, hele husstandens udgift eller et
  beregnet skattenedslag.
- `renteindtægter_kroner`: årets renteindtægter, separat fra udgifterne.
  Træk dem ikke først fra udgiftsfeltet og indtast dem derefter igen.
- Næringsstatus og særregler skal stadig afklares. Et accepteret samlet
  beløb er ikke dokumentation for, at den enkelte låneudgift er fradragsberettiget.

Feltet er en sum af allerede afklarede kildefakta; det undersøger ikke selv
långiver, hæftelse, betalingshistorik eller fordeling mellem låntagere.
En ukendt ægtefælleandel skal ikke erstattes med et gæt. Brug
[betinget rapportafstemning](aarsopgoerelse-afstemning.md), hvis kun den
ene årsopgørelse foreligger, og hold den adskilt fra en selvstændig
genberegning af husstanden. Modellen ændrer intet i TastSelv.

## Fiktive fakta og aflæste resultater

Kilde: [SKATs anonyme årsberegner for 2025](https://www.tastselv.skat.dk/borger/beregn2025/profil.do).
Versionsnummer blev ikke registreret. Løn blev indtastet i rubrik 11,
renteudgift i rubrik 42 og renteindtægt i rubrik 31. Resultaterne er fra
beregnerens specifikation, ikke beregnet baglæns fra Futuruna.

Fælles profil: enlig, født 1. januar 1990, København (101), ingen kirkeskat,
fuldt skattepligtig og DBO-hjemmehørende i Danmark hele året. Ingen
udlandsudelukkelse, virksomhed, ejerbolig, børn, pension, ATP, andre
indkomster eller fradrag. Der blev ikke indtastet forskudsbetalinger.
Øvrige valgfrie beløbsfelter var blanke i denne fiktive profil; det er
ikke en regel om at sætte ukendte fakta til nul i en rigtig sag.

Friske anonyme sessioner blev brugt uden login, CPR eller private dokumenter.
Repoet indeholder kun de fiktive fakta og forventede beløb, ikke cookies,
sessionsnøgler eller rå HTML.

Løn, renter og skattepligtig indkomst er i **kroner**. Skattekolonnerne er
i **øre**. Kommuneskat er før personfradrag; § 11-kolonnen er det faktisk
udnyttede nedslag efter personfradrag. Samlet skat er beregnet skat inklusive
AM, før restskattetillæg og modregning af forskudsbetalinger, ikke beløbet
til endelig betaling.

| Løn | Renteudgift | Renteindtægt | Skattepligtig indkomst | Kommuneskat | Udnyttet § 11-nedslag | Samlet skat |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 600000 | 49999 | 0 | 443501 | 10422273 | 399992 | 19619485 |
| 600000 | 50000 | 0 | 443500 | 10422250 | 400000 | 19619454 |
| 600000 | 50001 | 0 | 443499 | 10422226 | 400000 | 19619430 |
| 600000 | 51001 | 1000 | 443499 | 10422226 | 400000 | 19619430 |
| 100000 | 30000 | 0 | 49700 | 1167950 | 240000 | 1000554 |
| 100000 | 40000 | 0 | 39700 | 932950 | 205554 | 800000 |

De tre første rækker kontrollerer grænsen ±1 kr. Den fjerde kontrollerer
netting af renteindtægter og -udgifter. De to sidste kontrollerer samspillet
med personfradrag og utilstrækkelig skat at modregne nedslaget i.

## Hvad betyder grænsen?

[Personskattelovens § 11](https://www.retsinformation.dk/eli/lta/2021/1284)
er bevaret med kildecitat i [personfradragsmodellen](kapitel-03-personfradrag.runa).
Den modellerede regel giver i 2025 et ekstra nedslag på 8 % af de første
50.000 kr. negativ nettokapitalindkomst for denne enlige profil. **Det er
ikke et loft på fradragsberettigede renteudgifter.** Den tredje række har
fortsat lavere kommuneskat end den anden. Ægtefællereglerne er ikke prøvet
af disse seks observationer.

Ved 100.000 kr. i løn og 40.000 kr. i renter er det beregnede § 11-nedslag
3.200 kr., men kun 2.055,54 kr. kan bruges efter personfradrag. AM-bidraget
på 8.000 kr. består. Det er derfor forkert at love en fast skattebesparelse
for enhver ekstra krone i renteudgift. Forskellen kan ikke i sig selv
begrunde at låne mere.

## Reproduktion og afgrænsning

Den [kanoniske integrationstest](../../tests/personskat_pension_timing.rs)
kontrollerer alle seks rækker gennem `runa template` og `runa call`, med
uafhængigt valgte kildefakta som input og faste observerede skattebeløb
som forventning. Den kontrollerer også gyldighed og bevarer modellens
udtrykkelige begrænsning af samlet dækning.

Brug den verificerede compiler fra
[tax-audit-opsætningen](../../website/public/ai-setup.md#tax-audit-runtime-check):

```sh
FUTURUNA_MODEL_TEST_RUNA="$RUNA_BIN" cargo test --quiet --test personskat_pension_timing ordinary_interest_offsets_match_official_2025_calculator -- --exact --nocapture
```

Testen kører offline med én beregningsarbejder. Observationerne validerer
ikke alle lånetyper, ægtefælleoverførsler, delår, andre år eller en hel
personlig årsopgørelse. Forskningssoftware, ikke individuel skatterådgivning.
