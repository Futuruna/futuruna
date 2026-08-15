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

### Milepæl 2: Kapitel IV-V

Gennemført den 16. august 2026.

- § 29 samler de fem valgretsbetingelser i `Valgretsforhold` og skelner den
  gældende valgretsstatus fra Grundlovens delegation af omfanget ved straf og
  understøttelse, der betragtes som fattighjælp.
- Valgbarhed, nyvalg og mandaters beståen afledes af navngivne forhold frem
  for løse booleske hjælpefakta.
- §§ 39-42 bruger fælles, valideret `Medlemsopbakning`, men bevarer særskilte
  sagstyper for mødeindkaldelse, udsættelse og folkeafstemning.
- Lovforslagsmodellen dækker alle udtrykkelige undtagelser i §§ 41-42,
  formkravene til begæringer, behandlingskravet, fristerne, den særlige
  § 19-undtagelse og strakslovens retsvirkning frem til kundgørelse.
- §§ 45-46, 49-50, 53 og 57 har navngivne forløb eller sager for finanslov,
  bevillingshjemmel, lukkekrav, beslutningsdygtighed, samtykke og immunitet.
  § 49 kræver udtrykkeligt det antal medlemmer, som forretningsordenen
  bestemmer, og § 57 lader kun fersk gerning undtage indgrebsimmuniteten, ikke
  ytringsansvaret.
- Tre nye scenariefiler har 30 meningsfulde invarianter. Sammen med milepæl 1
  består alle syv scenariefiler med i alt 99 invarianter parallelt.
- Den samlede audit har 212 bestående invarianter og består både fortolket og
  som genereret Rust.
- Den aktuelle topologiprøvning viser 24 dækningshuller mod 41 efter milepæl
  1. Faldet skyldes især, at parallelle nulargumentregler er samlet i typede,
  parametriske regler; det er ikke i sig selv evidens for 17 løste retlige
  spørgsmål.
- De 91 kildetekstblokke har fortsat kontrolsummen
  `0x5c1bbaf1142e3696`; metadataindekset har 91 ankre, 91 kodespænd og ingen
  fejlmeddelelser.

### Milepæl 3: Kapitel VI-VIII

Gennemført den 16. august 2026.

- §§ 59-65 modellerer Rigsrettens grund- og sagssammensætning, tiltaler og
  tiltalte, domstolsordning, øvrighedsprøvelse, dommerindgreb og
  retsplejeprincipper som navngivne sager og regelscopes.
- §§ 67-70 skelner religionsudtryk, personlig bidragspligt,
  ligebehandlingsgrund, rettighedstype og virkning. Afstamning har særskilte
  konstruktørnavne i §§ 70 og 71, så samlet import ikke afhænger af importorden.
- § 71 har særskilte modeller for frihedsberøvelsesgrund, anholdelsesforløb,
  den grønlandske undtagelse, anke, varetægt, administrativ prøvelse og
  Folketingets tilsyn. §§ 72-73 modellerer indgreb, kendelse, lovundtagelse,
  ekspropriationsbetingelser, udsættelsesbegæring og prøvelsesvej.
- §§ 74-85 bevarer forskellen mellem "skal" og "bør", samler forenings- og
  forsamlingsindgreb i lukkede domæner og gør § 85's henvisning til §§ 71, 78
  og 79 prøvbar uden at udlede, at andre rettigheder derfor er ubegrænsede.
- Den tværgående audit påstår ikke længere uden fortolkningsgrundlag, at én
  bestemmelse er "stærkere", at § 73 er en eksplicit undtagelse fra § 61,
  eller at en skat på 100 procent nødvendigvis er grundlovsmæssig. Sådanne
  forhold er nu tekstlige forskelle eller navngivne fortolkningsspørgsmål.
- Fire nye scenariefiler har 64 meningsfulde invarianter. Alle 11
  scenariefiler med i alt 163 invarianter består både fortolket og som
  genereret Rust med fire samtidige processer.
- Den samlede audit har 210 bestående invarianter og består både fortolket og
  som genereret Rust.
- Den aktuelle topologiprøvning viser 14 dækningshuller mod 24 efter milepæl
  2. Faldet skyldes især kanoniske sagsmodeller og fjernede parallelle regler;
  det er ikke i sig selv evidens for ti løste retlige spørgsmål.
- De 91 kildetekstblokke har fortsat kontrolsummen
  `0x5c1bbaf1142e3696`; metadataindekset har 91 ankre, 91 kodespænd og ingen
  fejlmeddelelser. Kilde- og metadataporten består 4 af 4 kontroller.
- En frontend/backend-uoverensstemmelse for uløste recordmetoder blev isoleret
  som compilerfejl `td-f82dfa`; kildemodellen bruger det idiomatiske interne
  regelscope og ingen kompatibilitetsalias.

### Milepæl 4: Kapitel IX-XI

Gennemført den 16. august 2026.

- § 86 er én typet hovedregel for kommunale råd og menighedsråd med en
  navngiven undtagelse for Færøerne og Grønland. Modellen skelner mellem at
  følge folketingsvalgretsalderen til enhver tid og at blive fastsat ved lov
  eller i henhold til lov.
- § 87 er ikke længere et ubetinget udsagn om alle islandske statsborgere.
  `IslandskRettighedssag` kræver både lige ret efter den nævnte ophævelseslov,
  grundlovshjemmel og tilknytning til dansk indfødsret.
- § 88 er opdelt i `Grundlovsforslagsforløb`,
  `Grundlovsafstemningsresultat`, `Grundlovsafstemning` og
  `Grundlovsændringssag`. Reglerne skelner procedureudløseren fra det fuldførte
  forløb, afviser inkonsistente stemmetal og bevarer den strenge
  flertalsgrænse, den inklusive 40-procentgrænse, halvårsfristen, den direkte
  afstemning og den kongelige stadfæstelse.
- § 89 bruger to overgangsfaser. Den nye grundlov er i kraft i begge, mens den
  hidtidige rigsdag og de tidligere rigsdagsbestemmelser kun består før
  nyvalget efter kapitel IV. Grundloven af 1915 og ændringen af 1920 er samlet
  i ét historisk forfatningsgrundlag.
- Stadfæstelsesteksten har én typet akt med dato, sted, monark, kongelig hånd
  og kongeligt segl i stedet for tre parallelle fakta.
- En ny scenariefil har 12 meningsfulde invarianter. Alle 12 scenariefiler med
  i alt 175 invarianter består både fortolket og som genereret Rust med fire
  samtidige processer.
- Den samlede audit har 191 bestående invarianter og består både fortolket og
  som genereret Rust. Tværgående sammenligninger beskriver konkrete
  procesforskelle uden at rangordne bestemmelser som stærkere eller svagere.
- Topologiprøvningen viser 134 regler, 13 hulfamilier og ingen rapporterede
  paradokser, spændinger eller asymmetrier. Faldet fra 14 skyldes især den
  kanoniske stadfæstelsesakt, ikke et nyt retligt bevis.
- De 91 kildetekstblokke har fortsat kontrolsummen
  `0x5c1bbaf1142e3696`; metadataindekset har 91 ankre, 91 kodespænd og ingen
  fejlmeddelelser. Kilde- og metadataporten består 4 af 4 kontroller.

#### Klassifikation af de 13 topologiske hulfamilier

Topologirapporten grupperer nulargumentregler efter navne, ikke efter retligt
emne. En hulfamilie er derfor et arbejdssignal og ikke i sig selv et udækket
retligt spørgsmål.

| Hulfamilie | Klassifikation | Beslutning |
| --- | --- | --- |
| `folketinget` | Navnebaseret samlegruppe af 12 forskellige kompetencer og organisationsudsagn. | Bevidst åben; gennemgås pr. kodespænd og lukkes ikke med én tautologi. |
| `folketingsåret` | To faste kalenderudsagn uden variabelt sagsdomæne. | Bevidst udækket, indtil en konkret kalenderforespørgsel kræver en model. |
| `kommissioner` | To beslægtede oplysningskompetencer. | Kandidat til en lukket adressattype i prøvningslaget. |
| `kongen` | Navnebaseret samlegruppe af 13 forskellige roller og kompetencer. | Bevidst åben; fysisk monark, embede og statsorgan må ikke samles for at tilfredsstille topologien. |
| `mandatfordeling` | Tre udtrykkelige hensyn i samme bestemmelse. | Kandidat til en lukket hensynstype og en dækningskontrol. |
| `mellemfolkelige` | To egenskaber ved de myndigheder, § 20 omtaler. | Kandidat til ét domæneobjekt; ingen tautologisk kontrol tilføjes. |
| `ministerråd` | To organisationskrav i § 18. | Kandidat til en `Ministerrådsordning` med intern regel. |
| `ministre` | Adgangsret og taleret i Folketinget. | Kandidat til et lukket rettighedsdomæne. |
| `regeringsmyndigheden` | To strukturelle udsagn om begrænsning og udøvelse. | Bevares foreløbig som tekstnære fakta; relationen genprøves tværgående. |
| `revisorer` | To kompetencer ved statsregnskabet. | Kandidat til en lukket revisionskompetencetype. |
| `statsministeren` | To pligter fra forskellige bestemmelser samlet alene af navnet. | Bevidst åben og behandles som to uafhængige kildespørgsmål. |
| `tilsyn` | Civil og militær forvaltning som to udtrykkelige områder. | Kandidat til en lukket tilsynsområdetype. |
| `valg` | Tre valgprincipper og ét særskilt forholdstalsvalg samlet af navnet. | Splittes efter retligt emne; de tre principper er kandidat til et lukket domæne. |

## Nu

- Bevar den officielle ordlyd og kildekoblingen uændret.
- Adskil lokale overensstemmelseskontroller, procedurekontroller,
  rettighedskontroller og tværgående undersøgelser i målrettede auditfiler.
- Flyt resterende scenarieværdier ud af den samlede audit, og erstat
  tautologiske kontroller med egentlige egenskaber eller en dokumenteret
  bevidst afgrænsning.
- Bevar fortolkningsspørgsmål som spørgsmål, indtil de får navngivne modeller,
  synlige forudsætninger og særskilte retskilder.

## Næste

- Giv væsentlige fund typet metadata, udsagnsstatus, prøvningsomfang og
  kontrollerede programhenvisninger.
- Modellér konkurrerende fortolkninger side om side for de udvalgte åbne
  spørgsmål uden skjult standardvalg.

## Senere

- Omskriv de danske grundlovssider, så ordlyd, model og fortolkning vises
  nøgternt og afledes fra det aktive korpus.
- Kør den samlede juridiske, fortolkede, genererede og visuelle
  udgivelseskontrol.
