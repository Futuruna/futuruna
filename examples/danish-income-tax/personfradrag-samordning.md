# Personfradrag mellem ægtefæller

Den kanoniske beregning overfører **fradragsgrundlag**, ikke afsenderens
kommunale skatteværdi. Grundlaget bliver beregnet fra kildefakta; ingen
personfradragspost fra årsopgørelsen sættes ind som beregningsinput.

Egne personfradrag bruges først, også mod øvrige relevante indkomstskatter.
Resten omregnes til fradragsbeløb og værdisættes med modtagerens satser.
Årsafsluttende skattemæssigt samliv er fortsat en betingelse. Det følger af
[PSL §§9, 10, stk. 3, og 12](https://www.retsinformation.dk/eli/lta/2021/1284)
og [CIR 129/1994, pkt. 11.1.1–11.1.2](https://www.retsinformation.dk/eli/mt/1994/129).

Derfor kan en modtager få kirkelig skatteværdi, selv om afsenderen ikke er
medlem af folkekirken. Kommune, kirkemedlemskab, indkomster og fradrag skal
fortsat være faktiske oplysninger. Ukendte ægtefællefakta må ikke erstattes
med nul: brug da [betinget afstemning](aarsopgoerelse-afstemning.md).

## Uafhængige observationer

Aflæst 25. september 2026 i SKATs anonyme
[Beregn skatten 2025](https://www.tastselv.skat.dk/borger/beregn2025/profil.do).
Alle personer er fiktive, født 1. januar 1990, gift og samlevende hele året.
Modtageren bor i København og har 400.000 kr. i løn. Afsenderen bor i Ballerup.
Der er ingen øvrige indkomster, pensioner, ATP, ejendomme eller fradragsudgifter;
de almindelige arbejdsfradrag beregnes af lønnen. Ingen private dokumenter,
CPR-numre eller MitID-sessioner er anvendt.

| Afsenders løn, kr. | Kirke: modtager / afsender | Overført bundskatteværdi, kr. | Overført lokal værdi, kr. | Modtagers beregnede skat inkl. AM, kr. |
|---:|:---|---:|---:|---:|
| 0 | nej / nej | 6.197,16 | 12.126,00 | 113.786,98 |
| 0 | ja / nej | 6.197,16 | 12.538,80 | 115.488,58 |
| 40.000 | nej / nej | 1.777,48 | 4.634,20 | 125.698,46 |
| 60.000 | nej / nej | 0,00 | 489,74 | 131.620,40 |
| 60.001 | nej / nej | 0,00 | 489,74 | 131.620,40 |
| 60.000 | ja / ja | 0,00 | 518,07 | 133.706,47 |

»Lokal« er beregnerens samlede kommune-/kirkepost. Beløbene er før
restskattetillæg og betalinger, ikke det beløb, der skal betales til SKAT.
Regressionen i `tests/personskat_validity.rs` bruger kun de fiktive kildefakta
og kontrollerer både gyldighed og skatten til sammenligning.

Ved 60.000 kr. uden kirkeskat er afsenderens resterende kommunale skatteværdi
963,90 kr. Før overførsel bruges 432,36 kr. mod egen bundskat. De resterende
531,54 kr. svarer til 2.084,4705… kr. i fradrag ved 25,5 %. Beregnerens
overførsel svarer til **2.084 hele kroner × 23,5 % = 489,74 kr.**
Helkrone-nedrundingen er her en eksplicit fortolkning af den observerede
beregningspraksis, ikke en særskilt lovbestemmelse fundet i kilden.

## Beregningsspor og kompatibilitet

[Samordningsmodulet](personfradrag-samordning.runa) er fælles for egne og
modtagne fradrag. Det bruger de eksisterende §9-deltrin og udvider ikke
personfradraget til AM, CFC eller lav aktieindkomstskat.
Den samlede lokale rest fordeles forholdsmæssigt på kildekomponenterne;
divisionsrester bevares. Ved modtagelsen bevares den samlede lokale værdi,
og kommunen bærer en eventuel øre-rest efter opdeling. Disse komponentfordelinger
er modellens præsentation, ikke en eksternt valideret SKAT-fordeling.

`LønmodtagerEksakteUdgåendeOverførsler.personfradragsgrundlag` viser de afledte
hele kronebeløb for bundskat, historisk sundhedsbidrag og lokale skatter.
De eksisterende `ubrugt_*_personfradrag_øre` viser stadig afsenderens restværdi,
nu efter egen samordning. De må ikke læses som modtagerens kredit.
Den kanoniske helkroneberegning bruger samme eksakt afledte grundlag som
øreberegningen; dens øvrige helkroneafrundinger er fortsat en
kompatibilitetsberegning, ikke tallet til afstemning.

Dette er en adfærdsrettelse i forskningsmodellen med ændret Preview-outputtype.
Generér en ny skabelon og flyt kun kendte inputfakta; ret ikke gamle
fingeraftryk. Direkte `.runa`-brugere af `MedIndgåendeÆgtefælleoverførsler`
skal erstatte de to gamle personfradragsresultater med
`afsenders_personfradragsgrundlag`, beregnet fra det eksakte resultat.
`personskat_indgående_ægtefælleoverførsler` kræver nu også dette eksakte
afsenderresultat. Den offentlige indgang bruger det automatisk.

## Afgrænsning og fokuserede kontroller

De seks eksterne cases vedrører ordinær helårsbeskatning i 2025. De etablerer
ikke fuld konformitet for andre år, delår, udenlandsk lempelse, alle
underskudskombinationer eller komponentfordelingen mellem kommune og kirke.
Modellens [gyldighedsforbehold](personskat-validity.md) gælder stadig.

```sh
cargo test --quiet --test personskat_validity spouse_allowance_uses_recipient_rates_from_source_facts -- --exact --nocapture --test-threads=1
cargo test --quiet --test tax_parameter_domain personal_allowance_transfer_matches_native_execution -- --exact --test-threads=1
cargo test --quiet --test tax_parameter_domain personal_allowance_recipient_offsets_obey_cohabitation -- --exact --test-threads=1
```

Ved modelændringer kan `FUTURUNA_MODEL_TEST_RUNA` pege på en allerede
verificeret compiler fra samme compilerrevision. Den anden test sammenligner
fortolkning og native udførelse af syv små samordnings-/overførselskontroller.
Den tredje kontrollerer to §10-integrationstilfælde fortolket. Native kontrol
af hele §10-modulet rammer eksisterende problemer med betingede tabelopslag
(`td-124b83`); der påstås ikke native dækning af hele Personskat.
