# Status og milepæle for grundlovsmodellen

Denne log gør arbejdsplanens fremdrift efterprøvbar. Den beskriver den aktive
model af Danmarks Riges Grundlov; det historiske korpus er ikke omfattet.

## Fastfrosset udgangspunkt

Udgangspunktet er registreret den 15. august 2026 ved Git-revision `84041bd7`.

- Primærkilde: Danmarks Riges Grundlov, lov nr. 169 af 5. juni 1953,
  <https://www.retsinformation.dk/eli/lta/1953/169>.
- 91 officielle tekstblokke: indledning, §§ 1-89 og stadfæstelse.
- 91 entydige kildeetiketter med 91 sammenhængende kodespænd.
- Kontrolsum for den normaliserede ordlyd: `0x5c1bbaf1142e3696`.
- 219 bestående invarianter i den hidtidige samlede prøvning.
- 372 registrerede regler, heraf 365 uden parametre og 7 med parametre.
- 49 topologiske dækningshuller og ingen rapporterede paradokser,
  spændinger eller asymmetrier i udgangspunktets automatiske topologiprøvning.
- 91 metadatahenvisninger, 91 ankre, 91 kodespænd og ingen
  metadatafejlmeddelelser.
- Den kompakte, maskinlæsbare oversigt over udgangspunktets labels,
  kodespænd og 443 symboler ligger i `grundlov-baseline.meta.json`.

Tallene er et sammenligningsgrundlag, ikke et mål i sig selv. Særligt betyder
219 bestående invarianter ikke, at alle 219 prøver en selvstændig retlig
egenskab. Arbejdet skal erstatte tautologier og parallelle sandhedskilder med
meningsfulde domæneegenskaber.

## Spørgsmål modellen skal kunne besvare

Domænemodellen udvikles ud fra konkrete statsretlige spørgsmål:

1. Hvor placerer ordlyden hver statsmagt, hvilke statsorganer deltager, og
   udøves magten hos ét organ eller i forening?
2. Hvornår betegner "kongen" den fysiske monark, et embede eller et
   statsorgan, og hvilke regler kan lovligt forbinde rollerne?
3. Hvilke krav gælder for monarken og tronfølgeren ved tronfølge,
   myndighed, trossamfund, forsikring og regentskab?
4. Hvilke beslutninger kræver samtykke, kontrasignatur, flertal,
   folkeafstemning eller efterfølgende forelæggelse?
5. Hvem har kompetence, pligt og ansvar i forholdet mellem kongen,
   ministrene, Folketinget, Statsrådet og domstolene?
6. Hvilke frister og tærskler gælder i lovgivningsprocessen, og hvad sker der
   præcis på hver side af en grænse?
7. Hvilke rettigheder, indgreb, hjemmelskrav, undtagelser og
   prøvelsesmuligheder følger direkte af ordlyden?
8. Hvilke udsagn er tekstnære, hvilke er modelresultater, og hvilke kræver en
   navngiven fortolkning eller en supplerende retskilde?
9. Er hver regelkaskade opfyldelig, dækkende og entydig i sit erklærede
   domæne, og kan dens undtagelser faktisk nås?
10. Kan hvert offentligt fund føres tilbage til ordlyd, kodespænd,
    fortolkningsmodel og prøvningsomfang?

## Gennemført

### Milepæl 1: Fælles ordforråd og kapitel I-III

Gennemført den 16. august 2026.

- § 3 har én kanonisk, udtømmende relation mellem `Statsmagt`, `Statsorgan`
  og `Udøvelsesform`; alle deltageropslag afledes fra relationen.
- Den fysiske `Monark` og `Tronfølger` er adskilt fra
  `KongenSomStatsorgan`.
- § 8's lovbestemte ordning er en afgrænset undtagelse i selve
  tronskiftereglen, og scenarierne prøver alle dens grene.
- §§ 14, 15, 18-20, 22-25 og 27 bruger navngivne sager, betingelser og udfald
  frem for uafhængige booleske hjælpefakta.
- § 17-modellen siger ikke længere, at Statsrådet "fører forsædet" i
  undtagelsestilfældene; den positive påstand havde ikke grundlag i ordlyden.
- § 19 skelner mellem at opfylde bestemmelsens samtykkekrav og at være
  generelt retligt tilladt.
- Fire fokuserede scenariefiler med i alt 69 invarianter består parallelt.
- Den samlede audit har 213 bestående invarianter og består både fortolket og
  som genereret Rust.
- Den aktuelle topologiprøvning viser 41 dækningshuller mod udgangspunktets
  49. Faldet skyldes især fjernede parallelle og tautologiske regler og er
  derfor ikke i sig selv evidens for otte løste retlige spørgsmål.
- De 91 kildetekstblokke har fortsat kontrolsummen
  `0x5c1bbaf1142e3696`; metadataindekset har 91 ankre, 91 kodespænd og ingen
  fejlmeddelelser.

## Nu

- Bevar den officielle ordlyd og kildekoblingen uændret.
- Gennemgå kapitel IV-V som én model for valg, Folketingets arbejde og
  lovgivningsprocessen.
- Gør tærskler, frister, forslag, begæringer og procedureudfald typede og
  grænseprøvede uden at sammenblande deres retlige betydning.
- Erstat parallelle hjælpefunktioner i den samlede audit med afledte opslag
  fra de kanoniske kapitelregler.

## Næste

- Gennemgå kapitel VI-VIII som én model for domstole, trossamfund og
  grundrettigheder.
- Gennemgå kapitel IX-XI og alle tværgående henvisninger, frister og
  delegationer.
- Klassificer de 49 kendte dækningshuller som dækket eller bevidst udækket.

## Senere

- Adskil kildescenarier, fortolkningsscenarier og prøvninger i de planlagte
  `.scenario.runa`- og `.audit.runa`-filer.
- Giv væsentlige fund typet metadata og kontrollerede programhenvisninger.
- Omskriv de danske grundlovssider, så ordlyd, model og fortolkning vises
  nøgternt og afledes fra det aktive korpus.
- Kør den samlede juridiske, fortolkede, genererede og visuelle
  udgivelseskontrol.
