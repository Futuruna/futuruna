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

### Afstem A-skat/AM-totaler uden dobbelttælling

I [kildemappingen](personskat-aarsopgoerelse-kildemapping.runa) kræver
`personskat_aarsopgoerelse_kortlaeg_a_skat_og_indeholdt_am(linjer)` afklaring
af linjernes art og årsgrundlag. En etiket er ikke en klassifikation.

Når kilderne er afklaret, bruges
`personskat_aarsopgoerelse_kortlaeg_afklaret_indeholdelse(poster, indkomstår)`.
Hver `PersonskatÅrsopgørelseIndeholdelsespost` bevarer `kildelinje` og angiver
`art`: `ÅretsIndeholdteASkat`, `ÅretsIndeholdteAm` eller
`ÅretsSamledeIndeholdteASkatOgAm`. Det er årstotaler for samme person og
dokument, ikke enkelte arbejdsgiver- eller månedsrækker.

En dokumenteret samlet A-skat/AM-total kan stå alene. Ellers kræves begge
komponenter; manglende AM bliver ikke nul. Vises både total og komponenter,
afstemmes de uden dobbelttælling. Fiktivt: 105.000 kr. A-skat, 45.000 kr. AM
og en samlet post på 150.000 kr. giver 150.000 kr. i kredit og bevarer alle
tre observationer. En enkelt komponent ved siden af en samlet post må ikke
overstige totalen; en manglende komponent udledes ikke som en ny kildeoplysning.

`IndeholdelsesartUoplyst` kræver afklaring. `AndetSkatteEllerKreditbeløb`
hører ikke til denne indgang: en bred forskudsskattesum eller beregnet AM
må ikke omklassificeres for at få et resultat. Modstridende totaler, gentagne
årstotaler af samme art, dublerede linjeidentifikationer, forkert år og negative
rettelsesbeløb giver ingen direkte mapping. Originale linjer og eksakte øre
bevares, også ved afvisning. Selv `DirekteMapping` beholder faktakrav om
person, klassifikation og fuldstændighed; hjælperen autentificerer ikke bilag.
Den ændrer ikke den kanoniske kreditkontrakt og læser ikke PDF-filer.
Se det [eksekverbare fiktive eksempel](../../tests/personskat_withholding_mapping_test.runa)
for komplette linjer og afstemningskontroller.

## Fortegn og rettelser

De rå kreditfelter er **ikke-negative bruttobeløb**. § 55-feltet angiver også
tilbagebetalingens størrelse uden minus; modellen foretager fradraget efter
§ 60, stk. 3. Et negativt felt tilbageholder nu den kanoniske sammenligning
med en fejl ved `årsopgørelse.kreditter`, også i den ældre hele-kroneindgang.
Kontrollen tilhører `kontrolgrundlag.beregning`, ikke den senere betalingsfase.

Hvis en rapport viser »tilbagebetalt: -1.000 kr.« som en fradragslinje, skal
AI'en først afklare, at det er en § 55-tilbagebetaling, og registrere både
original linje og feltets positive bruttobeløb i kildeloggen. En ukendt negativ
post må ikke automatisk blive 1.000 kr., nul eller en anden kreditart.
Et negativt forskelsbeløb fra en rettelse er heller ikke en ny årstotal;
brug det dokumenterede korrigerede grundlag eller afklar korrektionsforløbet.

Dette er inputkonventionen for disse felter, ikke et generelt forbud mod
negative indkomster eller korrektioner. En negativ § 55-værdi vendte tidligere
fradraget til ekstra kredit: -1.000 kr. gav 2.000 kr. mindre restskat end en
korrekt oplyst tilbagebetaling på 1.000 kr. Rå tal bevares som diagnostik ved
afvisningen; modellen tager ikke absolut værdi og omskriver ikke kilderne.

Fortegnskontrollen beviser ikke kreditternes størrelse, indkomstår eller
fuldstændighed. Den indfører heller ikke en juridisk grænse, hvor § 55-refund
altid skal være mindre end de nuværende bruttokreditter. Korrektionsforløb og
modellens nulafgrænsning af en mulig negativ nettokredit kræver særskilt
afklaring (`td-145088`); det er ikke dækket af denne kontrol.

## Tidligere udbetaling er ikke automatisk § 55

KSL § 60, stk. 3, fratrækker tilbagebetalt **foreløbig skat efter § 55**.
En tidligere udbetaling af årsopgørelsens overskydende skat efter §§ 62/62 A
er et andet forhold. Etiketten »tidligere udbetalt overskydende skat« kan
ikke alene afgøre, hvor beløbet hører til.

I [kildemappingen](personskat-aarsopgoerelse-kildemapping.runa) kræver
`personskat_aarsopgoerelse_kortlaeg_tidligere_udbetalt_overskydende_skat(linje)`
derfor nu afgørelse/betalingsopgørelse og afklaring af hjemmel og år. Den
tidligere direkte mapping til § 55 var forkert. Hverken positivt beløb,
minus, nul eller en bestemt linjeetiket ophæver afklaringskravet.

Når kilden faktisk dokumenterer § 55-forskudsskat, kan man bruge
`personskat_aarsopgoerelse_kortlaeg_tilbagebetalt_forskudsskat_par55(linje, indkomstår)`.
Den bevarer eksakte øre og kræver samme indkomstår og ikke-negativ størrelse.
Manglende kildeidentifikation, forkert år eller negativt beløb giver
`UgyldigeKildelinjer`; original linje bevares. Ved en negativ dokumentlinje
skal den dokumenterede omregning til et positivt bruttoinput foretages og
forklares særskilt i den private kildelog, ikke ved at ændre originalen.
Selv `DirekteMapping` beholder et faktakrav om § 55-hjemmel og om, at
bruttokreditterne ikke allerede er reduceret. Hjælperen autentificerer ikke
dokumentet eller disse påstande.

Er beløbet en allerede udbetalt årsrefusion, skal det holdes ude af
`tilbagebetalt_par55_øre`. Den
[betingede afstemning af en ændret rapport](aarsopgoerelse-afstemning.md#ændret-rapport-beløb-til-betaling-eller-udbetaling)
kan bruge en dokumenteret tidligere udbetaling som negativ post i
`betaling.korrektioner_til_udbetaling`. Posten må ikke også være fratrukket
det oplyste kreditgrundlag. Det er afstemning af rapportens observationer,
ikke en ny selvstændig beregning af renter eller hele korrektionshistorikken.

Den [eksekverbare regression](../../tests/personskat_refund_mapping_test.runa)
bevarer sondringen med fiktive linjer. Typede `Source`/`Warning`-metadata
knytter lovgrundlag og forbehold til begge hjælpere, så `runa meta --json`
kan følge dem. Den kanoniske `@ calculate`-kontrakt importerer ikke denne
mappingfil: dette er en rettelse af en interviewhjælper, ikke automatisk
PDF-læsning eller en ny kontrol af ethvert indsendt kreditbeløb.

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

Vejledningen og den efterfølgende fortegnskontrol bevarer skatteformler,
inputtyper og maskinnøgler. Negative bruttobeløb accepteres ikke længere som
grundlag for sammenligning. Metadata indgår i kontrakthashen, så generér frisk schema/skabelon.
Gennemgå eksisterende kreditbeløb mod deres kilder før overførsel. Ret ikke
beløb for at få et bestemt resultat, og genbrug ikke eksemplets opdigtede fakta.
