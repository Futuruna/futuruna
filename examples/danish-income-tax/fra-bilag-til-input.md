# Fra bilag til beregningsinput

En korrekt beregning kræver, at det rigtige beløb kommer i det rigtige felt.
Dette gennemarbejdede **fiktive** eksempel viser løn, pension/ATP og renter
fra kildeuddrag til `beregn_personskat`, med en særskilt kildelog. En AI eller
et menneske klassificerer fakta; Futuruna beregner skatten. Eksemplet er ikke
en PDF-importør, en automatisk kontrol af bilagenes ægthed eller individuel
skatterådgivning. De typede beregninger er Preview.

## Prøv eksemplet

Brug den samme compiler, som bestod
[runtime-tjekket](../../website/public/ai-setup.md#tax-audit-runtime-check).
Med Node.js 18 eller nyere allerede installeret køres fra repoets rod:

```sh
node examples/danish-income-tax/bilag-demo.mjs "$RUNA_BIN"
```

Kørslen bruger de medfølgende [fiktive kildeuddrag](bilag-demo-kilder.json),
genererer schema/skabelon og udfører fire beregninger med én worker.
En femte sag tilbageholdes før beregning. Der oprettes en ny midlertidig
mappe uden for checkoutet; stien vises ved start. Ingen installation,
netværkskald eller personlige dokumenter indgår.

I mappen findes:

- `sources.json`: de opdigtede kildeuddrag med dokument- og linje-id.
- `guidance.json`: relevante spørgsmål, hjælp, enheder og kildespor fra den
  faktisk genererede kontrakt; opløs `source_group` gennem `source_groups`
  og `source_objects` for at finde de vedhæftede kilder.
- `mapping.json`: inputsti, værdi, kildelinjer og begrundelse; fravalgte
  dubletter/afdrag og den tilbageholdte sag er særskilte.
- `cases.json` og `results.json`: kanonisk input og uændret modeloutput.
- `resultat.txt`: den eksisterende danske visning med alle returnerede
  kontroller, forbehold og diagnostik.
- `summary.json`: kort oversigt, der skelner tidligere ekstern konformitet
  fra rene modelkontroller.

Scriptets afslutning med kode 0 betyder, at **eksemplets forventninger**
bestod, inklusive den forventede ugyldige ATP-sag. Det betyder ikke, at
alle sager har gyldige fakta eller at nogen årsopgørelse er godkendt.
Den særskilte resultatviser returnerer kode 2 for denne blandede batch.

## Hvad indtastes — og hvad indtastes ikke?

Alle profiler gælder 2025, en ugift person født 1. januar 1990, skattekommune
København og ingen kirkeskat. Fuld dansk skattepligt og DBO-hjemsted samt
fravær af andre indkomster, udgifter og pensionsudbetalinger er **udtrykkeligt
opdigtede fakta** i dokument F, ikke noget en rigtig skabelon fortæller os.
Eksemplerne er selvstændige profiler, ikke ændringer af samme lønpakke.
EP:4 bekræfter også almindelig bankpensionsbehandling uden særgodkendt tidligere
år. Det nye `bank_årsplacering` sættes ud fra denne fiktive bekræftelse, aldrig
som et foreslået standardvalg for virkelige bilag.

| Kildefakta | Input eller handling | Hvorfor? |
| --- | --- | --- |
| L:1: løn 600.000 kr.; P:1: privat ratebetaling 40.001 kr. | Løn 600.000; én privat PBL18-betaling 40.001 | Privat pension trækkes ikke fra lønfeltet; modellen beregner fradragene. |
| B:1: hele lånets renter 102.002 kr.; A:1: dokumenteret egen andel 50 % | `kapitalindkomst.renter.renteudgifter_kroner = 51001` | Egen andel, ikke hele lånets beløb. |
| B:3: egne renteindtægter 1.000 kr. | `renteindtægter_kroner = 1000` | Indtægt og udgift oplyses hver for sig, ikke først som et nettobeløb. |
| R:1: samme renteudgift 51.001 kr. gentaget på årsopgørelsen | Krydskontrol, ingen ny post | Samme udgift må ikke tælles igen. |
| B:2: afdrag 20.000 kr. | Udelades fra renter | Hovedstolsbetaling er ikke renteudgift. |
| E:1–4: kontant løn 621.000, eget pensionsbidrag 20.000, egen ATP 1.000; indberettet ordinær løn 600.000 | `lønmodtager.bruttoløn_kroner = 600000` | Lønlinjen er allerede efter de to bidrag; ingen anden subtraktion. |
| E:2/E:5 og EP:1: medarbejder 20.000 + arbejdsgiver 30.000 = brutto 50.000 | Én arbejdsgiveradministreret ratebetaling på 50.000 | Medarbejderens løntrukne andel bliver ikke en yderligere privat betaling. |
| EP:2–3: indeholdt AM 4.000, netto 46.000 | Indeholdt AM 4.000 ved bruttoposten | Netto er en kontrol af samme betaling, ikke endnu et bidrag. |
| T:1–3: samlet ATP 3.000, dokumenteret netto 2.760 | Én særskilt ordinær arbejdsgiver-ATP-post | Brug samlet bidrag, ikke blot eget løntræk, og oplys netto fra kilden. |

Lønafgrænsningen følger
[SKATs forklaring af AM-bidrag](https://skat.dk/borger/am-bidrag) og
[eIndkomst-vejledningens felt 13 og 46](https://info.skat.dk/data.aspx?oid=2233519).
Felt 13 kan rumme flere indkomstarter; dette eksempel angiver udtrykkeligt,
at lønlinjen kun indeholder ordinær løn. ATP-beløbene er illustrative
årsbeløb, **ikke** en udledning af en bestemt ATP-sats eller ansættelsesperiode.
Se også [pensionsguiden](pension-og-fradrag.md#løn-og-arbejdsgiverpension-brug-ikke-samme-beløb-to-gange).

Renteeksemplet forudsætter afklaret fradragsret og fordeling, ikke blot en
bankoverførsel. [SKATs rentevejledning](https://skat.dk/borger/fradrag/fradrag-for-renter)
og [modellens renteguide](skatdk-rentefradrag-ekstern.md) beskriver grænsen.
Scriptet afviser modstridende kildebeløb; det ændrer dem ikke for at passe
til en forventet skat. Alle beløb er hele kroner. Ved ørebeløb må man ikke
bare afrunde for at opfylde en heltalskontrakt; den eksisterende
[kildemapping](personskat-aarsopgoerelse-kildemapping.runa) skelner eksplicit
mellem direkte mapping, kontrolresultat, supplerende fakta og tab af ørepræcision.

## Manglende oplysninger: to forskellige grænser

**ATP mangler:** Arbejdsgivervarianten `atp-uoplyst` har stadig den kendte
løn og pension, men dokument T er ikke til rådighed. Eget ATP-løntræk er
ikke et grundlag for at gætte det samlede bidrag eller netto. Input bruger
`AtpUoplyst`; den kanoniske model skal returnere ugyldigt grundlag og
`slutskat_til_sammenligning_øre = null`. Andre tal i dette resultat er kun
diagnostik.

**Egen renteandel mangler:** Varianten `renteandel-uoplyst` har hverken
fordelingsdokument A eller årsopgørelseslinje R. Den sendes **ikke** til
`runa call`. Rentebeløbsfeltet er et obligatorisk heltal og kan ikke selv
udtrykke »ukendt«. Kildeloggen bevarer spørgsmålet om egen andel.
Dette er interviewets stop, ikke en modelkontrol, som automatisk ville
opdage et forkert positivt beløb. Nul eller en antaget halvdel ville
kunne give en misvisende beregning.

## Hvad er faktisk kontrolleret?

Privatpensions- og renteprofilen sammenholdes med allerede registrerede,
uafhængige observationer fra SKATs 2025-beregner:

- Privat ratebetaling 40.001 kr.: **196.611,94 kr.** i samlet modelleret skat;
  ekstra pensionsfradrag 4.801 kr.
  [Registreret pensionsobservation](skatdk-pensionsfradrag-ekstern.md).
- Egen renteudgift 51.001 kr. og renteindtægt 1.000 kr.: **196.194,30 kr.**
  i samlet modelleret skat.
  [Registreret renteobservation](skatdk-rentefradrag-ekstern.md).

Der foretages ingen ny ekstern observation ved kørsel. Forventningerne
læses aldrig ind som personfakta. Arbejdsgiver-/ATP-profilen kontrollerer
fordelingen af løn, pension og ATP gennem den eksisterende model, herunder
arbejdsfradragsgrundlag 653.000 kr. og ekstra pensionsfradrag 5.852 kr.; dens
samlede skat er **ikke** et bekræftet eksternt match. En efterfølgende
[kontrol af den offentlige 2025-formular](skatdk-arbejdsgiverpension-ekstern.md)
gav 211.866,52 kr. mod modellens 210.569,32 kr. for ratepension/ATP-profilen.
Afvigelsen er uafklaret og vises nu i demoens korte oversigt samt modellens
forbehold. En særskilt livrenteprofil matcher; det giver ikke ret til at
omklassificere demoens dokumenterede ratepension for at få samme resultat.
Beløbene er skat inklusive løn-AM før betalingsafregning, ikke en lovet
tilbagebetaling eller et beløb, der skal indbetales.

Eksemplet gør en eksplicit, manuelt udarbejdet klassifikation reproducerbar.
Det må ikke beskrives som bevis for, at vilkårlige AI'er læser rigtige PDF'er
korrekt. Det autentificerer heller ikke bilag eller beviser fuld lovdækning.
De kendte [afrundingsforbehold](skatdk-fradrag-oere-ekstern.md#kendte-afvigelser--ikke-match)
består. Ingen skatteformel eller inputtype er ændret af dette eksempel.

## Brug mønstret i din egen gennemgang

Bed din AI om at bevare én privat kildelog med:

1. Dokument, år, side/linje og originalt beløb/enhed.
2. Valgt felt og variant fra en frisk kontrakt samt begrundelse i dens hjælp.
3. Eventuel kildebaseret fordeling — og hvorfor samme beløb ikke tælles igen.
4. Ukendte forhold og det næste nødvendige spørgsmål; ikke skabelonens nulværdi.
5. Modelresultat og forbehold adskilt fra observerede beløb på årsopgørelsen.

Brug ikke demoens kildepakke eller fiktive bekræftelser som dine egne fakta.
Hold dokumenter og resultater uden for repoet. Hvis nødvendige fakta ikke
kan skaffes, kan [betinget rapportafstemning](aarsopgoerelse-afstemning.md)
stadig besvare snævrere spørgsmål uden at påstå en uafhængig genberegning.
