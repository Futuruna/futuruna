# Renter: input og ekstern kontrol

Seks **fiktive** 2025-beregninger hos Skattestyrelsen blev kontrolleret den
25. september 2026. Futurunas kanoniske beregning stemte i alle seks tilfælde
til øren, også omkring 50.000 kr. i negativ nettokapitalindkomst og ved lav
indkomst. Der var **ikke behov for en formelrettelse**. Kontrollen er nu en
fast regressionstest.

Den særskilte ægtefællekontrol nedenfor fandt derimod en manglende
§ 6, stk. 3-modregning i den kanoniske beregning. Den er nu forbundet
til den allerede kodede lovregel; de oprindelige enligeberegninger ændres ikke.

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

### Vejledning i det genererede input

De to rentefelter har nu fælles typet metadata for hovedperson og ægtefælle.
`runa schema` viser spørgsmål, enhed, kilde og hjælp om egen andel, fortegn,
dobbeltregistrering og adskillelse af indtægt og udgift. Rubrik 44 kan også
rumme provisioner; dens total er derfor ikke automatisk et rentebeløb til
dette felt. Afklar sådanne poster via `kapitalindkomst.finansielle_poster`.
LL §§ 6/6 A-fradrag har ligeledes egne input og må ikke tælles med igen.

Et minustegn i bilaget skal afklares som fradragsvisning eller rettelse,
ikke automatisk fjernes. Den [afklarede renteudgiftsmapping](fra-bilag-til-input.md#renteudgifter-bevar-kildens-fortegn)
bevarer originalt fortegn og øre og kan foreslå en positiv udgiftsstørrelse
uden afrunding. Den generelle helper kræver klassifikation først.

Dette er inputvejledning, ikke en ny kontrol af bilagene. Et forkert positivt
årsbeløb kan stadig passere; et ukendt beløb skal afklares før den kanoniske
beregning. Nul er kun til bekræftet fravær. De eksisterende kontroller afviser
negative indtægts- og udgiftsbeløb, også for en aktiv ægtefælle.

Metadataændringen ændrer Preview-kontraktens fingerprint, ikke dens datatyper
eller skatteformler. Generér en ny skabelon og overfør gennemgåede kildefakta;
ret ikke et gammelt hash. Den [fokuserede test](../../tests/tax_interest_input.test.mjs)
kontrollerer alle fire genererede felter samt nul, fortegn og den eksisterende
netting af separate indtægter og udgifter med fiktive fakta.

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

## Ægtepar: særskilte observationer for begge personer

Den 25. september 2026 blev fire yderligere **fiktive ægtepar** indtastet
i samme anonyme 2025-beregner. Begge personer har den ovenstående profil,
men er gift og samlevende ved årets udløb; løn og renter varierer som vist.
Der er ingen tidligere underskud, særlige lempelser eller forskudsbetalinger.
Beløbene er personernes allerede afklarede andele, ikke en anbefaling om
at omfordele renter. Andre valgfrie beløbsfelter var blanke.

Løn, renteudgift og renteindtægt blev indtastet i rubrik 11/42/31 for A
og formularfelterne 211/242/231 for B. Begge specifikationer blev hentet
separat (`hop`: skatteyder A, `bip`: ægtefælle B). Tabellen gengiver
**observeret beregnet skat inklusive AM**, ikke restskat inklusive tillæg.
Alle beløb i denne tabel er **DKK**; testens forventninger bruger hele øre.

| Tilfælde | Løn A / B | Renteudgift A / B | Renteindtægt A / B | Skat A | Skat B |
| --- | ---: | ---: | ---: | ---: | ---: |
| Fælles grænse + 1 kr. | 600.000 / 200.000 | 100.001 / 0 | 0 / 0 | 180.444,30 | 57.234,24 |
| Begge har negativ nettokapitalindkomst | 600.000 / 200.000 | 60.000 / 40.000 | 0 / 0 | 193.044,54 | 44.634,24 |
| Positiv kapitalindkomst hos B | 600.000 / 200.000 | 100.001 / 0 | 0 / 20.000 | 182.044,22 | 61.934,24 |
| B kan ikke selv udnytte nedslaget | 600.000 / 0 | 0 / 40.000 | 0 / 0 | 181.021,38 | 0,00 |

Observationerne undersøger forskellige dele af
[PSL § 6 og §§ 10–13](https://www.retsinformation.dk/eli/lta/2021/1284):

- I første række bruger A 8.000 kr. i § 11-nedslag. I anden række er
  nedslaget fordelt med 4.800 kr. hos A og 3.200 kr. hos B.
- I tredje række reducerer B's positive kapitalindkomst grundlaget for A's
  § 11-nedslag; det udnyttede nedslag er 6.400,08 kr. B's bundskat før
  personfradrag er fortsat 22.098,40 kr.; specifikationen viser modregning
  af B's positive kapitalindkomst med A's negative kapitalindkomst.
- I sidste række overføres B's underskud på 40.000 kr. til A. A's
  skattepligtige indkomst er derfor 453.500 kr. Specifikationen viser også
  overført personfradragsværdi på 6.197,16 kr. i bundskat og 12.126 kr.
  i kommuneskat samt 3.200 kr. i uudnyttet § 11-nedslag. B's skat er nul.

Den [fokuserede ægtepartest](../../tests/personskat_married_interest.test.mjs)
bruger otte kanoniske beregninger, én for hver person i hvert par, så begge
personers eksakte sammenligningsbeløb og gyldighed kontrolleres. Den bevarer
også de observerede bundskatter, kommuneskatter og udnyttede nedslag som
uafhængige forventninger. Ingen forventet skat eller overførsel indgår som
input. Modellen skal selv beregne overførslerne.

Kontrollen fandt en konkret fejl i tredje række: B's bundskat før
personfradrag var 24.500,40 kr. i modellen mod 22.098,40 kr. i
specifikationen. Den kanoniske sammensætning brugte ikke den eksisterende
§ 6, stk. 3-regel om ægtefællens negative kapitalindkomst. Samlet skat
blev derfor 64.336,24 kr. i stedet for 61.934,24 kr. Rettelsen forbinder
lovreglen i både øreberegningen og den ældre helkroneopdeling; den ændrer
ikke rentefakta eller kommuneskattegrundlaget for at opnå et match.
Efter rettelsen stemmer alle otte personers beregnede skat, bundskat,
kommuneskat og udnyttede § 11-nedslag med de registrerede specifikationer
til øren. De forventede beløb er ikke ændret.

Denne faste test bytter de to personers fuldstændigt beskrevne fakta;
parrets øvrige input er neutrale. Den er **ikke** en generel adapter til at
bytte ægtefæller i personlige sager med personbundne tab, lempelser eller
betalingsoplysninger. Ukendte ægtefællefakta bliver ikke kendte gennem
testen, og observationerne dækker ikke forskellige kommuner, separation,
delår, udenlandske forhold eller grøn check.
Delårsmodellens særskilte kapitalgrundlag og ægtefællekontekst undersøges
fortsat (`td-fd0cba`); denne årsberegning dokumenterer ikke deres korrekthed.

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
FUTURUNA_MODEL_TEST_RUNA="$RUNA_BIN" node --test tests/personskat_married_interest.test.mjs
```

Testene kører offline med én beregningsarbejder. Observationerne validerer
ikke alle lånetyper, ægtefælleoverførsler, delår, andre år eller en hel
personlig årsopgørelse. Forskningssoftware, ikke individuel skatterådgivning.
