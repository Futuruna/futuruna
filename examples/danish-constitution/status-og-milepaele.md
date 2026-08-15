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

#### Klassifikation af de 23 topologiske hulfamilier

Topologirapporten grupperer nulargumentregler efter navne, ikke efter retligt
emne. En hulfamilie er derfor et arbejdssignal og ikke i sig selv et udækket
retligt spørgsmål.

| Hulfamilie | Klassifikation | Beslutning |
| --- | --- | --- |
| `bestemmelser` | Otte meningsfulde auditegenskaber, der kaldes gennem `bestemmelser_audit_består`. Topologiprøvningen følger ikke aggregatets afhængigheder. | Teknisk rapporteringshul; alle otte egenskaber prøves af den lokale og den samlede audit i begge backends. |
| `folketinget` | Navnebaseret samlegruppe af 14 forskellige kompetencer og organisationsudsagn. | Bevidst åben; gennemgås pr. kodespænd og lukkes ikke med én tautologi. |
| `folketingsåret` | To faste kalenderudsagn uden variabelt sagsdomæne. | Bevidst udækket, indtil en konkret kalenderforespørgsel kræver en model. |
| `grundloven` | To atomiske udsagn om geografisk rækkevidde og gældende ret. | Bevidst åben; deres indhold prøves gennem konkrete rigsdele og stadfæstelsesakten frem for ved gentagelse af fakta. |
| `ingen` | Skattepålæg og udskrivning af mandskab er kun samlet af den fælles sætningsstart. | Bevidst åben; forskellige genstande samles ikke for at tilfredsstille navneheuristikken. |
| `kommissioner` | To beslægtede oplysningskompetencer. | Kandidat til en lukket adressattype i prøvningslaget. |
| `kongen` | Navnebaseret samlegruppe af 16 forskellige roller og kompetencer. | Bevidst åben; fysisk monark, embede og statsorgan må ikke samles for at tilfredsstille topologien. |
| `mandatfordeling` | Tre udtrykkelige hensyn i samme bestemmelse. | Kandidat til en lukket hensynstype og en dækningskontrol. |
| `mellemfolkelige` | To egenskaber ved de myndigheder, § 20 omtaler. | Kandidat til ét domæneobjekt; ingen tautologisk kontrol tilføjes. |
| `ministerråd` | To organisationskrav i § 18. | Kandidat til en `Ministerrådsordning` med intern regel. |
| `ministre` | Adgangsret og taleret i Folketinget. | Kandidat til et lukket rettighedsdomæne. |
| `ministrene` | Regeringsansvar og medlemskab af Statsrådet fra forskellige bestemmelser. | Bevidst åben; udsagnene er retligt forskellige trods samme grammatiske subjekt. |
| `procedurer` | Seks grænse- og samtykkeegenskaber, der kaldes gennem `procedurer_audit_består`. | Teknisk rapporteringshul; alle seks egenskaber prøves lokalt og samlet i begge backends. |
| `regeringsmyndigheden` | To strukturelle udsagn om begrænsning og udøvelse. | Bevares foreløbig som tekstnære fakta; relationen genprøves tværgående. |
| `rettigheder` | Syv lukkede rettighedsegenskaber, der kaldes gennem `rettigheder_audit_består`. | Teknisk rapporteringshul; alle syv egenskaber prøves lokalt og samlet i begge backends. |
| `revisorer` | To kompetencer ved statsregnskabet. | Kandidat til en lukket revisionskompetencetype. |
| `rigsretten` | Kompetencen i ministersager og delegationen af den nærmere ordning. | Bevidst åben; kompetence og lovdelegation er ikke alternative værdier i samme domæne. |
| `statsministeren` | Tre pligter fra forskellige bestemmelser samlet alene af navnet. | Bevidst åben og behandles som tre uafhængige kildespørgsmål. |
| `statsydelse` | Lovfastsættelse og gældsforbud i § 10. | Kandidat til en samlet `Statsydelsesordning`, hvis en konkret forespørgsel kræver relationen. |
| `tilsyn` | Civil og militær forvaltning som to udtrykkelige områder. | Kandidat til en lukket tilsynsområdetype. |
| `tvaergaaende` | Seks tværgående egenskaber, der kaldes gennem `tvaergaaende_audit_består`. | Teknisk rapporteringshul; alle seks egenskaber prøves lokalt og samlet i begge backends. |
| `valg` | Tre valgprincipper og ét særskilt forholdstalsvalg samlet af navnet. | Splittes efter retligt emne; de tre principper er kandidat til et lukket domæne. |
| `årpenge` | Lovfastsættelse og samtykkekrav ved nydelse uden for riget i § 11. | Kandidat til en samlet `Årpengeordning`, hvis relationen senere skal forespørges samlet. |

### Milepæl 5: Fortolknings- og prøvningslag

Gennemført den 16. august 2026.

- Den gamle samlede audit på over 1.200 linjer er erstattet af fire fokuserede
  filer for bestemmelser, procedurer, rettigheder og tværgående relationer.
  De indeholder tilsammen 27 meningsfulde egenskaber og eksporterer hvert sit
  samlede auditresultat til den tynde `grundlov.audit.runa`.
- Rodfilen beviser alle fire importerede resultater eksplicit. Arbejdet
  afdækkede, at importerede scriptkontroller ikke udføres automatisk, og at en
  importeret invariant kan passere frontendkontrol og derefter give et
  fortolkerpanik. Compilerfejlen er registreret som `td-55d565`; korpusset
  bruger den almindelige, eksporterbare regelgrænse.
- `troskrav_gælder_ikke_tronfølger()` er fjernet fra § 6. Ordlydens tavshed om
  tronfølgeren er ikke længere gjort til en positiv undtagelse; kildemodellen
  siger alene, at kravet er formuleret for kongen.
- To navngivne fortolkningsmodeller sammenligner henholdsvis §§ 6/70 og
  §§ 43/73. Ingen model vælges skjult, og modellerne er udtrykkeligt markeret
  som arbejdshypoteser uden supplerende retskilder.
- En ny scenariefil har syv kontroller af eksplicitte fortolkningsvalg. Alle
  13 scenariefiler med i alt 182 invarianter består både fortolket og som
  genereret Rust med fire samtidige processer.
- Alle fem auditindgange består både fortolket og som genereret Rust. Den
  samlede indgang prøver de 27 underliggende egenskaber gennem fire
  eksporterede aggregatregler.
- Seks væsentlige fund har typet metadata med modellag, udsagnsstatus,
  prøvningsomfang, afgrænsning og i alt 17 kontrollerede
  `ProgramReference`-henvisninger. Metadataindekset har nu 97 ankre, 97
  kodespænd og ingen fejlmeddelelser: 91 kildetekstblokke og seks
  prøvningsfund.
- Den officielle ordlyd har fortsat kontrolsummen
  `0x5c1bbaf1142e3696`, og kilde- og metadataporten består 4 af 4 kontroller.
- Topologiprøvningen viser 165 regler, 23 klassificerede hulfamilier og ingen
  rapporterede paradokser, spændinger eller asymmetrier. Fire huller skyldes,
  at den eksperimentelle analyse ikke følger auditaggregaternes afhængigheder;
  seks andre blev synlige, da den gamle audits tautologiske fakta-gentagelser
  blev fjernet.

## Nu

- Bevar den officielle ordlyd og kildekoblingen uændret.
- Omskriv de danske grundlovssider, så ordlyd, kildemodel, navngivne
  fortolkninger og afgrænset prøvning vises i den rækkefølge.
- Fjern de forældede betegnelser, optællinger og bastante påstande fra den
  offentlige fremstilling.

## Næste

- Gennemfør den samlede juridiske og tekniske læsning af alle 91
  tekst-og-kodepar.
- Kør den endelige fortolker-, Rust-, metadata-, kilde-, web- og visuelle
  udgivelsesport.

## Senere

- Tilføj særskilte officielle forarbejder, domme eller statsretlige kilder,
  hvis en arbejdshypotese senere skal løftes til en
  `RetskildestøttetKonklusion`.
