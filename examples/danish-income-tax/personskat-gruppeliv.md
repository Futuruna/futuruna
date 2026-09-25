# Gruppeliv: finansieringen afgør input og arbejdsfradrag

Gruppeliv betalt gennem løntræk i en arbejdsgiveradministreret pension
kræver to forskellige kildebeløb: præmien **før allerede indeholdt AM** til
beskæftigelses- og jobfradrag og den indberettede **nettoværdi** til personlig
indkomst. Modellen må hverken opkræve AM igen eller behandle denne
ikke-fradragsberettigede forsikring som en indbetaling til ekstra pensionsfradrag.

Dette er den almindelige lønfinansierede PBL § 56, stk. 5-gren, ikke alle
forsikringer i en pensionsoversigt. [Bonus fra en fradragsberettiget pension](#gruppeliv-betalt-af-pensionsbonus)
har sin egen indgang uden arbejdsfradrag. Modellen er forskningssoftware, og
`@ calculate` er Preview; resultater er ikke individuel rådgivning.

## Kilder og afgrænsning

- [PBL § 56, stk. 5](https://www.retsinformation.dk/eli/lta/2024/1243)
  adskiller den ikke-fradragsberettigede gruppelivspræmie fra pensionens
  almindelige bortseelsesret.
- [Den juridiske vejledning C.A.10.4.3.10](https://info.skat.dk/data.aspx?oid=2048498)
  beskriver personlig indkomst uden nyt AM, fordi pensionsinstituttet
  allerede har indeholdt bidraget.
- [eIndkomst 8.5, felt 91](https://info.skat.dk/data.aspx?oid=2045911)
  medtager lønfinansieret gruppeliv i beskæftigelsesfradragsgrundlaget.
  Gruppeliv finansieret af **pensionsbonus**, ikke løntræk, henvises derimod
  til felt 38, indtægtsart 37, uden dette arbejdsfradrag.
- [LL §§ 9 J–9 L](https://www.retsinformation.dk/eli/lta/2025/1500)
  og [vejledningen om beskæftigelsesfradrag](https://info.skat.dk/data.aspx?oid=2273718)
  knytter arbejdsfradraget til AM-grundlaget. § 9 L har sit særskilte
  fradrags-/bortseelsesberettigede pensionsgrundlag.
- [Hjælpen til rubrik 347](https://info.skat.dk/data.aspx?oid=2134461)
  beskriver indskud efter allerede indeholdt AM. Rubrikken kan også indeholde
  rateoverskud; den er derfor ikke en entydig gruppelivsklassifikation.
  Hjælpens viste rategrænse bruges ikke som årsparameter.

Kilderne blev kontrolleret 25. september 2026. Modellens skelnen mellem
brutto til arbejdsfradrag og netto til personlig indkomst er også prøvet mod
to offentlige 2025-beregninger nedenfor. Det er ikke en verifikation af alle
forsikringsarter eller administrative afrundinger i andre år.

## Fra bilag til felter

I `lønmodtager.personlig_indkomst.ordinære_forhold.arbejdsgiverydelser`
vælges `GruppelivSomUadskiltDelAfPbl19Ordning` **kun** for den dokumenterede
løntræksgren. I en frisk JSON-skabelon kan ydelsen fx se sådan ud:

```json
{
  "$variant": "GruppelivSomUadskiltDelAfPbl19Ordning",
  "præmie_før_indeholdt_arbejdsmarkedsbidrag_kroner": 1000,
  "personlig_indkomst_efter_indeholdt_arbejdsmarkedsbidrag_kroner": 920
}
```

Tallene er **fiktive, særskilt oplyste kildebeløb**, ikke en tilladelse til
at beregne ukendt brutto ved netto/0,92. Mangler brutto, beholdes `null`;
den kanoniske sammenligningsskat tilbageholdes. Det gælder også den
afgrænsede `ArbejdsgiveradministreretSamlepostEfterPbl19EllerPbl56`.
Brutto under netto og negative beløb kan heller ikke give en gyldig sammenligning.
Samme kontrol og genererede hjælp gælder for en aktiv ægtefælle.

Nettobeløbet kan stadig indgå i komponentens diagnostiske indkomstopgørelse,
selv om brutto mangler. Brug altid den kanoniske `vurdering`, ikke et isoleret
komponentbeløb, som godkendelse af sammenligningsgrundlaget.

Undgå især disse dobbelttællinger:

- En pensionsoversigts samlede indbetaling kan omfatte forsikring. Brug en
  dokumenteret opdeling, før pension og gruppeliv indtastes hver for sig.
- En rubrik 347-total kan omfatte rateoverskud, som modellen allerede beregner
  fra pensionslisten. Indtast ikke det samme overskud igen som en samlepost.
- Direkte arbejdsgiverbetalt gruppeliv skal ikke lægges til igen, hvis det
  allerede er med i det oplyste lønbeløb. Den direkte gren er ikke grenen
  med AM indeholdt af pensionsinstituttet.

Gruppeliv finansieret af pensionsbonus må ikke flyttes til løntræksgrenen
for at få en beregning. Brug den særskilte bonusgren nedenfor, når dens
kildefakta er dokumenteret. Ellers kan en snævrere
[betinget rapportafstemning](aarsopgoerelse-afstemning.md) stadig være nyttig.
Inputtyper autentificerer ikke bilag og opdager ikke enhver forkert
klassifikation eller dublet bag forskellige identifikationer.

## Gruppeliv betalt af pensionsbonus

Her er præmien betalt af **bonus på en fradragsberettiget pensionsordning**,
ikke af årets løntræk. [eIndkomst 8.5](https://info.skat.dk/data.aspx?oid=2045911)
henviser netop dette tilfælde til felt 38, indtægtsart 37, uden
beskæftigelsesfradrag. [PBL § 56, stk. 1 og 3](https://www.retsinformation.dk/eli/lta/2024/1243)
og [C.A.10.4.3.10](https://info.skat.dk/data.aspx?oid=2048498) er grundlaget for
den ordinære præmies personlige indkomst uden AM og henføring til det år,
præmien vedrører. Det er ikke en almindelig PBL § 20-pensionsudbetaling.

Vælg `PersonskatGruppelivFraPensionsbonus` i
`lønmodtager.personlig_indkomst.ordinære_forhold.forenings_og_arbejdsløshedsydelser`.
Samlingen har et historisk navn, men indeholder også andre dækkede poster uden
AM. En **fiktiv pensionsmeddelelse** kunne dokumentere:

- Linje 1: præmien vedrører 2025.
- Linje 2: den er betalt af bonus på en fradragsberettiget pension, ikke løntræk.
- Linje 3: årets præmie er 920 kr.
- Linje 4: ordinære danske forhold uden korrektioner er afklaret.

Det giver denne post, ikke et gæt ud fra årsopgørelsens slutskat:

```json
{
  "$variant": "PersonskatGruppelivFraPensionsbonus",
  "fakta": {
    "identifikation": "fiktiv-bonuspræmie",
    "indkomstår": 2025,
    "kildereference": "Fiktiv pensionsmeddelelse:1–4",
    "finansiering": {
      "$variant": "GruppelivBetaltAfBonusPåFradragsberettigetPension"
    },
    "præmie_kroner": 920,
    "ordinær_dansk_præmie_uden_korrektioner": true
  }
}
```

Modellen medregner 920 kr. i personlig indkomst uden AM. Beløbet giver ikke
beskæftigelses-, job- eller ekstra pensionsfradrag og er ikke en indkomstart
i [LL § 9 C, stk. 4](https://www.retsinformation.dk/eli/lta/2025/1500)'s
aftrapningsgrundlag. Det samme beløb skal ikke også stå under løn,
arbejdsgiverydelser eller pensionsind-/udbetalinger. Brug præmien, ikke hele
pensionsbonussen, pensionssaldoen eller en forsikringssum.

Hvis finansieringen ikke er oplyst, vælges
`GruppelivsfinansieringUoplystEllerUdenForModellen`. Løntræk og bonus fra
**selve gruppelivsforsikringen** har også særskilte valg, men accepteres ikke
af denne beregningsgren. Sådan forsikringsbonus kan have andre ejerafhængige
skatteregler: »bonus« alene er ikke bevis for skattepligt. Manglende kilde,
uklare ordinære forhold, negative beløb, dublerede identifikationer og forkert
indkomstår tilbageholder ligeledes den kanoniske sammenligning. En målrettet
kontrolforklaring vises også for ægtefællen. Dette kontrollerer de oplyste
fakta; det beviser ikke, at udbyderens dokument eller klassifikationen er rigtig.

## Fejl før rettelsen og uafhængige observationer

Den tidligere præcise gruppelivsgren lagde netto til personlig indkomst,
men gav **nul** i arbejdsfradragsgrundlaget. Ved fiktiv løn 100.000 kr. og
nettopræmie 920 kr. blev input accepteret med beskæftigelsesfradrag
12.300 kr. og sammenligningsskat 19.782,23 kr.

To friske anonyme sessioner i
[SKATs 2025-beregner](https://www.tastselv.skat.dk/borger/beregn2025/profil.do)
brugte fødselsdato 1. januar 1990, ugift, København, ingen kirkeskat,
børn, virksomhed, ejendom, anden pension/ATP, udbetalinger eller fradrag.
Kun `RUBRIK11` og `RUBRIK347` havde indkomstbeløb. Fraværet af andre
forhold er udtrykkeligt fiktivt, ikke udledt af manglende bilag.

| Løn, rubrik 11 | Netto, rubrik 347 | Beskæftigelsesfradrag | Jobfradrag | Ekstra pensionsfradrag | Samlet skat |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 100.000 | 920 | 12.423 | 0 | 0 | 19.753,32 |
| 230.000 | 920 | 28.413 | 293 | 0 | 68.796,78 |

Den rettede kanoniske model matcher begge profiler frem til samlet skat i
øre med særskilt oplyst bruttopræmie 1.000 kr. Nettopræmien forbliver 920 kr.,
og der opkræves ikke nyt AM af forsikringen.

Alle tabelbeløb er DKK. Samlet skat er specifikationens skat inklusive
løn-AM, før betalingsafregning/restskattetillæg, ikke inklusive
pensionsinstituttets allerede indeholdte AM. Fradrag, som ikke vises i
specifikationen, er her nul. Den lave løn undersøger beskæftigelsesfradraget
under loftet; den anden profil undersøger også jobfradraget.

HTTP-sessionerne fulgte formularens faktiske felter og handlinger, som i
[arbejdsgiverpensionskontrollen](skatdk-arbejdsgiverpension-ekstern.md#kilder-metode-og-afgrænsning).
Tilbagefunktionen bevarede de indsendte tal. Ingen login, CPR eller private
rapporter indgik. Dette er ikke en grafisk browserafprøvning eller en
observation af en faktisk årsopgørelse; beregnerens versionsnummer blev ikke
registreret. Rå sessioner opbevares uden for repoet.

To yderligere anonyme 2025-sessioner den 25. september 2026 brugte samme
profil, men `RUBRIK17 = 920` i stedet for rubrik 347:

| Løn, rubrik 11 | Beløb, rubrik 17 | Beskæftigelsesfradrag | Jobfradrag | Ekstra pensionsfradrag | Samlet skat |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 100.000 | 920 | 12.300 | 0 | 0 | 19.782,23 |
| 230.000 | 920 | 28.290 | 248 | 0 | 68.836,26 |

Bonusgrenen matcher begge profiler frem til samlet skat i øre og bevarer
præmien som 920 kr. i personlig indkomst uden AM eller arbejdsfradrag.

Disse observationer afprøver den ikke-AM-pligtige indkomst **uden**
arbejdsfradrag, ikke automatisk identifikation af pensionsbonus ud fra rubrik
17. [Rubrikkens hjælp](https://info.skat.dk/data.aspx?oid=170513) omfatter også
andre poster, og det er den særskilte udbyderkilde, der afgør den juridiske
gren. De indsendte beløb blev kontrolleret via formularens tilbagefunktion;
samme metodebegrænsninger som ovenfor gælder.

## Migration og regression

Gruppelivsvarianten har fået et nyt valgfrit bruttofelt, og den genererede
kontrakthash ændres. Generér en frisk skabelon og overfør gennemgåede fakta;
overskriv ikke blot den gamle hash. I `.runa`-kald skal det nye argument
angives som `Some(dokumenteret_brutto)` eller `None`. En tidligere
samlepost med ukendt brutto giver nu heller ikke en kanonisk sammenligningsskat.
Den nye bonusvariant ændrer kontrakthashen igen uden at omklassificere gamle
poster automatisk. Generér også en frisk kontrakt ved denne udvidelse.

[Regressionen](../../tests/personskat_group_life.test.mjs) bruger faste
officielle observationer, ikke beløb udledt af modellen. Den kontrollerer
de to profiler, den afgrænsede samlepost, manglende/modstridende brutto,
ægtefælleberegning og projekteret metadata. Den kører offline med én worker:

```sh
FUTURUNA_MODEL_TEST_RUNA="$RUNA_BIN" node --test tests/personskat_group_life.test.mjs
```

[Bonusregressionen](../../tests/personskat_group_bonus.test.mjs) følger den
fiktive meddelelse ovenfor gennem den kanoniske model. Den sammenholder to
profiler med de registrerede rubrik 17-observationer og kontrollerer en aktiv
ægtefælle, ukendt/forkert finansiering, dubletter og årsmismatch samt de seks
genererede kilde-/interviewfelter for begge personer. Den genberegner ikke skat
i JavaScript. [Komponentgrænserne](../../tests/personskat_group_bonus_test.runa)
dækker også nul, negativt beløb, blank kilde og understøttede år.

```sh
"$RUNA_BIN" check tests/personskat_group_bonus_test.runa
"$RUNA_BIN" tests/personskat_group_bonus_test.runa
"$RUNA_BIN" run tests/personskat_group_bonus_test.runa
FUTURUNA_MODEL_TEST_RUNA="$RUNA_BIN" node --test tests/personskat_group_bonus.test.mjs
```

Brug den compiler, der består [runtime-tjekket](../../website/public/ai-setup.md#tax-audit-runtime-check).
De øvrige [konformitetsforbehold](../../docs/tax-audit-readiness.md#limits-to-keep-visible)
består, herunder den særskilte uafklarede arbejdsgiverrate-observation.
