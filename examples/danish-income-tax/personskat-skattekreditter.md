# Foreløbige skatter: kredit er ikke altid kontant betaling

Slutskat er den beregnede skat. Først derefter modregnes årets foreløbige
skatter og øvrige kreditter i slutskatten med eventuelle tillæg. Resultatet er
restskat eller overskydende skat; den senere betalingsafregning er et særskilt
trin. Modellen er forskningssoftware, og beregningskontrakten er Preview.

## Vælg kilde før beløb

[Kildeskatteloven § 60](https://www.retsinformation.dk/eli/lta/2024/460/pdf)
skelner mellem forskellige kreditgrundlag. Derfor er en liste over faktiske
bankbetalinger ikke tilstrækkelig til at udfylde alle felterne:

| Feltets grundnavn | Det relevante kildebeløb |
| --- | --- |
| `a_skat_og_am_indeholdt` | Årets indeholdte A-skat og AM-bidrag, ikke beregnede skatter. |
| `par68_indbetalt` | Årets pligtige beløb efter § 68, ikke kun det faktisk betalte. |
| `b_skat_betalt` | Årets pålignede skattebilletbeløb, inklusive relevant foreløbigt AM-bidrag én gang. |
| `frivillig_indbetaling_par59` | Faktisk frivillig indbetaling, men kun det godskrevne skattebeløb efter eventuel indeholdt § 59-rente. |
| `tilbagebetalt_par55` | Tilbagebetalt foreløbig skat efter § 55, som modellen fratrækker én gang. |

Feltnavnene `b_skat_betalt_*` og `par68_indbetalt_*` er historiske maskinnøgler,
ikke en regel om kun at medregne kontant betaling. Sondringen mellem pålignet
B-skat og restskat ses også i
[denne offentliggjorte henstandsafgørelse](https://info.skat.dk/data.aspx?oid=2459943).
At B-skat modregnes i årsopgørelsen beviser ikke, at raterne er betalt eller
at der ikke findes særskilte restancer. Denne indgang opgør ikke hele personens
gælds- eller betalingshistorik.

Opret en privat kildelog med indkomstår, dokument/linje, oprindeligt beløb,
enhed, valgt felt og begrundelse. En gentaget oplysning i bankudtog og
årsopgørelse er ikke en ny kredit. Kopiér heller ikke subtotalen forskudsskat
ind som A-skat/AM oven i de enkelte kreditter. Kontrollér, om en samlet post
allerede indeholder AM eller allerede er reduceret med en tilbagebetaling.

Brug `MedEksaktÅrsopgørelse` og `_øre` for eksakte beløb: 12,34 kr. er 1234 øre.
Den ældre `MedÅrsopgørelse` bruger `_kroner`; afrund ikke kildens øre vilkårligt
for at passe til den. Manglende beløb er ikke nul. Kan en nødvendig kredit
ikke afklares, skal den fulde afregning vente. En
[betinget rapportafstemning](aarsopgoerelse-afstemning.md) kan stadig besvare
snævrere spørgsmål, men etablerer ikke kreditgrundlaget uafhængigt.

## Fiktivt eksempel på fejlen

Den [fokuserede regression](../../tests/tax_credit_input.test.mjs) bruger en
fiktiv almindelig 2025-profil med 600.000 kr. i løn. Den eksisterende models
slutskat er 211.944,54 kr. Alle øvrige forhold er udtrykkeligt opdigtede;
beløbet er ikke et nyt uafhængigt administrativt kontrolresultat.

Kilderne angiver 105.000 kr. indeholdt A-skat, 45.000 kr. indeholdt AM,
40.000 kr. efter skattebillet, 10.000 kr. efter § 68, en frivillig betaling på
2.100 kr. inklusive 100 kr. rente og en § 55-tilbagebetaling på 1.000 kr.
Kreditterne bliver 201.000 kr.; restskatten før betalingsafregning bliver
10.944,54 kr.

Banken viser kun 30.000 kr. betalt på skattebilletten. Bruges det beløb i
stedet for de pålignede 40.000 kr., bliver den modellerede restskat fejlagtigt
20.944,54 kr. Slutskatten er uændret. De manglende ratebetalinger må ikke
omdannes til yderligere restskat ved at ændre kreditten.

Testen holder også § 68-bankbetaling, § 59-rente og beregnet AM adskilt fra
de korrekte kildebeløb. De forkerte positive tal kan stadig være gyldige
heltal og bestå modelkontrollerne. Det er netop inputrisikoen: testen er
ikke bevis for automatisk dokumentlæsning, kildeægthed eller fuld lovdækning.

## Frisk kontrakt og uændrede fakta

Spørgsmål, hjælp, enheder og kilder er knyttet til de to kredittyper med
typede meta-ankre. Vejledningen følger derfor typerne gennem indlejrede input,
ikke kun ét hårdkodet felt i Personskat.

Ændringen retter vejledningen; skatteformler, inputtyper og maskinnøgler er
uændrede. Metadata indgår i kontrakthashen, så generér frisk schema/skabelon.
Gennemgå eksisterende kreditbeløb mod deres kilder før overførsel. Ret ikke
beløb for at få et bestemt resultat, og genbrug ikke eksemplets opdigtede fakta.
