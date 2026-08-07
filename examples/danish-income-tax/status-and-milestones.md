# Personskatteloven as Futuruna

Status: active implementation; source-backed calculation gaps remain
Last updated: 2026-08-06
TD epic: `td-56cf8d`
Current implementation slice: `td-6fe980` (Kildeskattelovens § 60-kreditter og §§ 61-62-slutopgørelse har nu en ørepræcis kanonisk grænse; hele-krone-input bevares gennem en tabsfri kompatibilitetsadapter, og udbetaling/opkrævning afrundes først ved de udtrykkelige lovtrin)
Current LL § 9 A island-lodging slice: `td-e53d8f` (§ 9 A, stk. 12 er nu et typet årsforløb for bopæl på ikkebrofaste øer, faste arbejdssteder, umulig hjemmeovernatning og egne logiudgifter; fradraget bruger den officielle døgnsats, deler årsloft med rejser og dobbelt husførelse og føres gennem den kanoniske Personskat-beregning og XLSX-arbejdsbog)
Current LL § 9 foreign-income slice: `td-c61f53` (identificerede udenlandske lønindkomstkilder deles nu strukturelt af alle tilknyttede rejser; samme § 9, stk. 2-loft kan kun bruges én gang, mens manglende, dublerede, modstridende eller uanvendte kilder fejler lukket i regler og arbejdsbog)
Current LL § 9 A period slice: `td-3515b8` (12-månedersperioden udledes nu af rejsedatoer, tidligere perioderejser, daterede arbejdsdage og effektive afstande mellem arbejdssteder; 8 km, 40 arbejdsdage, privat hjemmearbejde og manglende afstande er verificeret i interpreter, compiler og XLSX)
Current LL § 9 A short-service slice: `td-bc07c9` (undtagelsen for enkeltstående kortvarige tjenesterejser udledes nu af rejseart, arbejdssted, dato, varighed og månedens øvrige rejser; godtgørelses- og fradragsperioder føres særskilt, og arbejdsbogen spørger ikke længere brugeren om en retlig konklusion)
Previous LL § 9 A granularity slice: `td-45f9fb` (kostprincippet gælder for hele rejsen, mens logidækning, godtgørelse og standard- eller dokumenteret fradrag afgøres pr. rejsedøgn; allerede lønopdelt overskud bevares som supplerende løn)
Previous ABL reporting slice: `td-3f2ee9` (ABL § 39 A's årlige oplysningspligt kontrolleres nu mod en typet kildehistorik med opgørelsesdato og dokumenterede fristudsættelser; manglende oplysninger afledes først efter den effektive frist og bliver et synligt ledgerresultat)
Previous negative share-tax ledger slice: `td-9ffd39` (uudnyttet negativ aktieindkomstskat efter Personskattelovens § 8 a, stk. 5-6, føres nu mellem Personskat-år som typede ejer- og årstrancher; årets negative skat anvendes før tidligere fremførsel, ældste tranche anvendes først, og hele ledgeren bevares gennem JSON/XLSX)
Previous simultaneous exit-tax slice: `td-421749` (samtidig fraflytning for samlevende ægtefæller bruger eksakte datoer og modregner kun ABL § 38-tab på tværs, når begge bliver fraflytterskattepligtige; én rolleuafhængig parberegning fordeler skatten uden dobbeltregning og bevarer kildeproveniens gennem JSON/XLSX)
Previous employee-expense slice: `td-80292f` (Ligningslovens § 9, stk. 1 og 3, modellerer typede, dokumenterede lønmodtagerudgifter, den fælles årsgrænse og repræsentationsbegrænsningen; fradraget føres gennem Sømandsbeskatningslovens § 4 til den kanoniske Personskat-beregning og arbejdsbog)
Previous DIS implementation slice: `td-80c439` (Sømandsbeskatningslovens §§ 5-8 klassificerer DIS, DAS og udenlandske skibe fra typede kildefakta; DIS-løn indgår i progression uden AM-bidrag, den forholdsmæssige lempelse beregnes pr. skattekomponent og føres til den kanoniske Personskat-slutskat og arbejdsbog)
Previous travel-expense slice: `td-d17087` (Ligningslovens § 9 A er et kildebåret, typet årsdomæne for rejser, godtgørelser og fradrag; den kanoniske Personskat-graf udleder Sømandsbeskatningslovens § 4, stk. 2-udelukkelse og fører skattepligtig godtgørelse samt fradrag til de rette skattegrundlag)
Previous seafarer-coordination slice: `td-2a827d` (Sømandsbeskatningslovens § 4 samordner LL §§ 9 B-9 D og 13 samt PBL § 49, stk. 1, ud fra typede kilde- og arbejdstilknytninger; blandede arbejdsår er verificeret uden dobbelt forholdsmæssig fordeling)
Previous seafarer-deduction slice: `td-302f8e` (sømandsfradraget efter Sømandsbeskatningslovens §§ 3-4 er typede beskæftigelsesfakta med valg, betingelser, undtagelser, delårsfordeling og kanonisk samordning; godkendt)
Previous implementation slice: `td-292327` (Personskattelovens § 13-fremførsel er et typet årsledger med kildeproveniens, sammenhængende indkomstår, afledt egen anvendelse, ægtefælleoverførsel og ultimo; implementeret og verificeret, afventer uafhængig gennemgang)
Previous ordinary-benefit slice: `td-9e236a` (erstatninger og ydelser fra faglige foreninger samt udbetalinger fra arbejdsløshedsforsikring er typede kildefakta; Ligningslovens §§ 13, 30 og 31 samt Pensionsbeskatningslovens §§ 49, stk. 2, og 55 afledes i den kanoniske Personskat-graf; implementeret og verificeret, afventer uafhængig gennemgang)
Previous common-deduction slice: `td-a47465` (faglige kontingenter, A-kasse/arbejdsløshedsforsikring, efterløn, fleksydelse og gaver er typede kildefakta; Ligningslovens §§ 8 A, 8 H, 12 og 13 samt Pensionsbeskatningslovens §§ 49-49 B afledes i den kanoniske Personskat-graf; godkendt)
Previous implementation slice: `td-1306f6` (ordinære arbejdsgiverydelser og virksomhedsresultater uden virksomhedsordningen er typede kildefakta; PSL § 3- og AM-virkninger afledes gennem reglerne; afventer uafhængig gennemgang)
Earlier implementation slice: `td-5beddd` (Personskats resterende rå KGL-årsnetto er erstattet af typede fordrings-, obligations- og ABL § 22-forløb; klassifikation, opgørelse, fælles bagatelgrænse og resultat afledes gennem reglerne; afventer uafhængig gennemgang)
Current source-document mapping slice: `td-5a52eb` (AI- eller menneskelæste årsopgørelseslinjer bevarer dokumentproveniens og eksakte ørebeløb; kun retligt tilstrækkelige linjer peger på typede beregningsfelter)
Previous per-holding provenance slice: `td-4e42fd` (hvert ABL-resultat bevarer sit eget typede markeds- og indberetningsgrundlag gennem § 38, § 13 A og KGL § 32; godkendt)
Previous mixed exit-tax slice: `td-8e8561` (blandede ABL § 38-porteføljer beregnes som den kanoniske årlige slutskatteforskel med særskilt henstand for realisationsposter; godkendt)
Previous ABL classification slice: `td-7469fc` (§ 38's aktiver og tab klassificeres fra kildefakta til personlig indkomst, kapitalindkomst eller aktieindkomst; godkendt)
Current fidelity slice: den anonymiserede årsopgørelse for 2025 afstemmer nu også boligskatterne til øret
Previous canonical integration slice: `td-aea204` (ABL §§ 37-40's signerede aktieindkomstkontekst afledes fra den kanoniske Personskat-graf; afventer uafhængig gennemgang)
Previous dependency slice: `td-86bd9a` (KGL § 32's identificerede kontrakter, flerårshistorik, tabsrækkefølge og ABL-afledte relationer er ført gennem den kanoniske Personskat-grænse; godkendt)
Earlier implementation slice: `td-0b0a4b` (ABL §§ 35 G-35 K's valg, kildehændelser og vedvarende overdragerskat er ført gennem den kanoniske Personskat-grænse; afventer uafhængig gennemgang)
Latest structural audit: `td-ba70c7` (kanonisk rækkevidde og afledte input er opgjort; afventer uafhængig gennemgang)
Planned monetary audit: `td-24963d` (enheder, afrundingstrin og ikke-additive delvirkninger)
Previous ordinary-income completion: `td-1306f6` (typede arbejdsgiverydelser og virksomhedsresultater gennem den kanoniske § 3-gren; implementeret og verificeret, afventer uafhængig gennemgang)
Current common-deduction completion: `td-a47465` (fagforening, A-kasse mv. og gaver med egne kildefakta, lovregler, beregningsmetadata og kanonisk samordning)
Current exact-credit completion: `td-6fe980` (den typede dokumentmapping og den kanoniske Personskat-arbejdsbog kan føre foreløbige skatter i eksakte øre gennem hele § 60-modregningen)
Planned result audit: `td-bf5e81` (trin-konsistens og bevarelsesinvarianter i sammensatte resultater)
Planned date-domain work: `td-01c72e` (fælles typede datoer med særskilte lovbestemte kalenderkonventioner)
Planned source-provenance audit: `td-091de2` (ændringslove, virkningstidspunkter og årsspecifikke regelhenvisninger)
Current ABL § 44 completion: `td-85ed17` (statusændringens anskaffelsesgrundlag, typet fondsaktielinje, årsvalidering og kanonisk Personskat-komposition er verificeret fortolket, kompileret og gennem XLSX-kontrakten)
Current residual share-tax completion: `td-b95e35` (den signerede § 8 a-parberegning føres nu til egen modregning, ægtefællemodregning, Kildeskattelovens § 60-kredit og eksplicit fremførsel uden at ændre det rå ABL-kildespor)
Current negative share-tax ledger: `td-9ffd39` (typede åbnings- og lukningssaldi fører den uudnyttede § 8 a-skat videre mellem indkomstår uden afledte skatteinput; implementeret og verificeret, afventer uafhængig gennemgang)
Previous spouse property-tax capacity: `td-d85f63` (ægtefællens typede ejendomsskattekilder, resultatproveniens og komplette slutskat indgår i § 8 a-overførslen; arbejdsbog, skema og rolleombytning er verificeret)
Current exit-tax projection correction: `td-44859f` (begge personers ABL §§ 37-40-projektioner bruger komplet slutskat efter modregning af årets og tidligere fremførte § 8 a-skat inklusive ejendomsskatter)
Planned KGL § 33 signed-value completion: `td-89a28a` (negative kontraktværdier og afregningsbeløb skal kunne krydse nul uden at blive afvist eller beskåret)
Planned KGL foreign-currency completion: `td-1e2380` (adskil kredit- og valutakomponenter efter KGL §§ 14 og 25)
Planned KGL partial-realization completion: `td-aac8d3` (typede delafdrag og gentagne realisationer uden rå årsnetto)
Current language support slice: `td-d25733` (genbrugelige, typede beregningsfeltreferencer er implementeret og afprøvet på CFC-domænet; pending independent review)
Current calculation-cache slice: `td-884d24` (validerede `@ calculate`-kontrakter genbruges på tværs af CLI-processer med en nøgle over compiler, prelude og hele det transitive importindhold; Personskat-skema falder fra 115,43 s koldt til 12,31 s varmt)
Latest compiler-artifact cache: `td-22e56f` (validerede `check`-resultater og native binære artefakter genbruges sikkert ud fra hele den transitive kildegraf; kanonisk Personskat-check falder fra cirka fire minutter koldt til 2,6 s varmt)
Current backend-check performance slice: `td-7b61da` (validerede, rene metadata-bindinger udelades fra Rust-runtimekoden, medmindre almindelig kode kan nå dem; den genererede Personskat-kode falder fra 12,91 MB til 11,65 MB, og isolerede kolde checks falder fra cirka 40,6 s til 31,0-33,4 s, mens et varmt artefakthit tager 0,07 s. Rustc-incremental er nu opt-in via `FUTURUNA_RUSTC_INCREMENTAL_CHECK=1`, og `FUTURUNA_COMPILER_TIMING=1` viser de enkelte compilerfaser)
Previous compiler performance slice: `td-95076b` (kompilerede RuleScopes memoiserer nul-argumentsregler under én rodevaluering; afventer uafhængig gennemgang)
Latest approved interpreter performance slice: `td-38f074` (uforanderlige runtimeværdier, RuleScope-bindinger og regel-AST'er deler struktur med copy-on-write, og globale samt scoped regler findes gennem kildeordnede navneindeks; det fokuserede LL § 9-scenarie faldt fra cirka 333 sekunder til 7-10 sekunder, mens den fulde Personskat-XLSX-regression faldt fra cirka 1.144 sekunder til cirka 387 sekunder i den målte referencekørsel)
Current typed-rule performance slice: `td-440cd4` (fortolkeren genbruger den parsede type for hvert særskilt typet regelhoved; en varm 26-sagers Personskat-XLSX-kørsel faldt fra 54,96 s til 27,89 s med byteidentisk JSON-resultat)
Current indexed-miss performance slice: `td-f18b54` (fortolkeren klassificerer nu hver regelfamilies resultat ved registrering i stedet for at gennemløbe hele korpusset to gange efter et mislykket regelmatch; en varm 50-sagers Personskat-kørsel faldt fra 21,88 s til 16,99-18,01 s med byteidentisk JSON-resultat)
Current compiler-fingerprint performance slice: `td-941594` (det eksakte SHA-256-fingeraftryk af `runa` genbruges på tværs af CLI-processer bag filidentitet og tidsstempler; en varm Personskat-kørsel faldt fra 7,94 s ved tvungen genberegning til 6,47 s ved cache-hit)
Current batch/corpus performance slice: `td-8f1f24` (uafhængige `@ calculate`-sager køres deterministisk på isolerede workers, og en vedvarende optimeret corpus-profil holder den almindelige compilerudvikling hurtig; den varme komplette Personskat-XLSX-regression faldt fra 189,69 s til 50,54 s)
Planned interpreter dispatch slice: `td-a42ade` (forudberegn aritet, prioritet, head-bindinger og oprydningsplaner pr. regel uden effektrisikoen ved generel resultatmemoisering)
Previous compiler correctness slice: `td-75f6ca` (kompilerede synkrone programmer bruger en afgrænset 64 MiB-workerstack; afventer uafhængig gennemgang)
Previous language support slice: `td-8a34e0` (`refof`-referencer er implementeret, verificeret og godkendt)
Planned calculation-runner performance: `td-783a9c` (genbrug en kompileret eller resident, initialiseret beregningsrunner på tværs af kald)
Deferred performance issues: `td-6659f1`, `td-370d3f`
Deferred metadata cleanup: `td-e4cfd3` (flyt PBL § 15 A's eksisterende præsentationsmetadata til genbrugelige typeankre)
Deferred workbook topology: `td-70d182` (undlad inaktive variantark i den komplette Personskat-arbejdsbog)
Latest approved implementation slice: `td-4e42fd`
Latest approved language slice: `td-8a34e0`

This folder is the working home for encoding Danish personal income tax law in
Futuruna. The aim is not only to display the law as source code, but to make the
rules executable enough to calculate ordinary tax cases and strict enough to
audit tensions, cliffs, missing definitions, and delegated dependencies.

Current project priority: finish the source-backed Personskatteloven
implementation first. Audit files remain important as validation gates for
implemented slices, but deeper exploratory audits should wait until the main law
model is materially complete.

Den kanoniske aktieindkomstgren bevarer nu `AktieindkomstParÅrsskatResultat`
som et signeret mellemresultat efter indkomstmodregning mellem ægtefæller.
Kun en resterende negativ skat går videre til
`AktieindkomstNegativSkatParSag`, som anvender § 8 a, stk. 5-6 i rækkefølgen
egen slutskat, samlevende ægtefælles resterende slutskat og fremførsel. Begge
ægtefællers modregning er begrænset af den disponible skat, og parresultatet
udstiller bevarelsesstørrelser for skat før og efter modregning, anvendt negativ
skat og uudnyttet fremførsel. For hovedpersonen omfatter den disponible slutskat
også ejendomsværdiskat og grundskyld; indkomstskatten dækkes først, og kun den
uudnyttede del går videre til ejendomsskatterne. Hovedpersonens anvendte beløb
føres samtidig som en afledt kredit efter Kildeskattelovens § 60 i
årsopgørelsen. Rå ordinære
ABL-resultater og ABL §§ 37-40-kilder ændres ikke. Rolleombytning, manglende
samliv, ABL § 5 A-tab, ejendomsskat og samtidig fraflytning er verificeret
fortolket og kompileret; de ordinære ABL- og fraflytningskilder krydser desuden
samme JSON/XLSX-beregningsgrænse.

Ægtefællens ejendomsskatter er nu en del af det samme kildedrevne
persondomæne som løn, kapital- og aktieindkomst. Beregningen bevarer både de
indsendte `EjendomsskattelovPersonÅrsfakta` og det afledte årsresultat for
ægtefællen. Den komplette ejendomsværdiskat og grundskyld lægges til
ægtefællens disponible slutskat før modregning efter Personskattelovens § 8
a, stk. 5-6. En rolleombyttet 2025-sag viser samme resultat i begge retninger:
6.012 kr. i ejendomsskatter giver præcis 6.012 kr. yderligere
modregningskapacitet og reducerer fremførslen tilsvarende. De samme typede
ejendomsfelter og danske etiketter genereres for begge personer, og den fulde
JSON-til-XLSX-rundtur bevarer resultatet byteidentisk.

Slutskatteprojektionen efter ABL §§ 37-40 er nu afstemt med denne komplette
slutskat for begge ægtefæller. Før rettelsen gav en rolleombyttet 2026-sag med
560.000 kr. i ordinært ABL § 5 A-tab, 100.000 kr. i fraflyttergevinst og
14.428 kr. i ejendomsskatter henholdsvis 39.346 kr. og 42.000 kr. i
fraflytterskat. Projektionen gav dermed et forskelligt resultat alene efter,
hvem der stod som hovedperson. Begge retninger giver nu 42.000 kr. En særskilt
rolleombyttet sag med 440.000 kr. i dokumenteret fremført negativ
aktieindkomstskat giver 4.452 kr. i begge retninger. Scenarierne kontrollerer
således både ejendomsskatternes og tidligere § 8 a-tranchers plads i den
kanoniske slutskat fortolket og kompileret.

Den uudnyttede negative aktieindkomstskat er nu også et kanonisk årsledger.
Hver åbningstranche bevarer ejer og oprindelsesår og kommer enten fra det
umiddelbart foregående, konsistente Futuruna-årsresultat eller fra en typet
myndighedsfastsættelse med dokumentreference. Reglerne afviser forkert ejer,
fremtidige år, dublerede oprindelsesår, oversprungne Futuruna-årsresultater og
ubalancerede åbninger. Årets § 8 a-skat anvendes først; derefter anvendes
tidligere trancher ældst først i egen og, ved samliv, ægtefællens disponible
slutskat. Primo er altid lig anvendt plus ultimo. Kildeskattelovens § 60-kredit
afledes af den aktuelle anvendelse og kan ikke indsendes som et konkurrerende
beregnet input. Fortolkede og kompilerede årskæder samt en
JSON-til-XLSX-til-`runa call`-rundtur bevarer proveniens og modregner en
dokumenteret 2024-tranche på 1.000 kr. fuldt i 2026.

Futurunas metadataindeks bygger nu på sprogets almindelige typesystem. En kort
`--@label:<label>::meta:<binding>--`-kommentar er kun forbindelsen mellem et
label og en binding; bindingens type skal eksplicit implementere markørtraitet
`Meta`. Metadataens betydning ligger i domænets egne typer. Gentagne roller som
`Source(value = ...)`, `Warning(value = ...)` og `Field(value = ...)` kommer fra
en sumtype, der implementerer `MetaRole`, ikke fra rollenavne kodet ind i
kommentaren eller fra en generisk rolle-/værdibeholder. Den tidligere
`MetaAttachment(role = ..., value = ...)`-form læses kun som
bagudkompatibilitet og bruges ikke i skattekorpusset.

Indekset finder typede efterkommere og roller rekursivt og bevarer både den
kanoniske binding og en stabil sti til værdien. `runa meta --type` og `--role`
kan derfor udføre målrettede audit-sweeps uden at ændre programmets semantik,
mens `runa meta --json` udstiller `futuruna.meta.v1` med råtekstankre,
regelspans, symboler, strukturerede værdier og diagnostikker. Spansymbolerne
kommer fra parserens faktiske deklarationer, så `match`-grene ikke fejlagtigt
optræder som regler, og både `|`-regler og `>`-funktioner indekseres.

Personskat-kontraktens beregningsmetadata ligger nu i et eksplicit typet og
domæneopdelt `PersonskatBeregningsmetadata`-objekt med dele for blandt andet
pension, kapitalindkomst, ejendomsavance og årsopgørelse. Der er ét kort
`::meta:<binding>`-anker ved `beregn_personskat`; det tidligere anker omkring
næsten hele filen er fjernet. Felter og retskilder ligger i de typede objekter.
Kilder på et ydre aggregat nedarves til alle dets feltbeskrivelser, mens kilder i
et indlejret aggregat kun følger felterne i den pågældende typede del. En
rekursiv korpustest afviser den gamle `MetaAttachment`-form, ældre
ankerstavemåder, ekstra referencer og markører over 160 tegn. Rust-kodegeneratoren
normaliserer samtidig generiske typeparametre med fuld Unicode-versalisering, så
typede danske metadatafelter som `årsopgørelse` bruger samme Rust-typeparameter
i deklarationer, felter og markørimplementeringer.

Beregningsmetadata kan nu bruge den strukturelle kompileringstidsværdi
`pathof(PersonskatInput::felt::underfelt)` i stedet for en uigennemsigtig
tekststi. Rodtypen og hvert segment kontrolleres mod lokale og importerede typer
under `runa check`; lister, mængder, valgfrie værdier og map-værdier gennemløbes
uden kunstige indekssegmenter, mens sumtyper kræver en udtrykkelig variant eller
den afsluttende diskriminator `$variant`. Udtrykket sænkes til den samme
kanoniske tekststi som hidtil, så kontrakt-, JSON- og XLSX-formaterne er
bagudkompatible. Personskat-filen bruger formen på både `skatteår` og tre dybe,
flerlinjede EBL §§ 8/9-stier. Den fulde Personskat-kontrol og hele Rust-suiten,
inklusive den kanoniske XLSX-rundtur, passerer med både ældre tekststier og nye
strukturelle stier.

Metadata kan nu pege på regler, funktioner, bindinger, typer, strukturelle
felter og RuleScope-medlemmer med `refof(...)`. Udtrykket giver en almindelig
typet `ProgramReference`-værdi og kontrolleres mod både lokale deklarationer og
almindelige importer. Ukendte, ugyldige og tvetydige mål afvises, mens
fortolkeren, Rust-kompileringen, metadata-JSON og LSP-navigation bevarer samme
reference. Referencen udfører ikke målet og skaber ikke en parallel
afhængighedsgraf; kun faktiske regel- og funktionskald bestemmer afhængigheder.
Virksomhedsskattelovens §§ 8-9 a bruger den generiske `Kildebelæg`-type til at
knytte lovtekst, Nationalbank-data og ministerielle satser til de konkrete
regler, som kilderne underbygger, uden at de underliggende kildetyper bliver
usøgbare i metadataindekset.

Beregningsmetadata kan desuden bevare rodtypen i et strukturelt
`refof(Domænetype::felt)`-mål. Kontraktudtrækket finder hver forekomst af
domænetypen i beregningsinputtet og projicerer den relative feltsti gennem
records, sumtypevarianter og normaliserede samlinger. Valgfrie sammensatte
værdier er fortsat ét kanonisk JSON-felt i kontraktversion 1 og har derfor ikke
selvstændige indre projektionsmål. Metadata kan
derfor forankres ved domænetypens eget label og følge typen gennem forskellige
ydre beregninger. En absolut tekst-/`pathof`-sti eller en `refof`-reference med
selve beregningsinputtet som rod vinder over projiceret metadata; konkurrerende
mål med samme præcedens afvises. Alle 28 CFC-felter er flyttet fra hårdkodede
`cfc...`-stier til referencer mod `PersonskatCfcInput`. Den fulde
Personskat-kontrakt gendanner 28 entydige kanoniske `cfc...`-stier med de samme
spørgsmål, hjælpetekster og kildespor; et enkelt typeforekomstindeks holder det
fulde skemaudtræk på 120,49 sekunder i stedet for at gennemløbe typetræet én
gang pr. felt.

Selskabsskattelovens historiske og gældende § 17-kilder var den første
korpusblok med gentagne `source`-referencer;
Personskattelovens § 3 udstiller nu også en typet `warning` om
ordlydsforskydningen i den dynamiske henvisning til Ligningsloven § 8 O fra
2026. Ligningsloven § 12 B bruger samme generiske mekanisme med rollerne
`source`, `historical_source`, `amendment_source`, `transition_source`,
`guidance` og `warning`. Afskrivningslovens nu opfyldte § 40-henvisning bruger
desuden den vilkårlige rolle `dependency_source` i stedet for at bevare en
forældet advarsel. § 40 C bruger den samme generiske mekanisme til en typet
historisk korpusadvarsel, der kan findes både med `--role warning` og med
`--type AfskrivningslovKorpusAdvarsel`. Dermed kan en audit søge efter enten
kildens rolle eller Futuruna-typen uden særlige metadataregler for juridisk
kode.

Website posture: den offentlige Personskatteloven-side er bevidst én dansk
overbliksside. Den forklarer Futuruna, regelkaskader, det almindelige
lønmodtagereksempel og udvalgte auditsignaler, mens lovtekst, regler, scenarier
og audits bliver i `examples/danish-income-tax/`.

Latest dividend integration: det rå beregningsfelt
`øvrig_aktieindkomst_kroner` er fjernet fra den kanoniske Personskat-kontrakt.
Ordinære udlodninger oplyses nu med entydig identifikation, udlodderens retlige
type, modtagerstatus, aktivtype og kildebeløb. Ligningsloven § 16 A, stk. 2, nr.
1, afleder derefter hjemlen, herunder 2. pkt. om medarbejderejevirksomheder fra
1. januar 2026 efter LOV nr. 1755 af 29/12/2025. Resultatet føres gennem
personskattelovens §§ 4 og 4 a præcis én gang for hver ægtefælle. Forkert år,
forkert modtager, uforenelige aktivforhold, negative beløb og dobbelte
identifikationer afvises før skatteposter dannes. Den genererede arbejdsbog har
et særskilt udbytteark med danske feltetiketter og kildespor. En udfyldt
XLSX-post på 12.000 kr. og den tilsvarende JSON-post giver samme fulde resultat:
12.000 kr. i aktieindkomst og 3.240 kr. i aktieindkomstskat. Der er ingen
automatisk PDF-import i dette forløb; en AI eller person læser dokumentationen og
udfylder arbejdsbogen, hvorefter Futuruna indlæser, typevaliderer og beregner.
Det fokuserede § 16 A-scenarie og alle 35 kanoniske Personskat-invarianter
passerer både fortolket og kompileret. Den fulde kompilerede kørsel afdækkede
først en sprogfejl, hvor gyldige store korpusprogrammer brugte platformens lille
standardstack. `td-75f6ca` retter kodegeneratoren, så synkrone programmer kører
på en navngivet, afgrænset 64 MiB-workerstack og bevarer både fejl- og
paniksemantik. Den tidligere stack overflow er dermed en permanent
sprogregressionstest, ikke et workaround i skatteloven.

Latest establishment-account integration: den kanoniske Personskat-kontrakt
modtager nu observerbare kildefakta efter Etableringskontolovens §§ 1-4 i et
typet `PersonskatPersonligIndkomstInput` for hver person. Det samme beregnede
kilderesultat føres ad to adskilte lovveje: iværksætterkontoens fradrag gennem
Personskattelovens § 3, stk. 2, nr. 11 som fradrag i personlig indkomst og
etableringskontoens fradrag som ligningsmæssigt fradrag. Begge virkninger føres
præcis én gang, mens topresultatet bevarer både kilderesultatet og § 3-broen.
Forkert skatteår og negative pengebeløb giver et synligt ugyldigt resultat og
ingen skattevirkning. En retligt afskåret kontoform er derimod et gyldigt
beregnet nulresultat, så forskellen mellem mangelfulde fakta og lovens afslag
kan auditeres.

De 29 typede beregningsfelter projiceres til både personen og ægtefællen med
danske etiketter, interviewspørgsmål, hjælp, enheder og kildehenvisninger. Ni
fulde Personskat-scenarier dækker neutraltilstanden, begge kontotyper, blandede
indskud, ægtefælleisolering og ugyldige fakta. Den genererede XLSX-arbejdsbog og
den tilsvarende JSON-kontrakt giver samme fulde resultat for et indskud på
30.000 kr. på iværksætterkonto: 522.000 kr. i personlig indkomst efter
AM-bidrag og 455.600 kr. i almindelig skattepligtig indkomst.

Latest ordinary personal-income integration: den kanoniske kontrakts
`PersonskatPersonligIndkomstInput` indeholder nu en typet kildeliste for
arbejdsgiverbetalte ydelser og ordinære virksomheder uden
virksomhedsordningen. Direkte arbejdsgiverbetalt gruppeliv, gruppeliv som
uadskilt del af en PBL § 19-ordning og naturalier efter
arbejdsmarkedsbidragslovens § 2, stk. 2 er adskilte varianter. Reglerne udleder
både PSL § 3-indkomstposten og den præcise AM-behandling. En uklassificeret
årsopgørelseslinje bevares som et synligt ikke-understøttet resultat og får
ingen skattevirkning; den bliver ikke et frit nettobeløb.

Virksomhedsgrenen modtager identificerede bruttoindtægter og udgifter med
`Par3Stk2Nr1Udgiftsafgrænsning`. Kun ordinær driftsindtægt og udgifter til at
erhverve, sikre og vedligeholde selvstændig erhvervsindkomst gennemløber denne
gren. Renter, kursresultater, aktieindkomst og uklassificerede poster afvises
synligt, så de kan føres til deres egne lovregler. `par3_personlig_indkomst_resultat`
udleder nettoresultatet. Et positivt resultat går gennem AM-lovens § 4 og ind i
AM-grundlaget; et underskud forbliver signeret personlig indkomst og kan derfor
ikke reducere AM-bidraget af løn.

De samme afledte værdier bruges i pensionsfradragets indkomstkontekst,
Ligningslovens § 9 C-aftrapningsindkomst, AM-beregningen og den endelige
personlige indkomst. Fokuserede scenarier dækker gyldige, ugyldige og
ikke-understøttede kilder samt den fulde `beregn_personskat`-vej. Et
virksomhedsoverskud på 60.000 kr. oven i 600.000 kr. løn giver 660.000 kr. i
AM-grundlag og 52.800 kr. i AM-bidrag, mens et virksomhedsunderskud på 20.000
kr. lader lønnens AM-bidrag stå på 48.000 kr. og sænker personlig indkomst efter
AM fra 552.000 kr. til 532.000 kr.

Arbejdsbogsforløbet er verificeret fra ende til ende med den fulde kontrakt.
En maskinelt udfyldt 2025-sag med 600.000 kr. i løn, København, ingen
ægtefælle og neutrale valg i de øvrige grene blev hydreret til XLSX og læst
tilbage af `runa call`. Resultatarket indeholder 48.000 kr. i AM-bidrag,
55.600 kr. i beskæftigelsesfradrag, 2.900 kr. i jobfradrag og 211.944 kr. i
samlet skat inklusive AM-bidrag; diagnostikarket indeholder ingen sager. En
helt tom skabelon har med vilje skatteår 0 og er ikke en beregningssag. Et
menneske eller en AI skal først udfylde de krævede fakta. Forældede
kontrakthashes og manglende nye domænefelter afvises med præcise stier.

Det fulde beregningsforløb afdækkede samtidig en generel sprogfejl:
beregningsinitialisering kunne udføre et tidligt fladt importmodul, før en regel
i et senere importmodul havde gjort modulets værdibinding nødvendig. Futuruna
opgør nu den samlede runtime-efterspørgsel på tværs af hele importgrafen, før
flade importer udføres. En sprogregression dækker en senere importeret regel,
der afhænger af både en regel og en værdibinding fra et tidligere modul.

Latest pressure test: en anonymiseret privat årsopgørelse for 2025 er ført
gennem `personskat-2025-aarsopgoerelse.scenario.runa` uden person-, adresse-
eller ejendomsidentifikationer. Scenariet rammer 29.059.034 øre i beregnet skat,
3.716.680 øre i overskydende skat og 8.200 øre i resterende udbetaling præcist.
Boligskatterne er ikke længere lokale, forudberegnede formler i scenariet.
Ejendomsskattelovens regler udleder 80-procentsgrundlagene, 150 af 360 dage,
ejerandelen og kommunens sats fra vurderings- og periodefakta og beregner
315.350 øre i ejendomsværdiskat samt 285.855 øre i grundskyld.
Årsopgørelsen viser ægtefælleoverførslernes virkning, men ikke ægtefællens
underliggende poster. Scenariet markerer derfor sin minimale rekonstruktion som
en antagelse: ingen anden indkomst og 39.617 kr. i renteudgifter. Fra disse
kildefakta udleder reglerne selv 39.617 kr. efter § 13, 18.875 kr. i uudnyttet
personfradragsværdi efter § 10 og 3.169 kr. efter § 11. Den afrundede
lønmodtagerkerne falder fra 312.321 kr. til 280.543 kr.; den særskilte
øreberegning tilføjer derefter aktie- og boligskatter og bevarer den præcise
afstemning. Slutbeløbet genberegnes fra det reducerede skattegrundlag. Det kan
afvige med en krone fra at trække flere allerede afrundede delvirkninger fra
en allerede afrundet baseline, så afstemninger skal fastholde lovens enhed og
afrundingstrin gennem hele beregningskæden.

Latest source-document mapping: `personskat-aarsopgoerelse-kildemapping.runa`
modellerer den grænse, som et AI-interview eller en manuel udfyldelse af
arbejdsbogen skal følge. Hver årsopgørelseslinje bærer stabil dokument- og
linjeidentifikation, indkomstår, den originale etiket og det eksakte beløb i
øre. Direkte beløb peger med `refof(...)` på et eksisterende, kompileringstjekket
kildeinput; flere linjer må kun summeres, når dokument og år er ens, og samme
linjeidentifikation kan ikke tælles to gange. Renter bevarer samtidig et
udtrykkeligt krav om næringsstatus og særregler i stedet for at gøre den
omgivende klassifikation tavs.

Årsopgørelsens beregnede AM-bidrag, beskæftigelsesfradrag, jobfradrag og ekstra
pensionsfradrag er kontrolresultater, ikke input. Pensionsbeløb,
investeringsudlodninger, investeringsgevinster og ægtefælleoverførsler kræver de
underliggende pensions-, depot- eller ægtefællefakta. Det eksisterende
afstemningsscenarie er derfor nu eksplicit mærket som et resultatbaseret orakel;
det er ikke en alternativ beregningskontrakt. Der er fortsat intet PDF-, OCR-
eller dokumentimporttrin: en AI eller et menneske læser kilden og udfylder de
typede fakta, hvorefter Futuruna validerer og beregner.

Trykprøven skelner nu mellem manglende korpus og manglende dokumentation.
Gruppeliv og øvrige arbejdsgiverydelser kræver eIndkomst, lønseddel eller
pensionsoversigt for at fastlægge ydelsens retlige art og AM-status.
Virksomhedens nettotal kræver regnskabets identificerede indtægter og udgifter
samt skatteordningen. Begge kanoniske grene findes nu, så årsopgørelsesmappet
rapporterer `KræverSupplerendeKilde` i stedet for `KorpusgrenMangler`.
Det samme gælder nu fagforening, A-kasse mv. og gaver: årsopgørelsens afrundede
fradragslinjer er kun proveniens og kontrol. Kontingent- eller bidragsopgørelsen
skal levere betaling, retlig art, foreningsformål, medlemskab, skatteposition,
ordningsvilkår, modtagergodkendelse og indberetning til de typede grene.

`td-a47465` gengiver Ligningslovens § 13, §§ 8 A og 8 H samt § 12, stk. 1-4,
og Pensionsbeskatningslovens § 49, stk. 1 og 3, § 49 A, stk. 1, § 49 B,
stk. 1, og § 54, stk. 1-2, umiddelbart over reglerne. Lønmodtagerens
fagforeningsloft er 7.000 kr.;
A-kasse-, efterløns- og fleksydelsesbidrag har intet fælles årsloft. En privat
arbejdsløshedsforsikring kræver mindst 1.300 kr. til et anerkendt
A-kassemedlemskab samt identitet mellem skatteyder, forsikringsejer og
forsikrede. Almindelige gaver begrænses til 19.000 kr. i 2025 og 20.000 kr. i
2026, forskningsgaver efter § 8 H er uden dette loft, og bindende løbende
ydelser efter § 12 kræver en skriftlig, ensidig og ikke umiddelbart ophævelig
aftale med fast beløb eller indkomstandel i mindst ti år eller resten af yderens
liv. § 12-fradraget begrænses efter personlig indkomst med tillæg af positiv
kapitalindkomst; negativ personlig indkomst modregnes før grundlaget beskæres
ved nul. Personskat beregner først AM-bidrag, udenlandske sociale bidrag og
pensionsvirkninger og afleder derefter § 12-grundlaget uden et råt
indkomstinput eller en regelcyklus. De tre komponenter samles præcis én gang i
`PersonskatLigningsfradragResultat`, og både loftsreduktioner og afviste
betalinger bevares i resultaternes `ikke_fradraget_kroner`.

Syvoghalvtreds typede beregningsfelter ved de tre domænetyper giver arbejdsbogen
danske etiketter, interviewspørgsmål, hjælp, enheder og kildespor for både
lister, sumtypevalg og variantdata. En AI eller et menneske kan dermed udfylde
de underliggende fakta; årsopgørelsens nettolinje kan ikke overstyre resultatet.
Metadatareferencer til en sumtypes `$variant` projiceres samtidig til den
kanoniske skalare enumkolonne, når summen kun har varianter uden data. Summer
med variantdata bevarer deres særskilte `$variant`-kolonne. Dermed forbliver
`refof(...)`-referencer typetjekkede, mens arbejdsbogens faktiske kolonneform
bruges konsekvent.

Den fulde Personskat-grænsetest passerer med direkte XLSX-input, direkte
JSON-input og JSON-hydreret XLSX samt eksakt resultatsammenligning på 2.365,91
sekunder. Den fortsatte forbedring af denne interaktive svartid spores i
`td-6659f1`.

Samordningen med sømandsfradraget efter Sømandsbeskatningslovens §§ 3-4 er
implementeret i `td-302f8e`. Typede beskæftigelser afleder § 3- og § 3 a-
betingelserne, det almindelige eller forhøjede fradrag, delårsfordelingen og
fradragsvalget. Den kanoniske beregning afskærer LL §§ 9 B-9 D, LL § 13 og PBL
§ 49, stk. 1, når § 3-fradraget foretages, omklassificerer den berørte
skattefrie LL § 9 B-godtgørelse til skattepligtig indkomst og bevarer PBL §§ 49
A-49 B samt øvrige fradragsgrene. § 4, stk. 2, afskærer LL § 9 A, stk. 1-9,
allerede når personen kan foretage sømandsfradraget, også ved fravalg.

Den almindelige rejsegodtgørelses- og fradragsgren efter LL § 9 A, stk. 1-9,
er nu implementeret i `td-d17087`. Korpusset bevarer den fulde gældende ordlyd
af § 9 A, stk. 1-13, samt § 9, stk. 2 og 4, og bruger de officielle satser for
2025 og 2026. Et `Ligningslov9AÅrsinput` samler personrollen, identificerede
rejser, identificerede udenlandske lønindkomstkilder, en dateret
arbejdshistorik og et eventuelt særskilt opgjort fradrag for dobbelt
husførelse. Hver rejse bærer startdato, arbejdsstedets faktiske
karakter, overnatningsforhold, hverv, varighed, måltider, arbejdsgiverkontrol,
lønomlægning, udbetalt godtgørelse, egne udgifter, fradragsvalg og
indkomstforhold. En udenlandsrejse henviser til sin årlige indkomstkilde frem
for selv at gentage lønnen og de øvrige fradrag. Reglerne udleder
rejsebetingelserne, 12-månedersperioden,
standardsatserne,
måltidsreduktionen, 25 pct.-satsen, skattefri og skattepligtig godtgørelse,
faktisk eller standardiseret fradrag, udlandsindkomstloftet og det fælles
årsloft. Det resterende udlandsindkomstloft bruges deterministisk i
rejserækkefølgen og reduceres af tidligere rejser med samme kilde; forskellige
kilder holdes adskilt. Ugyldige fakta eller kildehenvisninger fejler lukket,
mens en udbetalt godtgørelse bevares som
skattepligtig.

Den kanoniske `beregn_personskat` bevarer både resultatet før og efter
Sømandsbeskatningslovens § 4, stk. 2. Udelukkelsen afledes af
`kan_foretage_fradrag_efter_par3`, ikke af valget om faktisk at foretage
sømandsfradraget. En afskåret skattefri rejsegodtgørelse føres derfor til både
løn- og AM-grundlaget samt § 9 C's aftrapningsindkomst, mens det anvendte
rejsefradrag kun føres til de ligningsmæssige fradrag. Et kanonisk scenarie med
800 kr. i godtgørelse og 986 kr. i muligt fradrag bevarer begge beløb for en
almindelig lønmodtager; en sømandsberettiget person, der fravælger sit
sømandsfradrag, får 0 kr. skattefrit, 800 kr. skattepligtigt og 0 kr. i
rejsefradrag.

Kost og logi er nu adskilt strukturelt i `td-45f9fb`. Én typet kostpost bærer
hele rejsens dæknings-, godtgørelses- og fradragsprincip, mens nummererede
logidøgn hver bærer arbejdsgiverens faktiske dækning, godtgørelsesafregning,
dokumenterede egenudgift og valg mellem standardsats, dokumenterede udgifter
eller intet fradrag. Fri logi eller dækning efter regning afskærer derfor kun
det berørte døgn. Godtgørelse over satsen kan enten stå uopdelt og blive fuldt
skattepligtig eller være endeligt opdelt af arbejdsgiveren i en skattefri
satsdel og supplerende løn. Reglerne udleder virkningen uden
fradragsberettigelsesflag fra kalderen og er verificeret i både interpreter,
compiler og den relationelle XLSX-grænse.

Den særskilte ø-logigren i § 9 A, stk. 12, er implementeret i `td-e53d8f`.
Typede fakta beskriver bopælskommune, ø, fast vejforbindelse, arbejdssted,
afstand og transporttid, hverv samt identificerede logiophold med dato,
varighed og egenudgift. Reglerne genkender både kommunerne i § 9 C, stk. 3, og
de ti særskilt nævnte småøer, kræver en ikkebrofast ø, et fast arbejdssted og
umulig hjemmeovernatning og genbruger stk. 11's person- og erhvervsudelukkelser.
Kun fulde døgn med egen logiudgift giver standardsats. Ugyldige, dublerede
eller overlappende ophold fejler lukket.

Ø-logifradraget tildeles efter dobbelt husførelse og almindeligt rejsefradrag
inden for det samme regulerede årsloft. Det kanoniske Personskat-resultat bruger
nu det samlede § 9 A-fradrag både før og efter Sømandsbeskatningslovens § 4,
stk. 2. Skatteforvaltningens Samsø-eksempel med fire fulde døgn giver 1.072 kr.
i 2026; en sømandsberettiget person får beløbet synligt før udelukkelsen og 0
kr. efter. Begge forløb passerer i interpreter og compiler.

Det fælles, identificerede indkomstgrundlag for flere rejser knyttet til samme
udenlandske løn efter § 9, stk. 2, er implementeret i `td-c61f53`; hver kilde
har én løn, ét beløb for øvrige relaterede fradrag og én samlet rest, som
rejserne deler.
Det særskilt opgjorte fradrag for dobbelt husførelse
reducerer allerede det fælles loft, men selve fradragsretten og det kanonisk
anvendte beløb spores i `td-18836f`. Udelukkelserne for DIS og
fiskerfradrag følges fortsat i henholdsvis `td-80c439` og `td-5759f5`.
Afledning af undtagelsen for enkeltstående kortvarige tjenesterejser fra
observerbare rejsefakta frem for en klassifikationsvariant spores i
`td-bc07c9`.
Den tidsmæssige 12-månedersperiode er nu kildeafledt i `td-3515b8`. Et årligt
forløb sorterer aktuelle og tidligere perioderejser efter startdato, bærer den
aktive periodestart mellem gentagne rejser og udleder en ny periode ved mindst
8 km til et nyt arbejdssted eller ved tilbagevenden efter mindst 40 daterede
arbejdsdage på et reelt andet arbejdssted. 39 dage fortsætter den gamle
periode, arbejde på privat bopæl tæller ikke, og en manglende eller tvetydig
effektiv afstand fejler lukket. Resultatet bevarer overgangsgrund,
periodestart, udløbsdato, fulde måneder og aktiv status som auditdata; ingen
rejserække kan længere erklære sin egen førstegangs- eller resetstatus.

Den fokuserede grænse
`@ calculate("Rejse- og logiopgørelse efter ligningslovens § 9 A")` genererer en
relationel XLSX-arbejdsbog med danske etiketter, interviewspørgsmål, dropdowns,
enheder og typede kildespor. Ud over logidøgnstabellen genereres særskilte
relationelle tabeller for tidligere perioderejser, daterede arbejdsdage og
effektivt daterede afstande mellem arbejdssteder. Udenlandske
lønindkomstkilder har deres egen relationelle tabel, og rejserne henviser til
den stabile kildeidentifikation. Ø-logi har tilsvarende en arbejdsstedstabel og
en underliggende opholdstabel, mens bopæl, ø og vejforbindelse er typede
variantfelter. Arbejdsbogen spørger dermed kun efter observerbare fakta og ikke
efter juridiske periode- eller fradragskonklusioner. En
JSON-hydreret rejserække er læst tilbage fra XLSX og beregnet til 800 kr.
skattefri godtgørelse, 0 kr. skattepligtig godtgørelse og 986 kr.
rejsefradrag uden diagnostikker; en fokuseret arbejdsbogstest bevarer desuden
den lønopdelte kostgodtgørelse og de forskellige logidøgnsresultater. En anden
fokuseret grænsetest hydrerer tre udenlandsrejser og to indkomstkilder fra JSON
til XLSX og tilbage: to rejser deler 900 kr. som 893 kr. og 7 kr., mens den
tredje beholder sit særskilte loft på 500 kr.; XLSX- og JSON-resultaterne er
identiske. En tredje fokuseret grænsetest hydrerer Samsø-fakta og et fire døgns
logiophold til de relationelle XLSX-tabeller, læser dem tilbage og får samme
1.072 kr. som den direkte JSON-beregning.

Den faktiske periodefordeling er implementeret i `td-2a827d`. Hver berørt
LL § 9 B-sag og hvert PBL § 49, stk. 1-bidrag har en strukturel kildeidentitet;
LL §§ 9 C-9 D og det loftsberegnede LL § 13-årsfradrag har hver sin entydige
kildegren. Kilden knyttes til en navngiven kvalificerende sømandsbeskæftigelse,
et andet navngivet arbejdsforhold eller, kun hvor kilden reelt er et årsbeløb,
en udtrykkelig årsfordeling. LL § 9 B kan ikke bruge årsfordelingen, fordi hver
kørselssag skal henføres til sit faktiske arbejde. Ukendte, dublerede eller
manglende tilknytninger gør beregningen ugyldig og får fradragsaggregatet til at
fejle lukket i stedet for at gætte. Et kompileret blandet-år-scenarie bevarer
7.880 kr. i skattefri kørselsgodtgørelse fra andet arbejde, omklassificerer
3.940 kr. fra sømandsperioden og bevarer LL §§ 9 C-9 D, LL § 13 og PBL § 49,
stk. 1, præcis én gang uden endnu en sødagsfordeling.
Den særskilte DIS-lempelse af selve søindkomsten efter Sømandsbeskatningslovens
§§ 5-8 er ikke en del af §§ 3-4-samordningen; dens kanoniske lønvirkning spores
i `td-80c439`.
Den endnu ikke implementerede kildefaktagren for øvrige lønmodtagerudgifter
efter LL § 9, stk. 1, spores i `td-80292f`; § 4-reglen er klar til at samordne
grenen, når dens eget lovkorpus kobles på.
Den tilsvarende, beskæftigelsesknyttede samordning med fiskerfradraget efter LL
§ 9 G er afgrænset i `td-5759f5`. Ingen af delene indføres som et løst flag i de
almindelige kontingent- eller bidragsdomæner.

Den modregningsberettigede udbytteskat på 4.353,14 kr. føres nu tabsfrit fra
den typede dokumentlinje til `KildeskatPar60KreditterØre` og videre gennem den
kanoniske Personskat-årsopgørelse. Den anonymiserede 2025-sag modregner
32.775.714 øre i foreløbige skatter, fratrækker 3.708.400 øre, der allerede er
tilbagebetalt efter § 55, og afstemmer 8.280 øre i overskydende skat. Først ved
Kildeskattelovens § 62, stk. 4, bliver udbetalingen 82 hele kroner, mens de 80
øre forbliver synlige i resultatet. § 62 C, stk. 4, er modelleret tilsvarende
for opkrævning. Eksisterende hele-krone-kald går gennem en tabsfri adapter; den
bredere revision af Personskats øvrige pengeenheder og afrundingstrin forbliver
afgrænset i `td-24963d`.

Fidelitetskontrollen afdækkede samtidig en generel Futuruna-fejl: en
topniveauværdi, som gennem en nulargumentregel afhang af en senere værdi, blev
tidligere initialiseret som `()`. Interpreter og compiler bruger nu samme
afhængighedsorden, og cykliske værdiinitialiseringer afvises eksplicit.

Latest property-tax integration: Ejendomsskattelovens §§ 23-27 og §§ 35-45
er nu forbundet med den kanoniske Personskat-beregning. Reglerne afleder
pensionistnedslag, tidligere-ejer-nedslag, skatterabat og
stigningsbegrænsning fra vurderinger, ejerandele, hændelser og historiske
kildefakta. De offentlige resultater skelner mellem ugyldige, ikke relevante
og anvendte grene, og hvert nedslag indgår præcis én gang i ejendommens
slutskat. De fokuserede scenarier gengiver blandt andet Skattestyrelsens
2024-eksempel med 5.090 kr. efter gamle regler, 6.180 kr. efter nye regler og
1.090 kr. i rabat. De fastholder også, at pensionistnedslag holdes uden for
forskelsbeløbet, samt grænserne på 4,75 pct. og 3,50 pct. for
stigningsbegrænsningen.

Kildegennemgangen mod Skatteministeriets aktuelle satsoversigt og Den juridiske
vejledning har samtidig udvidet den ordinære grænse til 2026: § 22 anvender
9.007.000 kr., mens § 26 anvender 239.800 kr. for enlige og 368.800 kr. for
ægtepar. Ægtefællers kapital- og aktieindkomst nettosummeres nu før det positive
beløb udvælges. Historisk afgiftsberigtiget aktieløn afledes fra tildelings- og
realisationsdatoer med entydige hændelses-id'er, og en længstlevendes succession
bærer den tidligere ægtefælles aldersgrundlag i selve hændelsen i stedet for at
genbruge feltet for en nuværende samlevende ægtefælle.

Latest municipal-parameter integration: `Kommune` dækker nu alle 98 danske
kommuner med stabile Futuruna-varianter. De officielle Skatteministeriet-filer
for 2024, 2025 og 2026 er bevaret som fulde tabeller umiddelbart over de
tilsvarende typede parameterlister. Hver af de 294 kommune-årsposter bærer
kommunekode, kommuneskat, kirkeskat, skatteloftsnedslag og ordinær
grundskyldspromille i eksakte heltalsenheder. Kildemetadataen bevarer fil-URL,
format, hentet dato og SHA-256 for hvert års regneark. En særskilt audit beviser
98 forskellige kommuner, 98 forskellige kommunekoder og præcis én post pr.
kommune for hvert understøttet år samt stabile kommunekoder på tværs af årene.
Bopælskommunen og hver ejendoms kommune forbliver forskellige input. Den fulde
XLSX-rundtur beviser, at begge felter udstiller den samme komplette dropdown med
98 valg uden at ændre det kanoniske skatteresultat.

Den genererede arbejdsbog udstiller overgangsreglernes kildefakta med danske
etiketter, spørgsmål, hjælp og kilder. Der findes ikke et automatisk PDF- eller
dokumentimporttrin. En AI eller et menneske læser dokumenterne eller gennemfører
interviewet og udfylder arbejdsbogen; Futuruna indlæser og typevaliderer derefter
fakta, udfører reglerne deterministisk og eksporterer resultatet. Den komplette
tomme Personskat-arbejdsbog er gyldig, men har aktuelt 303 ark, fordi også
inaktive sumtypegrenes relationelle strukturer materialiseres. Den særskilte
UX-opfølgning `td-70d182` skal reducere denne topologi uden at svække den typede
kontrakt eller rundturen.

Latest debt integration: Kursgevinstlovens §§ 19-24 A modtager nu identificerede
gældsposter som observerbare kildefakta. Arbejdsbogen beskriver blandt andet
værdier, frigørelsesart, valuta, finansieringsnæring, selskabsforbindelse og
særlige § 22-hændelser med danske etiketter og interviewspørgsmål. Ved en
frivillig kreditorordning oplyser AI'en eller mennesket ordningens identitet, om
hele den usikrede kreditormasse er oplyst, samt hvert kravs beløb, tilstrækkelige
sikkerhed, deltagelse, restkrav og eventuelle dokumenterede småkravsgrundlag.
Det oplyser ikke, om ordningen juridisk er "samlet".

Personskat leverer indkomståret, mens Futurunas `|`-regler afleder den usikrede
gæld, deltagelsesandelen, § 24-vurderingen, prioriteten mellem §§ 19, 21, 22,
23, 24 og 24 A, den samlede 2.000-kronersgrænse for § 23 og kapital- eller
personlig-indkomstvirkningen. Ufuldstændige kreditormasser, præcis 50 pct. og
udokumenterede udeladte krav fejler lukket; mere end 50 pct. med dokumenterede
små udeladte krav kan kvalificere, mens et dokumenteret væsentligt udeladt krav
afviser den samlede ordning. Seks kanoniske Personskat-scenarier og femogfyrre
fokuserede gældsscenarier passerer både fortolket og kompileret.

XLSX/JSON-rundturen dækker både en almindelig valutagæld og en frivillig ordning
med fem relaterede kreditorrækker. Arbejdsbogen og de samme direkte typede
JSON-fakta giver identiske komplette Personskat-resultater. I den frivillige sag
afleder reglerne en deltagelsesandel på 82,08 pct., anvender § 24 og medregner
50.000 kr. efter fordringens værdi for kreditor. Kildesporet peger på gældende
lovtekst og Den juridiske vejledning for §§ 19, 21, 22, 23 og 24.

Realitetstesten fastlægger fem generelle revisionsprincipper for korpusset.
Offentlige beregningsgrænser skal modtage observerbare kildefakta, ikke
afledte retskonklusioner, skattegrundlag eller skattebeløb. Ugyldige input,
gyldige forhold uden for en regels anvendelsesområde og endnu ikke
understøttede regelgrene skal kunne skelnes i resultaterne. Hver skatteart skal
have ét kanonisk kompositionspunkt med en bevarelsesinvariant mod dobbelt eller
manglende medregning. Eksakte enheder og lovbestemte afrundingstrin skal
bevares, indtil reglerne selv kræver en konvertering. Endelig kan datoens
civile struktur genbruges, mens 30/360, faktiske kalenderdage og andre
lovbestemte dagtællinger skal forblive særskilte politikker. Kildesporet er
også tidsligt: selv en materielt uændret formel skal have en ændringskilde, når
dens placering eller styknummer ændres. De målrettede opfølgninger ejes af
`td-ba70c7`, `td-24963d`, `td-bf5e81`, `td-01c72e` og `td-091de2`.

Realitetstesten har nu fundet fem materielle integrationsfejl: Frederiksberg og
kommunens 2025-sats manglede, § 9 L-fradraget blev afkortet i stedet for
afrundet, den endelige lave aktieindkomstskat var ikke medregnet i slutskatten,
virksomhedsordningen og kapitalafkastordningen kunne vælges samtidig, og
indgående ægtefælleoverførsler fandtes kun som isolerede regler. Den første
samlede ægtefællekørsel fandt desuden en sjette invariantfejl: negativ
skattepligtig indkomst skabte negativ kommune- og kirkeskat og oppustede det
overførte personfradrag. Begge skattegrundlag afgrænses nu ved nul med særskilte
regressioner.

Den kanoniske `PersonskatInput` modtager nu enten ingen ægtefælle eller en hel
typet ægtefælleprofil og samlivsstatus. § 13-, § 10- og § 11-resultater kan ikke
længere leveres som rå reduktionsbeløb. JSON- og XLSX-kontrakten udstiller de
samme kildefakta med danske spørgsmål og med både LBK 1284/2021 og LOV
1564/2023 i kildesporet. Den ordinære boligskat indgår nu én gang i det
kanoniske skattetotal og i årsopgørelsen. Den generelle boligskattegrænse har
nu også kildefaktabåret dækning af nedslag, rabat og overgangsregler; alle
kommuners årsparametre for 2024-2026 er nu dækket. De resterende investerings-,
ligningsfradrags-, virksomhedsunderskuds- og personlige indkomstposter er heller
ikke fuldt dækket. Den generelle opfølgning
`td-ba70c7` skal derfor kontrollere alle offentlige beregningsgrænser for
afledte input, utilgængelige regelgrene og manglende flerpersons- eller
tværårsscenarier.

`runa template` kan nu tage en valideret JSON- eller TOML-konvolut som
`--input` og hydrere den genererede XLSX-kontrakt. Det bevarer menneskelige
etiketter, dropdowns, relationelle ark og skjult skema- og stimetadata og gør den
AI-interviewede JSON til en kontrollerbar arbejdsbog uden en separat
regnearksmodel.

Latest integration: Virksomhedsskattelovens § 8 afleder nu det signerede
kapitalafkastgrundlag fra typede aktiver, gældsposter, konti, hensættelser,
privatoverførsler og boliger stillet til rådighed for den lovbestemte
nærtståendekreds. Fast ejendom og § 3, stk. 4's øvrige aktivarter beholder hver
sin værdiansættelse, mens dubletidentifikationer og ugyldige historiske
vurderingsvalg fejler lukket. §§ 9 og 9 a afleder 2025- og 2026-satserne fra de
seks lovbestemte månedsobservationer i Danmarks Nationalbanks DNRUUPI-tabel,
inklusive afrunding til to decimaler, simpelt gennemsnit med én decimal,
2-procentpointnedsættelse og henholdsvis ned- og oprunding. § 7's
kildegrænse modtager derfor ikke længere et færdigberegnet afkastgrundlag eller
en kapitalafkastsats fra kalderen. Den fokuserede scenariofil passerer både
fortolket og kompileret og sammenholder 2025-resultatet 2/5 pct. med
Skatteministeriets offentliggjorte satser; 2026-resultatet 2/5 pct. bevares med
status som endnu ikke offentliggjort på satsoversigten pr. 30. juli 2026.

Latest integration: Ejendomsavancebeskatningslovens § 10 bærer nu hver
genopført ejendom som et typet objekt med stabil identifikation og faktiske
genopførelsesudgifter. Ved genopførelse på en eller flere andre ejendomme
afledes stk. 5-grunden og den krævede meddelelse særskilt. Stk. 7 og 8 fordeler
den overførte anskaffelsessum og overskydende genopførelsesudgifter efter hver
ejendoms eksakte andel af de samlede udgifter. En deterministisk
restfordeling afstemmer hele kronebeløb til lovens totaler uden at skjule den
eksakte tæller og nævner. Den valgte ejendoms andel af den genanbragte
fortjeneste går gennem det fælles genanbringelsesresultat og videre til en
senere ordinær, § 8- eller § 9-afståelse. Dubletidentifikationer, tomme lister,
ikke-positive udgifter og en manglende valgt ejendom fejler lukket. En for sen
meddelelse er gyldigt oplyst, men opfylder ikke betingelserne og skaber ingen
fordeling. De to løse ja/nej-felter for stk. 10 er erstattet af et typet
afskrivningsforhold. Den ergonomiske variant uden afskrivningsberettigede
bygninger kræver ingen yderligere data. Varianten med afskrivningsberettigede
bygninger bærer derimod kildeinputtet til Afskrivningslovens § 24 og en
eksplicit forbindelse fra hvert identificeret afskrivningsaktiv til den
genopførte ejendom. EBL-reglerne beregner selv § 24-resultatet og afstemmer
skadeår, endelig erstatningsfastsættelse, genopførelsesår, placeringshjemmel,
at AL-erstatningen ikke overstiger EBL-beløbet, entydige
aktividentifikationer og de afskrivningsrelevante
genopførelsesudgifter pr. ejendom. Manglende eller dobbelte forbindelser,
ukendte ejendomme, dobbelte aktividentifikationer og modstridende beløb eller
kildefakta fejler lukket og bliver synlige som
`afskrivningsforhold_afstemt = Falskt`. Afskrivningslovens gældende LBK nr.
1222 af 13. oktober 2025 er samtidig knyttet til den ordrette § 10-blok som en
typet `dependency_source`.

De sidste to kalderstyrede anvendelsesfelter er nu fjernet. Det selvstændige
§ 21-spor beregner først den ordinære fortjeneste eller det ordinære tab uden
at kende § 24-konklusionen. § 24 beregner derefter et typet kandidatresultat
for betingelserne før stk. 10. En fælles EBL § 10-samordning afstemmer de to
loves skadeår, erstatningsfastsættelse, genopførelsesår, placering, beløb,
aktiver og ejendomme og afleder først derefter den samtidige anvendelse. Dens
forklaringsspor bevarer § 21 før og efter samordning, § 24-kandidaten, det
endelige § 24-resultat og EBL § 10-resultatet. Et gyldigt forløb kan derfor
vise, at 50.000 kr. først beregnes som genvundne afskrivninger og derefter
udskydes; modstridende kildefakta bevarer i stedet den ordinære
§ 21-beskatning. § 24, stk. 11's fristsvigt og femprocents tillæg afledes
uafhængigt af en EBL-til/fra-kontakt. Sytten EBL § 10-invarianter, ti
AL §§ 21-24/Personskatteloven-invarianter og de berørte overgangsscenarier
passerer i både interpreter og compiler.

Ejendomsavancebeskatningslovens § 10, stk. 9 er nu også et beregnet
beløbsspor frem for en ikke-understøttet stilling. Hver identificeret
genopførelsesejendom bærer erhvervsmæssige udgifter og et typet valg mellem
ingen ejerboliganvendelse ved genopførelsen og erhvervserstatning anvendt til
ejerbolig ved genopførelsen. I det sidste tilfælde oplyses ejerboligens
genopførelsesudgifter, den anvendte erstatning og den historiske
anskaffelsessum for den skaderamte del. Reglerne validerer både hver ejendom og
de samlede fordelinger, trækker ejerboligdelen ud af den ordinære
avanceopgørelse og medregner samtidig hele den anvendte erstatningssum efter
stk. 9 uden fradrag for den historiske anskaffelsessum. Medregningsåret afledes
af genopførelsesåret. Det ordinære § 10-resultat og samordningen med
Afskrivningslovens § 24 bevares som særskilte forklaringsspor.

En senere ændring til § 8-boligejendom er derimod et særskilt dateret faktum
ved den senere afståelse. For § 9 bærer ændringen både datoen og boligandelen i
promille før og efter ændringen. Reglerne validerer kronologien mod
genanbringelsesåret og afståelsesdatoen, afleder selv om boligandelen er
forøget og anvender overgangsreglen for ændringer før den 15. december 2005.
Dermed beskattes den gamle genanbragte erhvervsfortjeneste efter § 8, stk. 5,
eller § 9, stk. 4, uden at en senere boligbrug fejlagtigt behandles som
§ 10, stk. 9 ved genopførelsen.

Den kanoniske `beregn_personskat`-graf modtager egne og en samlevende
ægtefælles identificerede skadegenopførelser. Kun den ordinære
ejendomsfortjeneste går gennem Ejendomsavancebeskatningslovens § 6-tabsspor;
personens bruttobeløb efter § 10, stk. 9 lægges derefter særskilt til
kapitalindkomsten efter Personskattelovens § 4, stk. 1, nr. 14. Et blandet
scenarie med 50.000 kr. i ordinær straksbeskatning, 100.000 kr. i fremført tab
og 200.000 kr. i ejerboligerstatning modregner kun de 50.000 kr. og ender med
200.000 kr. i kapitalindkomst samt 50.000 kr. i fortsat fremført tab.
Nul-, delvis- og fuld ejerboliganvendelse ved genopførelsen, ugyldige
fordelinger, senere klassifikationsskifte, AL-samordning og den kanoniske
kapitalindkomst passerer i de fokuserede interpreter- og compiler-scenarier.
Senere boligbrug følger nu det særskilte klassifikationsskifteregelsæt i
§§ 8, stk. 5, og 9, stk. 4 i stedet for at fejle lukket i § 10.

Den kanoniske `beregn_personskat`-graf modtager nu nul
eller flere identificerede pensions- og forsikringsordninger efter
Pensionsbeskatningslovens § 53 A som kildefakta. Skatteåret afledes fra den
omgivende Personskat-sag. Ordningstypen, undtagelsen efter stk. 4 og
udelukkelsen efter § 53 B er ikke længere juridiske konklusioner valgt af
kalderen. Hver ordning oplyser i stedet det faktiske produkt, ejer eller
berettigede, kapitel 1-status, oprettelsesdato, eventuelt afkald på afsnit I,
institutionsfinansiering og personens skattepligt eller skattemæssige hjemsted
ved oprettelsen. Indbetalingerne oplyser den faktiske udenlandske fradrags- eller
bortseelsesbehandling. Reglerne afleder hver af stk. 1, nr. 1-9, § 53 B,
stk. 1-3, risikoundtagelserne i stk. 4 og statsstøttebegrænsningen i stk. 6 i
et typet forklaringsresultat. Modstridende eller ufuldstændige omfangsfakta
fejler lukket.

Hver ordning har desuden en stabil ordnings- og skatteyderidentifikation samt
en sammenhængende, kronologisk liste af årlige afkastfakta. Et år oplyser enten
pensionsudbyderens PAL-afkast eller kalenderårets faktiske primo- og
ultimodepotværdi, begyndelsesstatus for skattepligt og eventuel
direktørsikkerhed, daterede ændringer med depotværdi og de faktiske berettigede
ved den relevante afkastperiodes udgang. Kalderen leverer ikke længere metodehistorik, negativ
fremførsel, en beregnet afkastandel, summerede indbetalinger eller udbetalinger
eller den skattepligtige periodes grænser.

Et flerårigt fold afleder den bindende metode, negativt afkast for samme
ordning, den sammenhængende skatteperiode og periodens faktiske betalinger. Et
helt år uden skattepligt medregner intet PAL-afkast og etablerer ikke et nyt
metodevalg. Udeladte mellemår, metodeændringer, dubletter og flere adskilte
skatteperioder i samme år forbliver synlige og fejler lukket. Ved ind- eller
udtræden bruger kapitalværdimetoden depotet på ændringsdatoen. For en
direktørordning gælder det tilsvarende, når sikkerhedsstillelsen etableres eller
ophører. Flere berettigedes afkastandel beregnes af deres eksakte, validerede
indeståender ved afkastperiodens udgang. Hver ordning bevarer årets afledte PBL-input,
PBL-resultat og resultatet efter Personskattelovens § 4, stk. 1, nr. 13.
Positivt afkast efter egen negativ fremførsel bliver kapitalindkomst, mens et
negativt afkast forbliver knyttet til den samme identificerede ordning.

Afkastforløbet har nu en eksplicit, typet åbning: enten erklæres det første
angivne år som ordningens første afkastår, eller også leveres seneste
dokumenterede år, den allerede bindende moderne metode og daterede, uudnyttede
negative afkast. Til og med 2009 kræver reglerne den historiske
kapitalværdimetode uden at etablere et moderne metodevalg. Fra 2010 anvendes
det moderne metodevalg for ordninger oprettet den 18. februar 1992 eller
senere. Negative afkast fra 2001 og tidligere udløber efter oprindelsesåret og
de fem følgende indkomstår, mens saldi fra 2002 og senere bevarer den
ubegrænsede fremførsel efter samme ordning. Oprindelsesåret bevares gennem
hele foldet, så udløb og modregning kan forklares særskilt. Ordninger bevarer nu
en kronologisk kæde af oprettelse og senere erhvervelser med entydig
identifikation, præcist tidspunkt, overdrager, erhverver og kapitalværdi. Hver
gyldig erhvervelse lukker den tidligere rettighedshavers halvåbne periode og
åbner den næste på samme tidspunkt og værdi. En almindelig erhvervelse den 18.
februar 1992 eller senere aktiverer de moderne regler fra erhvervelsen. Direkte
arv af en ældre livsforsikring bevarer derimod den historiske behandling efter
den tidligere § 50 eller statsskatteloven, mens en senere arv ikke ophæver
virkningen af en allerede sket almindelig erhvervelse. Ugyldige datoer,
dubletidentifikationer, omvendt kronologi, brudte rettighedskæder og
erhvervelser før oprettelsen fejler lukket. Den enkelte skatteyders afkastfold
afleder rettighedsgrænserne fra kæden uden at forveksle dem med ind- eller
udtræden af dansk skattepligt. De afledte rettighedsgrænser findes kun i det
interne beregningsspor og kan ikke vælges som input i regnearket. Kapitalværdien
ved erhvervelse erstatter årets primoværdi, og betalinger før erhvervelsen
medregnes ikke i den nye rettighedshavers afkast. Ved afståelse og senere
generhvervelse vælger reglerne nu kun de rettighedsperioder, der skærer det
aktuelle indkomstår. En tidligere og en senere ejerperiode kan derfor beregnes
hver for sig, mens et helt mellemliggende år uden rettighed giver et
udtrykkeligt tomt periodevalg. Flere adskilte perioder for samme skatteyder i
samme indkomstår giver et typet flertydigt valg og fejler lukket, indtil en
flerperiodeberegning er modelleret. Bindende tilvalg af de moderne regler og
materielle kontraktændringer, der kan udgøre en ny ordning, er registreret
særskilt som `td-8ddbca` og `td-f2ca68`.

Erhvervelsesårets feltprojektion afdækkede samtidig en generel
kodegeneratorfejl: Når en topniveau-binding blev dannet af en værdiregel, kunne
en senere regel miste bindingens kendte recordtype ved direkte feltprojektion.
Kodegeneratoren løser nu værdireglers returtyper og topniveau-bindingers typer
til et fælles fikspunkt før Rust-emission. En fokuseret kompileret regression
fastholder, at projektionen bruger den kendte recordtype frem for at forsøge
samme feltnavn på uvedkommende sumtypevarianter.

Stk. 2 og 5 bruger nu desuden et ordnet hændelsesforløb pr. ordning.
Arbejdsgiverens og den tidligere arbejdsgivers bidrag bliver personlig indkomst
med AM-grundlag; bidrag fra en udsendt ægtefælles eller samlevers arbejdsgiver
bliver personlig indkomst uden AM-grundlag, når varighed, sted og fælles bopæl
er opfyldt, og undtagelsen for samtidige indbetalinger til afsnit I eller II A
anvendes direkte. Sikkerhedsstillelse efter stk. 1, nr. 6 afleder både
arbejdsgiverfradraget og den del af arbejdsgiverens senere pensionstilsagn, som
ikke skal medregnes.

Historiske indbetalinger bevarer deres danske eller udenlandske fradragsstatus.
Ved en udbetaling fordeles den resterende fradragsbegunstigede principal efter
det officielle krone-for-krone-princip som udbetalingen ganget med principalens
andel af den aktuelle kapitalværdi. Værditilvækst medregnes ikke igen. Reglerne
afleder modtagerkredsen efter § 55, 75-procentreglen for kvalificerende samlede
kapitaludbetalinger og § 20, stk. 6-resultatet med mulig lempelse efter
Ligningslovens § 33. En udbetaling til dækning af afkastskat er en dateret og
dokumenteret hændelse og er kun undtaget, hvis beløbet ikke overstiger den
dokumenterede skat og udbetales senest året efter optjeningsåret. Beløbene
føres én gang til Personskattelovens § 3 og den kanoniske AM-beregning. De to
fokuserede scenariefiler gennemfører henholdsvis 8 og 11 invariantscenarier i
både interpreter og compiler; den almindelige mapper-scenarie gennemfører
yderligere 11 invariantscenarier i begge udførelsesveje. Et særskilt
omfangsscenarie gennemfører 18 invariantscenarier for de ni hjemler, § 53 B,
stk. 4, stk. 6 og fail-closed-validering i begge udførelsesveje. Et nyt
overgangsscenarie gennemfører desuden 9 invariantscenarier for 2001-, 2002-,
2009- og 2010-grænserne samt dokumenteret åbningshistorik i både interpreter og
compiler. Et særskilt erhvervelsesovergangsscenarie gennemfører 11 yderligere
invarianter for skæringsdatoen, overdragelse uden tilbagevirkende kraft,
direkte arv, overdragelse efterfulgt af arv, produktgrænsen for
arveundtagelsen og fail-closed kronologi i begge udførelsesveje.

Historiske ordninger kan nu desuden bære daterede meddelelser om det bindende
valg af § 53 A eller § 53 B efter LOV nr. 1388 af 20. december 2004. Reglerne
afleder ikrafttrædelsen den 22. december 2004 fra bekendtgørelsen dagen før,
den ordinære frist før 2006, særvirkningen fra 1. januar 2004,
fristvejen ved senere fuld skattepligt og Skatterådets første-valgmulighed ved
senere arv. De kontrollerer modtager, beslutnings- og modtagelsesdato, om det
valgte regelsæt i øvrigt er anvendeligt og om tidligere ejere af en arvet
livsforsikring allerede havde dansk skattepligt eller valgt afsnit II A. Det
første gyldige valg forbliver bindende; et senere modstridende forsøg kan ses i
resultatet, men ændrer ikke regimet.

Den tidligere blanket 49.020 uden afkrydsningsfelt er modelleret som en særskilt
indsendelsestype med blanketudgave, indsendelses- og modtagelsesdato, modtager,
skatteyderens dokumenterede påberåbelse og ønsket virkning. Den har intet
målinput. Reglerne afleder i stedet § 53 A eller § 53 B fra ordningens øvrige
produkt-, oprettelses- og indbetalingsfakta efter SKM2025.658.LSR og Den
juridiske vejledning 2026-1, C.A.10.4.2.5. En påberåbelse af, at der ikke blev
truffet noget valg, respekteres, og den nye blanket med afkrydsningsfelt kan
ikke bruges gennem det historiske spor. Moderne meddelelser beholder deres
eksplicitte mål. De to spor samles først som daterede valgposter, så det første
gyldige valg fortsat er bindende på tværs af sporene. I alt nitten fokuserede
invarianter passerer i både interpreter og compiler.

Historiske ordningers senere kontraktændringer er nu modelleret som daterede
kildefakta med entydig identifikation, virkningstidspunkt, kapitalværdi,
forhåndsaftale og en lukket ændringsart. Aftalelovens § 6, Den juridiske
vejledning 2026-1, C.A.10.4.2.3.6, SKM2013.481.SR og SKM2023.406.LSR er bundet
som typede kilder. Reglerne afleder, om ændringen ikke skaber en ny ordning,
skaber en ny ordning for hele kontrakten eller kun for en bestemt del. En
delvis ændring kræver, at beregningsrækken udtrykkeligt repræsenterer enten den
historiske rest eller den del, som den identificerede ændring skabte. Ukendte
ændringsarter, uoplyste eller modstridende forhåndsaftaler, dubletter og
omvendt kronologi fejler lukket. Seksten fokuserede invarianter passerer i både
interpreter og compiler.

Afkastfordelingen efter Pensionsbeskatningslovens § 53 A bruger nu den samme
daterede rettighedskæde som overgangsreglerne. Ved én rettighedshaver oplyser
kalderen ikke længere personens identitet endnu en gang; reglerne finder den
eneste relevante rettighedsperiode ud fra skatteyderens identifikation. Ved
flere samtidige berettigede knyttes de faktiske indeståender ved
afkastperiodens udgang til én identificeret, afledt rettighedsperiode. En
overdrager og en erhverver kan derfor få hver sin halvåbne afkastperiode i
overdragelsesåret. Det fokuserede 2026-scenarie beregner 20.000 kr. for den
tidligere ejer frem til overdragelsen og 29.000 kr. for erhververen efter
overdragelsen; betalinger på den anden side af grænsen medregnes ikke. Brudte
eller dublerede rettighedskæder, en ukendt fordelingsperiode og ufuldstændige
andelsfakta fejler lukket. Ved afståelse i 2024 og generhvervelse i 2026 vælger
reglerne den første periode i 2023, ingen periode i 2025 og den nye periode i
2026. De normale afkastregler beregner henholdsvis 15.000 kr. og 25.000 kr. i
kapitalindkomst. Flere adskilte ejerperioder i samme år bevares som
`Pbl53AFlereRettighedsperioderIIndkomståret` med periodernes identifikationer og
giver ingen kapitalpost. Otte fokuserede invarianter passerer i både interpreter
og compiler.

Fordelingsfakta for flere samtidige berettigede bruger nu den samme typede
oprindelse som de afledte rettighedsperioder. Kalderen vælger enten perioden fra
ordningens oprettelse eller perioden fra en navngiven erhvervelse. Den interne
tekstnøgle `oprettelse` er ikke længere en del af JSON- eller XLSX-kontrakten.
En ukendt eller tom erhvervelsesreference fejler lukket. Et fokuseret
erhvervelsesscenarie fordeler 29.000 kr. afkast med 40.000/160.000 til 7.250 kr.
for den berettigede i både interpreter og compiler.

Beregningskontrakten har 261 danske § 53 A-feltbeskrivelser med Aftalelovens
§ 6, PBL §§ 20, 53 A og 53 B, PSL §§ 3 og 4 samt AMBL § 2 som typede kilder.
Regnearket har fjorten
relationelle § 53 A-ark for ordninger, årlige fakta, daterede
grænsehændelser, berettigedes indeståender, indbetalings- eller
udbetalingshændelser, senere erhvervelser, overgangsvalg, historiske
blanketindsendelser,
daterede kontraktændringer,
livsforsikringsdækninger, daterede
negative afkast ved forløbets åbning og tre slags berettigede efter
direktørpensionstilsagn. Den fælles XLSX/JSON-afstemning dækker tre ordninger,
en senere erhvervelse, fem årsoptegnelser, en delårsgrænse, to berettigede, en
arbejdsgiverindbetaling på 60.000 kr., 35.500 kr. samlet kapitalindkomst og
13.000 kr. negativt afkast til særskilt fremførsel på den ordning, hvor tabet
opstod. XLSX-rundturen har desuden en selvstændig overdragersag med en præcis
overdragelsesgrænse, seks samlede årsoptegnelser og en afledt positiv
kapitalpost på 20.000 kr. for perioden før overdragelsen.

De sammensatte § 53 A-feltbeskrivelser afdækkede en sprogfejl i metadataindekset:
typede metadata, der var bygget med rene regelkald samt `concat`, `map` og
`flat_map`, blev accepteret, men forsvandt fra beregningsskemaet. Futuruna
evaluerer nu sådanne deterministiske grundværdier med et afgrænset trinbudget
og afviser effekter. Fokuserede regressionstests kræver både, at alle beregnede
felter udstilles, og at en beregnet ukendt inputsti stadig afvises.

Arbejdet afdækkede samtidig en Futuruna-fejl ved ens konstruktørnavne i flere
sumtyper. Typekontrollen, fortolkeren og Rust-kodegenereringen bruger nu den
forventede overtype og navngivne feltform til at vælge den rigtige
konstruktør, og match-kodegenereringen bevarer samme overtypekontekst.
Navngivne argumenter til et regelscope bliver samtidig ordnet efter
scope-parametrene og bevarer en rigtig regelscope-instans. Særskilte sprogtests
dækker fortolket konstruktion, regelscope-fold og kompileret match.

Latest integration: Den kanoniske `beregn_personskat`-graf modtager nu typede
kildefakta om udenlandske sociale bidrag efter Ligningslovens § 8 M. Skatteåret
og det almindelige arbejdsmarkedsbidrag af lønnen afledes af den omgivende
Personskat-sag; borgeren leverer hverken et fradragsbeløb eller en juridisk
konklusion. Reglerne udfører først § 8 M og derefter Personskattelovens § 3,
stk. 2, nr. 6. Da lønmodtagermodellen allerede fratrækker AM-bidraget, føres kun
de kvalificerende udenlandske person- og arbejdsgiverbidrag ind som yderligere
fradrag. Både § 8 M-resultatet, § 3-resultatet og det særskilte regnskab for
AM-bidraget bevares i svaret. Syv kompilerscenarier dækker fuld og begrænset
skattepligt, obligatoriske og frivillige bidrag, forkert indkomstår,
arbejdsgiverens indeholdelsespligt, modstridende skattepligtsfakta og en
personfradragsstatus, der ikke i sig selv beviser den underliggende
kildeskattepligt. Kontrakten har 14 nye danske feltbeskrivelser i et almindeligt
typet `Beregningsmeta`-objekt, som forbindes til beregningen med ét kort
`::meta:`-anker.

Latest integration: Den kanoniske `beregn_personskat`-graf modtager nu nul
eller flere typede CFC-forhold som kildefakta efter Ligningslovens § 16 H eller
§ 16 I, stk. 6 og 7. Skatteåret afledes fra den omgivende personsag, og
§ 16 H's danske sammenligningsskat afledes af selskabets samlede skattepligtige
indkomst og Selskabsskattelovens § 17-sats. Borgeren leverer derfor hverken et
færdigberegnet CFC-beløb eller sammenligningsskatten. Hvert forhold bevarer
både Ligningslov-inputtet, det fulde afhængighedsresultat og den afledte
Personskattelov § 4 b-post. Kun en samlet gyldig liste får skattemæssig virkning;
ugyldige basispoint eller ikke-understøttede år forbliver synlige og fejler
lukket. Det positive § 4 b-resultat føres til § 8 b's 22 pct. CFC-skat, mens
CFC-indkomsten fortsat holdes uden for §§ 6-8 a. Fokuserede scenarier dækker en
blandet positiv § 16 H/§ 16 I-sag, EU/EØS-fritagelsen, fremført negativt
merafkast og ugyldig ejerandel. § 4 b's ordrette tekst og regler ligger nu i
det særskilte `personskatteloven-par4b-cfc.runa`-modul.

Den genererede borgerkontrakt har samtidig 28 nye danske CFC-feltbeskrivelser,
herunder den relationelle CFC-liste, regelvalg og samtlige nødvendige
kildefakta. Metadataen ligger i et almindeligt typet `Beregningsmeta`-objekt
i et importeret modul og forbindes med ét kort `::meta:`-anker. Futurunas
beregningskontrakt indlæser nu målrettet sådanne ankre fra rekursive imports;
andre labels ignoreres, mens ugyldige stier og dubletter fortsat fejler lukket.

Latest integration: Pensionsbeskatningslovens § 15 A har nu et typet,
dateret holdingforløb. Et salg eller en likvidation af et datterselskab bærer
en stabil hændelses- og selskabsidentifikation, et ordnet tidspunkt og det
faktiske kontantprovenu. Reglerne følger selskabet fra de historiske
regnskabsperioder til personens afståelsestidspunkt og afviser uforklarede
huller, genbrugte identifikationer og samtidige opgørelser af aktier,
underliggende aktiver eller provenu. Når holdingselskabet består, medregnes et
beholdt provenu præcis én gang som passivt aktiv. Ved dokumenteret likvidation
fjernes provenuet fra afståelsesopgørelsen; en afvikling efter den eksplicitte
12-måneders praksisgrænse udløser alligevel pengetankresultatet. Fem særskilte
interpreter- og kompilerscenarier dækker fortsat ejerskab, salg med beholdt
provenu, et hændelsesforløb uden holding, rettidig likvidation og sen
likvidation. Den kanoniske kontrakt har 27 nye danske feltmetadata-poster for
forløbet og 734 feltmetadata-poster i alt.

Latest integration: Pensionsbeskatningslovens §§ 16 og 18 bruger nu en særskilt
kildebelagt årsparameterportefølje for 2010-2026. Hvert resultat bærer både
indkomståret og den officielle SKM-tidsserie, som leverer årets loft for
ratepension og ophørende livrenter, et eventuelt kapitalordningsloft og
opfyldningsfradraget. Porteføljen bevarer 100.000 kr.-overgangen i 2010-2011,
faldet til 50.000 kr. i 2012, kapitalfradragets bortfald efter 2012 og den
efterfølgende § 20-regulering. De 17 understøttede år er både afstemt mod de
officielle tabeller og genberegnet mod Personskattelovens § 20. Den officielle
2002-2009-tabel oplyser ikke et samlet rateloft for 2009; året er derfor
eksplicit ufuldstændigt og fejler lukket sammen med år efter 2026.

Latest integration: Den kanoniske `beregn_personskat`-graf modtager nu en
årsportefølje af identificerede pensionsindbetalinger efter
Pensionsbeskatningslovens § 18. Reglerne afleder fradragsåret, deler § 16-loftet
mellem ordningerne, giver arbejdsgiverindbetalinger prioritet efter indeholdt
arbejdsmarkedsbidrag og anvender ét fælles valg for livsvarig livrente. Et
aktivt tiårigt fordelingsforløb viderefører fradragsretten efter det oprindelige
betalingsår. Det ordinært fordelte livrentefradrag begrænses samtidig af de
forfaldne præmier, bidrag og indskud, som endnu ikke er fratrukket, også når den
beregnede tiendedel er større end den resterende fradragsramme. En
indeksordning skal nu bære de typede § 15-fakta om ordningsform og den
lovbestemte indekskontrakt. Ophørspension efter § 15 A tager tilsvarende imod
afståelser, fortjenester, passive kapitalposter, ordninger, kvalifikationsår og
tidligere indbetalinger som kildefakta. Reglerne afleder selv personens fælles
regulerede loft, hver afståelses resterende fortjeneste og den aktuelle
fordeling i indbetalingernes angivne rækkefølge. Det tidligere rå felt
`særligt_ordningsmaksimum_kroner` er fjernet. Det afledte § 18-fradrag føres
gennem Personskattelovens § 3, stk. 2, nr. 3 og genbruges som grundlag for
Ligningslovens § 9 L. Et gyldigt § 15 A-valg kan i stedet placere en del i
positiv aktieindkomst efter Personskattelovens § 4 a; kun resten fradrages i
personlig indkomst. Dermed kan hverken fortjeneste, fælles loft eller samme
indbetaling bruges to gange. Ugyldige årsporteføljer bevares i resultatsporet,
men giver hverken personligt fradrag, aktieindkomstfradrag eller ekstra
pensionsfradrag.

`@ calculate("Dansk personskat")` vises nu som titelrække på hovedarket og som
præfiks på alle relationelle ark. Læseren validerer titlerne sammen med den
skjulte kontrakt, så et menneskeligt navn ikke kan drive væk fra den
deterministiske beregningsgrænse. Pensionsgrenen har præcise danske etiketter,
interviewspørgsmål, hjælp, enheder og kilder på alle sine nye inputstier. En AI
kan derfor indsamle pensionsfakta og udfylde kontrakten, mens Futuruna alene
afgør, beregner og sporer den skattemæssige virkning.

Latest integration: Den kanoniske `beregn_personskat`-graf modtager nu nul eller
flere identificerede kørsels-, udgifts- og godtgørelsessager for
lønmodtageres erhvervsmæssige befordring efter Ligningslovens § 9 B. Hver sag
angiver sin fortløbende kronologiske rækkefølge og bevarer den godtgørende
arbejdsgiver. Reglerne udleder selv både arbejdsgiverens hidtidige kilometer og
den fælles kilometerhistorik for direkte fradrag fra de foregående sager;
historikken er ikke et borgerfelt. Skatteåret, lønmodtagerrollen og Skatterådets
fradragsmetode afledes tilsvarende af den omgivende skattesag. Reglerne afgør
først hver sags
60-dages-forhold, erhvervsmæssige kilometer, administrative
godtgørelsesbetingelser, skattefri godtgørelse og et eventuelt direkte fradrag
for kundeopsøgende aktivitet for flere arbejdsgivere. Derefter summeres kun en
samlet gyldig årslistes beløb. Fradraget føres gennem Personskattelovens § 3,
stk. 2, nr. 8. Skattepligtig godtgørelse føres som løn ind i både
AM-grundlaget, lønmodtagerfradragene og § 9 C's aftrapningsindkomst, mens det
direkte § 9 B-fradrag kun reducerer den personlige indkomst. De to beløb kan
derfor ikke medregnes dobbelt. Dublerede sagsidentifikationer, rækkefølger med
huller, ikke understøttede skatteår og negative kildefakta får
ingen skattemæssig virkning, men hvert delresultat forbliver synligt. Den
genererede kontrakt har 28 danske
§ 9 B-feltmetadata-poster med interviewspørgsmål, hjælp, enheder og
kildehenvisninger.

Recent integration: Den kanoniske `beregn_personskat`-graf modtager nu
kildefakta om driftsresultater fra bolig-, fritids- og lignende ejendomme efter
Personskattelovens § 4, stk. 1, nr. 6. Borgeren eller en interviewende AI
leverer ejendomstype, beliggenhed, erhvervsmæssig udlejning, eventuelle særlige
betingelser og årets underskud eller overskud. Skatteåret kommer fra den
omgivende Personskat-sag. Reglerne afleder selv hjemlen efter
Ejendomsskattelovens § 3, inklusive ændringen fra LOV nr. 679/2023 og
omnummereringen ved LOV nr. 615/2026 fra 1. januar 2027, og danner først derefter
§ 4-kapitalposten. En ejendom, der ikke er omfattet af Personskattelovens § 4,
stk. 1, nr. 6, og erhvervsmæssig udlejning er gyldige kildefakta med et synligt
beløb; de forsvinder
ikke som ugyldigt eller manglende input. Seks menneskelige feltmetadata-poster
giver arbejdsbogen og et AI-interview danske etiketter, spørgsmål, hjælp,
enhed og typede kilder, mens de stabile maskinstier er uændrede.

Latest integration: EBL § 6 D-årsforhold er nu ordnede lister af typede poster
frem for ét samlet betalings- og hændelsesfelt. Hver betaling navngiver den
berørte pantebrevsdel, og hvert ejerskifte navngiver både kilde- og
modtagerdelen. Betalinger før og efter ægtefællesuccession, dødsfald eller
boets overtagelse fordeles derfor efter den faktiske rækkefølge i samme
indkomstår. Et særskilt input for uafklaret betalingstidspunkt fejler lukket i
stedet for at vælge en ejer ved gæt. Lovteksten står fortsat ordret, og
metadataankeret henviser både til EBL § 6 D og bemærkningerne til L 36.

Kursgevinstlovens selvstændige behandling af sælgerpantebrevet forbruger den
samme ordnede postrække. Pantebrevets kontantværdi er anskaffelsessum,
hovedstolen frigives forholdsmæssigt ved afdrag, afståelse eller indfrielse, og
årets gevinst eller tab føres til Personskattelovens § 4, stk. 1, nr. 2 uden at
blive blandet sammen med den udskudte ejendomsavance. EBL-hændelsen bestemmer
fremrykningen ud fra berørt restgæld; KGL bruger særskilt det faktiske provenu.
Et pantebrev til kurs 80 giver derfor i vejledningens eksempel 300.000 kr. i
årlig EBL-fortjeneste og 75.000 kr. i årlig kursgevinst ved et afdrag på
375.000 kr. Broen sammenligner nu EBL's forventede realisation med KGL's
faktiske realisation for hver dannet disposition, så de to love ikke kan være
uenige uden at hele inputkæden bliver ugyldig.

Latest integration: Den kanoniske `beregn_personskat`-graf modtager nu
identificerede finansielle kontrakter og tidligere indkomstår som observerbare
kildefakta efter Kursgevinstlovens §§ 29-33. Kontraktens anskaffelses- og
afståelsesværdier, kronologi, underliggende aktiv og dokumenterede relation
afleder selv årets KGL-resultat, § 32-klasse og eventuelle ABL-reference. ABL
§ 17-relationen findes via samme stabile aktividentifikation; borgeren leverer
hverken et fradrag eller en retsklassifikation.

Historiske år genberegnes i rækkefølge, så tidligere gevinster, fremførte tab,
ægtefælleoverførsler og aktiemodregning kun føres én gang. Arbejdsbogen har
særskilte relationelle ark for aktuelle kontrakter, historiske år og deres
kontrakter med menneskelige danske etiketter og typede kildespor. Den fulde
XLSX/JSON-rundtur afstemmer identiske resultater: Et tab på 10.000 kr. fra 2025
modregnes i 6.000 kr. gevinst i 2026 og efterlader 4.000 kr. til fremførsel; et
ABL § 17-aktiv med 5.000 kr. gevinst og et identificeret kontrakttab på 9.000
kr. giver -4.000 kr. i samlet personlig omklassifikation. Blandede ABL § 19
B-/§ 19 C-/§ 22-grundlag afvises foreløbigt frem for at blive fordelt uden en
kildebelagt allokeringsregel. KGL § 33-kildeadapteren understøtter i denne
version kun ikke-negative primo-, ultimo- og afregningsværdier; kontrakter med
negativ dagsværdi kræver den særskilte, kildebelagte udvidelse `td-89a28a` og
afvises indtil da.

Kildeskattelovens §§ 26 B-27 kan nu lade en dansk ægtefælle fortsætte med samme
anskaffelsessum og -tidspunkt. Dødsboskattelovens § 20 kan lade boet fortsætte,
og §§ 58-59 kan føre positionen direkte til den længstlevende ægtefælle. Ved
udlodning fra boet afledes successionen nu af boets skattepligt, et eventuelt
valg af salgsbeskatning, dansk beskatningsret og forholdet mellem
boopgørelsesværdien og den resterende anskaffelsessum. Tab eller et valg af
salgsbeskatning udløser derfor realisation i boet i stedet for at bevare
grundlaget hos ægtefællen. Den kanoniske
Personskat-graf medregner kun realisationer, der både tilhører beregningens
skatteyder og har personrollen; boets indkomst kan derfor ikke glide ind i
afdødes eller ægtefællens personskat. Delvise ægtefælleoverførsler opdeler
hovedstol, grundlag og den resterende udskudte EBL-fortjeneste præcist i
identificerede pantebrevsdele. Hver årlig EBL-medregning bærer derefter
KGL-positionens ejeridentitet ind i Personskat. 2028-scenariet fordeler derfor
210.000 kr. til sælgeren og 90.000 kr. til ægtefællen og afstemmer de to beløb
til årets samlede 300.000 kr. Boets poster holdes uden for begge personers
beregning. Andre modtagere end de modellerede ægtefælle- og bogrene fejler
fortsat lukket.

Recent integration: Den kanoniske `beregn_personskat`-graf modtager nu egne og
en samlevende ægtefælles faktiske ejendomsafståelser, tidligere års uudnyttede
ejendomstab og samlivsstatus. Skatteåret afledes fra den omgivende skattesag.
Reglerne beregner hver afståelse efter Ejendomsavancebeskatningslovens §§ 1,
1 A, 2, 4, 5, 5 A, 8, 9 og 11, anvender § 6's tabsafgrænsning, egen modregning,
ægtefælleoverførsel og fremførsel og danner derefter Personskattelovens § 4,
stk. 1, nr. 14-kapitalpost. Det tidligere rå input for regulering efter §§ 5 og
5 A er fjernet. Borgeren oplyser i stedet anskaffelsesdato og -grundlag,
ejerandel, om hele eller en del af ejendommen afstås, vedligeholdelses- og
forbedringsudgifter, nedsættelser, eventuelle mælkekvoter, § 5, stk. 6-fakta og
indekseringsvalg; reglerne udleder det regulerede anskaffelsesgrundlag.
Mælkekvoter købt eller tildelt senest i 2004 får deres anskaffelsesgrundlag og
afståelsesvederlag fordelt efter disponerede enheder og en eventuel
delafståelse. Ved en delafståelse bruger det almindelige § 5-tillæg
anskaffelsessummen for hele ejendommen, mens mælkekvotevederlag bruger den
særskilte anskaffelsessum for den del, der ikke udgør boligdelen. Udløb
behandles som afståelse til 0 kr., mens toldning uden erstatning bevarer lovens
undtagelse. § 5, stk. 6 er obligatorisk, når betingelserne er opfyldt, og
omfatter både de to nummererede 1993-indgangsværdier og en vurdering efter den
dagældende vurderingslovs § 4 B. Reglen vælger den højeste af
tillægsparcelværdien og den tekniske værdi, fratrækker de
vurderingsomfattede bygningsbeløb og fordeler resten efter ejerandel og den
afståede jords anskaffelsessum. Det overførte rå beløb nedsætter samtidig
restejendommens anskaffelsessum. Ejendomstypen og dens kildefakta udleder
tilsvarende
parcelhusfritagelsen efter § 8 eller fordelingen mellem bolig- og erhvervsdel
efter § 9. Sommerhusgrenen kræver samme grundbetingelse som § 8, stk. 1, og
kan derfor ikke fritages alene på baggrund af privat anvendelse. Skattefri
boligfortjeneste, skattepligtig erhvervsfortjeneste og
afskåret boligtab bevares særskilt i auditsporet. Hvis boligbetingelserne i
§ 9 ikke er opfyldt, regulerer § 5 A i stedet hele ejendommens
anskaffelsesgrundlag; denne gren er dækket særskilt i både interpreter og
kompileret scenariekørsel. Modsat fortegn på bolig- og erhvervsdelen holdes
ligeledes adskilt: en skattefri boliggevinst udligner ikke et fradragsberettiget
erhvervstab, og et afskåret boligtab udligner ikke en skattepligtig
erhvervsgevinst. Et § 5 A-valg afvises som modstridende input, hvis den valgte
landbrugs-, skov- eller naturkategori ikke svarer til ejendommens typede
klassifikation.

Flere afståelser ligger i to relationelle regnearksfaner frem for brede
gentagne kolonner. Ægtefælleoverførsel begrænses af modtagerens
nettofortjeneste efter egne tab, så et overskydende tab ikke forsvinder i en
allerede forbrugt bruttofortjeneste. Genanbringelse efter §§ 6 A, 6 C og 10 er
nu et fælles typet domæne med oprindelig erhvervsfortjeneste, frister,
erhvervsanvendelse, ejerskab, placering, begæring, investeringsgrundlag,
genopførelsesfakta og senere reguleringer. En almindelig erhvervsejendom bærer
nu samme typede genanbringelsesdomæne. Ved en senere ordinær afståelse
nedsættes den kontante anskaffelsessum med det stadig aktive nedslag, før
§§ 5 og 5 A regulerer anskaffelsessummen. Den gamle fortjeneste tilføjes ikke
særskilt igen. Hvis bestemmende indflydelse allerede er ophørt, viser
auditsporet i stedet den tidligere beskatning og et aktivt nedslag på 0 kr., så
den senere afståelse bruger den ikke-nedsatte anskaffelsessum. § 8, stk. 5
beskatter den tidligere
genanbragte fortjeneste særskilt, når den nye ejendoms egen boligfortjeneste er
skattefri. § 9, stk. 4 udleder selv, om erhvervsdelen kan bære hele den
genanbragte fortjeneste; hvis ikke, beskattes den gamle fortjeneste særskilt,
og det tilsvarende nedslag i anskaffelsessummen bortfalder. Den gamle gevinst
og den nye ejendoms gevinst eller tab holdes dermed adskilt uden
dobbeltbeskatning. § 11, stk. 2 er nu tilsvarende kildeafledt og har erstattet
det tidligere boolske input om en genanbragt erhvervsfortjeneste. Ved
ekspropriation af den ejendom, som en tidligere fortjeneste er genanbragt i,
forbliver den nuværende ejendoms egen fortjeneste fritaget efter stk. 1. Den
regulerede gamle fortjeneste beskattes særskilt, og det tilhørende aktive
nedslag i den nuværende ejendoms anskaffelsessum bortfalder præcis én gang. Et
typet valg kan i stedet føre hele eller en del af den gamle fortjeneste videre
efter §§ 6 A eller 6 C. Ugyldige historiske fakta og et nyt
genanbringelsesvalg uden en aktiv gammel fortjeneste bevares som synligt
ugyldigt input. Den almindelige tabsberegning for den eksproprierede ejendom
bevares uafhængigt af den gamle fortjeneste. Mælkekvoter og § 5, stk.
6-overførsler er ikke længere
fail-closed særforhold; de er selvstændige, kombinerbare kildefaktagrene med
synlige delresultater og § 5 A-henføringsår. Fordeling af § 10-genopførelse på
flere andre ejendomme og erhvervserstatning anvendt til ejerbolig er fortsat
udtrykkelige og fail-closed. Værdipapirer med boligret efter
§ 8, stk. 4, er derimod nu flyttet ud af ejendomsavancegrenen og ind i
Aktieavancebeskatningslovens § 15-spor. Beboelse, ejendom med flere
beboelseslejligheder, udstederens direkte ejerskab, tidsmæssigt overlap mellem
boligbrug og den kvalificerende periode, eventuelt bestemt grundareal og
likvidationsåret afgør fritagelsen. Udstederens selskabsform eller
foreningstype, danske skattemæssige hjemsted og undtagelser efter
Selskabsskattelovens §§ 1 og 3 samt værdipapirets ABL-status afleder først, om
udstederen er et selvstændigt skattesubjekt, og om værdipapiret er omfattet af
ABL. En transparent udsteder eller et værdipapir uden for ABL gør hændelsen
ugyldig. Hvis et ellers gyldigt ABL-forløb alene ikke opfylder en
fritagelsesbetingelse, fortsætter gevinst eller tab gennem de almindelige
ABL-regler i stedet for at blive nulstillet. Et fokuseret scenarie dækker de
årlige 10.000 kr.-tillæg,
årsaggregering af forbedringsudgifter, § 5 A-indeksering, § 8-fritagelsen,
§ 9-fordelingen og ABL § 15's boligretsgren. Et særskilt ordinært
genanbringelsesscenarie og en `.audit.runa`-fil kontrollerer, at det aktive
anskaffelsessumsnedslag anvendes præcis én gang, at kontrolophør fjerner
nedslaget, og at et nedslag større end ejendommens grundlag afvises. Et nyt
fokuseret scenarie dækker
desuden mælkekvotekøb, vederlagsfri tildeling, delvis disposition, udløb,
toldning, § 5 A-år, § 5, stk. 6-værdiansættelse, 2027-overgangen for skov og
natur samt samtidig anvendelse af begge grene. En separat `.audit.runa`-fil
kontrollerer afstemning, direkte ejerskab, grundbetingelsen,
mælkekvoternes stk. 3/stk. 4, nr. 6-bevægelser og identiteten mellem § 5,
stk. 6-overførslen og restejendommens nedsættelse.

Recent integration: Den kanoniske `beregn_personskat`-graf modtager nu
kildefakta for befordring efter Ligningslovens § 9 C og den valgfri
erstatningsregel i § 9 D. Borgeren angiver arbejdsdage, afstand,
befordringsformål, godtgørelser, bropassager, særlig transport og eventuelle
§ 9 D-forhold. Skatteåret og § 9 C's aftrapningsindkomst afledes i den
nuværende almindelige lønmodtagervej fra det omgivende skatteår og
AM-bidragsgrundlaget; de er ikke regnearksfelter. Hvis modposten for fri bil er
AM-bidragspligtig, indgår den også i det endelige aftrapningsgrundlag.
§ 9 D-resultatet afledes internt. Ugyldige eller årsmæssigt forkerte fakta
bevares i resultatets auditspor, men får ingen skattemæssig virkning.
Arbejdsgiverbetalt befordring er et typet kildefaktum: et frikort til offentlig
transport føres til personlig B-indkomst uden AM-bidrag, mens modposten ved fri
bil føres til AM-bidragspligtig personlig indkomst. Et gyldigt § 9 D-fradrag
erstatter det ordinære § 9 C-fradrag.

Den samme graf modtager renteindtægter, renteudgifter, valgfri fradrag efter
Ligningslovens §§ 6 og 6 A samt identificerede omkostninger efter
Personskattelovens § 4, stk. 2. § 4, stk. 3 omklassificerer både posterne og de
tilhørende omkostninger til personlig indkomst ved fordrings-, kontrakt- eller
finansieringsnæring. Rente- og ABL-kapitalposter går gennem den samme
§ 4-opgørelse i stedet for parallelle nettobeløb.

Det typede input genererer et regneark direkte fra den nåbare domænegraf med
156 synlige overskriftsceller på `cases`-arket inklusive `case_id` og 52
relationelle kildeark. Pensionsindbetalingerne og de valgte indekskontrakter
ligger i to nye nøglebundne tabeller. § 9 B-sagerne ligger i deres egen tabel
med 30 kolonner inklusive `case_id`, `item_id` og `position`. Kontrakten når nu
394 domænedefinitioner. De to
ejendomsark rummer
også de kildefakta, der kræves af §§ 6 A, 6 C og 10, så en aktiv genanbringelse
kan følges gennem en ordinær afståelse, § 8, stk. 5 eller § 9, stk. 4 uden
beregnede mellemfelter fra brugeren.
Omkostninger efter § 4, stk. 2 ligger i deres egen nøglebundne tabel, to ark
rummer egne og ægtefællens ejendomsafståelser, og de indlejrede § 5-fakta giver
selvstændige relationelle tabeller for vedligeholdelses- og
forbedringsudgifter, nedsættelser og egne eller ægtefællens mælkekvoter. De
øvrige relationelle ark kommer blandt andet fra de ordinære og særlige
ABL-grene. Den udfyldte kombinationssag giver samme fulde resultat fra XLSX og
kanonisk JSON for renter, fradrag, befordring, to egne ejendomsafståelser,
ægtefællens ejendomstab og fremført tab. En særskilt roundtrip-sag bærer
ejerandel, delafståelse, en faktisk mælkekvote og § 5, stk. 6 gennem begge
adaptere.
Regnearksgeneratorens enum- og variantvalg ligger i et skjult `_choices`-ark
og bruges gennem navngivne celleområder; dermed gælder Excels grænse på 255
tegn for indlejrede valglister ikke for store domæneunioner.

Generisk feltmetadata er nu integreret i beregningskontrakten og regnearket.
Den kanoniske grænse hedder `@ calculate("Dansk personskat")`, så kontrakt og
regneark også har en menneskelig titel uden at ændre den stabile entry-nøgle.
Titlen står synligt øverst på alle inputark og valideres ved indlæsning.
Synlige kolonneoverskrifter bruger en udtrykkelig menneskelig etiket, mens den
stabile maskinsti, interviewspørgsmål, hjælp, enhed og typede kildereferencer
bevares i kontrakten og de skjulte regnearksfaner. Personskat-kontrakten har
etiketter og interviewspørgsmål for skatteår, kommune, bruttoløn, befordring,
pensionsindbetalinger, pensionsvalg, aldersstatus, kirkeskat, renter,
årsopgørelse og de centrale
ejendomsavancefakta samt ordinære aktiebeholdninger og boligret efter ABL § 15.
Kontrakten har aktuelt 1.518 eksplicitte feltmetadata-poster. Heraf beskriver
36 personrollen, rejsetabellen, arbejdsstedet, overnatningsforholdene,
12-månedersperioden, godtgørelsen, udgifterne, fradragsvalget og det fælles
årsloft efter Ligningslovens § 9 A. Yderligere 66 beskriver
egne og ægtefællens § 10-skadeforløb, genopførelsesejendomme,
ejerboligfordeling, frister, regulering og afskrivningsforhold. Alle 98 nåbare
§ 15 A-stier for en virksomhedsafståelse har en dansk etiket og et
interviewspørgsmål, herunder regnskabsperioder, ejerkæder og underliggende
selskabsindtægter og -aktiver. Seks af posterne
beskriver ejendomsdrift efter Personskattelovens § 4, stk. 1, nr. 6:
variantvalg, ejendomstype, beliggenhed, erhvervsmæssig udlejning, særlige
betingelser og årets underskud eller overskud. De otte
ABL § 15-poster spørger til udstedervariant, registreret selskabsform,
selskabets og foreningens danske skattemæssige hjemsted, foreningstype,
SEL § 3-undtagelse, Fondsbeskatningsloven og værdipapirets ABL-status. AI'en
leverer dermed de juridiske kildefakta; Futuruna afleder udstederens
skattesubjektstatus og fritagelsens sporbare resultat. De øvrige nye poster
navngiver ejerandel, delafståelse, de særskilte hele og ikke-boligdelens
anskaffelsessummer, mælkekvotetabellerne og alle deres dato-, enheds-,
anskaffelses- og dispositionsfelter samt § 5, stk. 6's værdiansættelse og
jordfordeling for både personen og ægtefællen. EBL § 6 D-valget,
sælgerpantebrevet, parternes faktiske anvendelse, meddelelsen, sikkerheden,
ejendommens placering og de efterfølgende års ordnede poster har tilsvarende
egne etiketter og interviewspørgsmål. Årslisten og dens postliste ligger i to
nøglebundne regnearksfaner; posttype, betalt pantebrevsdel, betaling,
fremrykningshændelse og kilde-/modtageridentiteter har menneskelige navne uden
at erstatte de kanoniske maskinstier. Det samme gælder KGL-valget, pantebrevets
identifikation, skatteyderens seks kildefakta, årets øvrige § 14-grundlag og
senere afståelser eller indfrielser med år, art, berørt hovedstol og provenu.
Både den berørte pantebrevstranche og en modtagers nye tranche har en
menneskelig etiket, et interviewspørgsmål og hjælp til at bevare den stabile
identitet gennem senere betalinger og ejerskifter.
Beregningens person, den valgfrie ægtefælleidentitet, pantebrevets oprindelige
skatteyder og de typede fakta om ægtefælle- eller dødsbosuccession har også egne
etiketter og spørgsmål. Det
omfatter modtageridentiteter, bopæl, boets identitet og skattefritagelse,
salgsvalg, fortsat dansk beskatningsret og boopgørelsesværdien.
EBL § 11, stk. 2 har yderligere 66 poster for personen og ægtefællen. De dækker
valget om ny genanbringelse, anvendelse, selskabsforhold, investering,
udenlandsforhold, begæringsdatoer, ejerskab og hjemmel efter §§ 6 A eller 6 C.
Genanbringelsens
lovgrundlag, oprindelige afståelsesår og erhvervsfortjeneste,
geninvesteringsår, erhvervsmæssige anskaffelsesgrundlag, anvendelse, placering,
begæring, ejerskab og overgangsforhold har nu egne menneskelige etiketter og
interviewspørgsmål for både personen og ægtefællen. De ordinære ejendommes
genanbringelsesvalg, selskabets erhvervsbrug,
kapital- og stemmeandele samt om den bestemmende indflydelse fortsat består,
har samme menneskelige lag. Det gælder også de
udenlandske betingelser om EU/EØS-område, fuld dansk skattepligt,
informationsudveksling og den særskilte begæring med oplysninger og
driftsbudget før fraflytning. En AI kan dermed indsamle kildedata i
menneskelige ord, udfylde de kanoniske stier og lade Futuruna beregne
deterministisk med den juridiske forklaringskæde bevaret. En metadataændring
ændrer kontraktens fingerprint, så gamle interview- og regnearksskabeloner
afvises som forældede. Den verificerede kontrakt har aktuelt fingerprint
`92f3fe8b57cdf4d3667a18453805197019fc93f199523ad2ff199c1e1f2ba8f9`.
De 18 nye udlejningsfelter efter ligningslovens § 15 Q har alle en dansk
etiket, et interviewspørgsmål, hjælp og en typet retskilde. De omfatter
boligrolle, udlejningsform, bolig- og indberetningsstatus, fradragsmetode,
beløb, et tidligere metodevalg samt den valgfri samordning med § 15 P. AI'en
leverer dermed kildefakta; Futuruna afleder selv bundfradrag, 40-pct.-fradrag,
samordning, skudårets længde og kapitalindkomst efter personskattelovens § 4,
stk. 1, nr. 17.
Felter uden en udtrykkelig etiket får nu en læsbar, deterministisk
sti-afledning i stedet for rå snake-case i regnearket; den kanoniske sti står
fortsat i kolonnens note. Den afledte tekst er kun et fallback, indtil feltet har
sin præcise juridiske etiket og sit interviewspørgsmål. Den aktuelle genererede
projektmappe materialiserer 1.577 eksplicitte etiketter, mens de øvrige
domænekolonner fortsat bruger dette fallback. Metadataudbygningen er derfor en
synlig korpusopgave og ikke skjult som færdig brugeroplevelse.
Beregningskald initialiserer nu den rene Futuruna-graf én gang pr. batch og
nulstiller derefter miljø og runtime-tilstand for hver sag. Den fulde
XLSX/JSON-afstemning, inklusive historiske § 6 D-, KGL- og § 11-forløb,
ejendomsdrift efter § 4, stk. 1, nr. 6, PBL § 53 A og de øvrige
kildefaktasager, passerer på 1.766,88 sekunder i den aktuelle debug-gate med
den typede rettighedsperiodereference.
`td-6659f1`
følger op på kontraktcache eller et kompileret beregningsspor, så samme
deterministiske kontrakt kan bruges interaktivt i et AI-interview.

Personskat-resultatet materialiserer nu sit kapitalindkomstforløb som en
enkelt afhængighedsgraf: rå EBL, KGL-broen, ejerfordelt EBL, valideringer og
kapitalposter beregnes én gang og genbruges i resultatet. Den fokuserede
ægtefællesuccession ligger i en selvstændig `.scenario.runa`-fil med delte,
regelbaserede fixtures. Den kompilerede scenario validerer 210.000 kr. hos
sælgeren, 90.000 kr. hos ægtefællen, samlet 300.000 kr. og nul
kapitalindkomst, når den nødvendige ægtefælleidentifikation mangler. Den
tidligere gentagne RuleScope-evaluering brugte mere end 22 minutter uden at nå
assertionerne; den materialiserede graf afslutter samme fokuserede scenario på
omkring fire minutter inklusive generering, kompilering og kørsel. Generisk
RuleScope-memoisering er nu implementeret i den kompilerede backend under
`td-95076b`: nul-argumentsregler cachelagres kun under én offentlig
RuleScope-rodevaluering, og parameteriserede regler samt almindelige metoder
deler samme evalueringsramme. En instrumenteret compiler-regression beviser, at
to søskendekald kun udfører den underliggende regel én gang. De private
memo-felter, hjælpermetoder, lokale variabler og hjælpetyper får friske Rust-navne,
så domæneregler som `depth()` og tilsvarende kildeidentifikatorer ikke kolliderer
med compileren. Den fulde `runa`-binsuite passerer 373 tests, og det sammensatte
Personskat-pensionsscenario gik fra mere end 30 minutters fuld CPU uden
afslutning til omtrent 89 sekunder med alle 13 invarianter bestået.
Kontraktcache er fortsat selvstændigt performancearbejde.

Previous integration: Den kanoniske graf modtager ABL-kildefakta for både
ordinære aktiers hændelsesforløb og de særlige aktivgrene i §§ 17-22. Grafen
afleder Personskattelovens personlige indkomst, kapitalindkomst og
aktieindkomst fra ABL-resultaterne; ugyldige kildekæder afvises og bevares som
synlige ugyldige resultater. Personlig indkomst fra ABL holdes særskilt fra
lønnen, så den ikke bliver gjort til AM-bidragspligtig løn eller udløser
lønmodtagerfradrag.

Earlier integration: Personskattelovens § 4, stk. 1, nr. 5 b modtager nu
kildefakta gennem `AktieavanceAktivklassifikationInput` i stedet for en
kaldervalgt § 19 C-/§ 22-etiket og et løst § 17-modprøveflag. ABL-reglerne
afleder den effektive aktivklasse og det kontrafaktiske § 17-resultat;
Personskatteloven bruger derefter almindelige `|`-regler, betingelser og
undtagelsen i stk. 6. Fem fokusscenarier dækker § 22, almindelig § 19 C,
§ 19 C hvis § 17, en ugyldig direkte klassepåstand og en ordinær aktie uden
for nr. 5 b i både interpreter og kompileret kode. Den eksisterende audit
bevarer det afledte ABL-resultat i sit spor og passerer alle 13 invarianter i
begge backends.

Arbejdet afdækkede samtidig en generel interpreter/codegen-forskel:
topniveau-bindinger kunne i interpreter fejlagtigt blive `Falskt`, når de
brugte en regel, funktion, type eller rulescope, som stod senere i samme fil.
Interpreteren registrerer nu statiske deklarationer før evaluering ligesom
codegen. Direkte og importerede regressionsprøver fastholder semantikken.

Earlier integration: Aktieavancebeskatningsloven §§ 19 B-19 C og §§ 21-22 er
nu afledte klassifikationer frem for mærkater valgt af kalderen. Den
gennemsnitlige aktivmasse bygges af direkte aktiver og KGL §§ 29-33-aktivers
underliggende aktiv. Ejerandelen angives som ejede og samlede kapitalenheder;
ved præcis 25 pct. eller mere erstattes markedsværdien af den forholdsmæssige
underliggende aktivmasse, når den ejede enhed faktisk er aktiebaseret; en
obligationsbaseret ejerpost medregnes fortsat direkte som et ikke-kvalificerende
værdipapir. § 19 B's meddelelse, nyoprettelsesfrist,
1. november-grænse, 1. juli-oplysninger og statusskift til § 19 C samt
§ 21/§ 22-grænsen afledes i scoped `|`-regler med integritetskontrollerede
resultater. Den samme faktabårne klassifikation afgør nu § 23, stk. 4 og 8,
Personskattelovens personlige, kapital- eller aktieindkomst og
Kursgevinstloven § 32. Fyrre fokusscenarier passerer i interpreter og
kompileret kode. KGL-aggregatet klassificerer hvert årsresultat én gang før
summering, så fuld produktvalidering bevares uden gentagen eksponentiel
genberegning. Domænet er samtidig egnet til det genererede regneark: brugeren
angiver aktiver og ejerposter som fakta, mens § 19 B/§ 19 C/§ 21/§ 22-status
aldrig bliver en inputkolonne.

Aktieavancebeskatningsloven §§ 6-7 er nu en kildebundet,
typet grænse fra skattepligt efter Selskabsskatteloven, Fondsbeskatningsloven,
Kildeskatteloven eller Dødsboskatteloven til lovens § 6- og § 7-regelspor.
Resultatet afledes gennem en scoped `|`-regelkaskade og kan ikke forfalskes ved
at kombinere et grundlag med vilkårlige resultatfelter. § 17, § 23 og § 9
forbruger og integritetskontrollerer samme resultat. Livsforsikringsselskabets
§ 6-status bevares som en særskilt, materiel subtype til § 23, stk. 6-7. Elleve
fokusscenarier passerer i både interpreter og kompileret kode.

Aktieavancebeskatningsloven § 5 A er nu et genbrugeligt,
kildebundet afståelsestabsresultat. Det opgør skattefrie udbytter, den del af
en dobbeltbeskatningslempelse, der overstiger den betalte udenlandske skat,
endnu uudnyttede præferenceudbytter og de kvalificerede koncernbeløb særskilt.
Reduktionen begrænses til bruttotabet, § 22, stk. 6-undtagelsen bevares, og
LOV nr. 254/2011's særlige overgang for tidligere statusskifter er
udtrykkelig. Resultatfelterne integritetskontrolleres mod det genberegnede
resultat, før § 9 bruger dem. Lageropgørelsen bærer nu identificerede
afståelsesposter med skattemæssig værdi og afståelsessum; § 5 A-oplysninger
kræves præcis én gang for hver post med et faktisk afståelsestab og ikke for
gevinstposter eller LL § 16 B-poster. Tyve fokusscenarier passerer i både
interpreter og kompileret kode.

§ 9's typede årsopgørelse for selskabers skattepligtige porteføljeaktier
forbruger nu § 5 A-resultatet før stk. 2-7. Den bruger § 23's beregnede
realisations- eller lagerprincip, holder § 8- og § 10-undtagelser uden for
stk. 1 og afskærer koncerninterne konvertible afståelsestab efter stk. 7.
Stk. 2-tab fradrages direkte, mens stk. 3-4- og stk. 5-6-tab føres i to
særskilte, årsbundne tabsbeholdninger. Typede principskift åbner de fremførte
tab for lagergevinster og kræver et sammenhængende år, samme aktiv og en
faktisk post i årsopgørelsen. § 23, stk. 6-valget kontrolleres på tværs af alle
kvalificerede poster. Afståelsessummer afledes fra de samme typede
afståelsesposter, og årsopgørelsen genberegner hver § 9-post fra dens input før
aggregering. De eksisterende atten § 9-scenarier passerer fortsat i begge
backends. Metadataindekset udstiller § 5 A som en opløst
`dependency_source`; kun fortolkningsvalget om rækkefølgen mellem samtidige
tabsbeholdninger består som advarsel.

Den ordinære personaktiegren anvender nu det samme § 5 A-resultat før §§ 13,
13 A og 14. Bruttotabet afledes fortsat af positionen og afståelsen; kalderen
kan kun levere de typede kildefakta om anvendelsesgrundlag, skatteyder,
ejertidsudbytter, præferenceudbytter og eventuelle koncernbeløb. Et
skattepligtigt tab uden disse fakta fejler lukket og kan hverken ændre
positionen eller danne et fradrag. Gevinster og tab, der allerede er skattefrie
efter § 15, kræver dem ikke. Den fokuserede kæde beviser både uændret tab,
reduktion og manglende fakta i interpreter og kompileret kode. Den kanoniske
`beregn_personskat`-kæde reducerer et bruttotab på 30.000 kr. med 12.000 kr. og
fører kun 18.000 kr. videre til § 13 og negativ aktieindkomst. Typede
beregningsfelter giver arbejdsbogen danske spørgsmål og kildespor for hele
§ 5 A-faktagrafen. Ejertidsudbytter og koncernbeløb materialiseres som to
relationelle underark i stedet for brede gentagne kolonner. En udfyldt
XLSX-rundtur med 12.000 kr. i skattefrit ejertidsudbytte giver samme fulde
Personskat-resultat som det tilsvarende JSON-input og bevarer både bruttotabet
på 30.000 kr., reduktionen på 12.000 kr. og § 13-tabet på 18.000 kr. Den
kanoniske scenariefil passerer desuden i kompileret kode. § 5 A-modulet og den
øvrige ABL-gren deler én `AktieavancebeskatningslovKildeInfo`-type, så
`runa meta --type` finder lov, undtagelse, ændringslove og vejledninger i samme
kildesweep uden en importcyklus.

Kursgevinstloven § 32 er nu en kildebundet, typet
årsopgørelse frem for et egnethedsflag leveret af kalderen. Den fordeler årets
og fremførte kontrakttab i lovens rækkefølge mellem egne gevinster i året,
egne skattepligtige nettogevinster fra tidligere år, en samlevende ægtefælles
kontraktgevinster og et samlet, typet aktiegevinstgrundlag. Grundlaget omfatter
ABL § 12-aktier og § 25-rettigheder samt § 20, stk. 2- og § 21-beviser gennem
§ 13 A-årsresultatet; ABL §§ 19 B, 19 C og 22 tilføres som typede supplerende
årsresultater. Ikke-kvalificerede klasser og unoterede
investeringsselskabsaktier udskilles, mens omsættelige investeringsbeviser anses
for optaget på et reguleret marked efter ABL § 3. § 19 D's
oplysningsbetingelse anvendes før nettogevinsten når § 32.

ABL § 13 A-tab anvendes først hos personen og derefter hos en samlevende
ægtefælle, før et kontrakttab kan bruge den resterende aktiegevinstkapacitet.
Det helt eller delvise modregningsvalg er udtrykkeligt, og kun en identitets- og
årsbundet rest føres videre. Fast-ejendomstab holdes i en særskilt saldo og
bliver efter modregning til nedsat afståelsessum eller forhøjet anskaffelsessum
uden fremførsel. Reguleret marked og MTF bevares som særskilte grundlag, så
virkningen fra 1. januar 2024 i LOV nr. 1563/2023 gælder på begge sider af
modregningen. Sytten hovedscenarier og ni aktieklassescenarier passerer i både
interpreter og kompileret kode. Personskattelovens § 4-bro forbruger nu det samlede § 32-årsresultat
og bevarer de enkelte kontraktposters kapital- eller
personlig-indkomstklassifikation. Entydige årsresultater bliver til én typet post,
mens en dokumenterbart blandet sag bliver opdelt i flere poster. Hvis en
årsfordeling krydser indkomstarter uden et tilstrækkeligt fordelingsgrundlag,
bevares resten som et udtrykkeligt uallokeret beløb og kan ikke glide ind i
kapitalindkomsten. Ni fokuserede § 4-scenarier passerer i begge backends. Den rå
enkeltkontraktbro er bevaret og mærket som før § 32-årsopgørelsen.

Personskatteloven § 13 a modtager ikke længere et løst skyldnertab. En lukket
union kan kun bære de faktiske årsresultater fra Aktieavancebeskatningsloven
§ 13 A, Kursgevinstloven § 32 eller Ejendomsavancebeskatningsloven § 6. En
scoped årssag samler de tre
fremførselsbeløb særskilt og afviser resultater fra senere indkomstår, før
gældsnedsættelsen fordeles mellem underskud, tab, negativ aktieskat og en
samlevende ægtefælles virksomhedsunderskud. Ejendomsavancebeskatningsloven
§ 6, stk. 3-5 beregner nu selv tabsafgrænsning efter §§ 8 og 9, § 4, stk. 3-
loftet, egen fortjeneste, overførsel til samlevende ægtefælle og dateret
fremførsel. Syv EBL-scenarier og seks § 13 a-invarianter passerer i både
interpreter og kompileret kode. Metadataindekset forbinder den ordrette tekst,
regelspændet og de officielle kilder og udstiller samtidig den gamle § 13 a-
henvisning til KGL § 32, stk. 3, over for den gældende fremførselsregel i
stk. 4 som en typet `warning`.

Personskatteloven § 3, stk. 2, nr. 2 har nu en lukket,
typet resultatunion for Ligningsloven §§ 8, stk. 1, 8 B, 8 K, 8 L, 8 N, 14,
stk. 1, 14 F og 30 A samt Kildeskatteloven § 25 A, stk. 3-5. De afhængige love
beregner selv beløbene, betingelserne og undtagelserne; Personskatteloven
kontrollerer derefter, at posten vedrører selvstændig erhvervsvirksomhed. Den
tidligere generiske nr. 2-kategori kan derfor ikke længere skabe et fradrag ved
at modtage et løst beløb. § 8 B bruger en lukket erhvervsstartstype, så en
endnu ukendt start ikke kan ligne et straksfradragsberettiget år. Den fokuserede
scenario-fil validerer 31 positive, begrænsende og afskærende udfald i både
interpreter og kompileret kode.

Aktieavancebeskatningsloven § 5 A, §§ 6-7, § 9, §§ 12-15, § 17, §§ 23-27, § 30,
stk. 1, § 33 A, §§ 35 G-35 K og §§ 37-40 har nu typede beregningsveje for
ordinære personaktier, næringsaktier, lageropgørelser, aktie- og tegningsretter,
medarbejderejeoverdragelser, statusskifter samt indgangs- og
fraflytterbeskatning.
Den vedvarende ordinære selskabsposition håndterer aktier med pålydende værdi,
homogene beholdninger af stykkapitalandele og dokumenterede blandede
beholdninger med begge kapitalformer.
Anskaffelses- og afståelseshændelser beregner gennemsnitlig anskaffelsessum,
delafståelser og hovedaktionærfordeling. Blandede beholdninger bevarer nominelle
kapitaldele og stykkapitaldele som særskilte domæneværdier. En eksakt rationel
kapitalvægt afleder stykkapitalens andel fra kapitalforhøjelsen og antallet af
udstedte stykkapitalandele; dermed fordeles anskaffelsessummen efter et fælles
kapitalandelsgrundlag og ikke efter rå aktieantal. En særskilt § 25-position bevarer
daterede rettighedspartier og bruger FIFO sammen med aktie for aktie-metoden;
aktionærtildelte rettigheder får 0 kr. i anskaffelsessum, og købte rettigheder
bevarer faktisk anskaffelsessum. Bortfald behandles som afståelse efter § 30.
MTF-overgangen den 1. januar 2024,
lagerprincipafgrænsningen, ligningslovens § 28-undtagelse og § 14's
oplysningsbetingelse er eksplicitte. Begge domæner føder den samme typede
§ 13 A-årsopgørelse og Personskatteloven § 4 a-bro.

`aktieavancebeskatningsloven-par17.runa` gør aktienæringens stk. 1-4 til en
scoped regelkaskade. Den skelner mellem selskaber efter § 6 og personer efter
§ 7, næringsstatus, næringsanskaffelse, koncerninterne konvertible
obligationer og tegningsretter, minimumsbeskattede investeringsbeviser samt
alle stk. 4-undtagelser. Den typede modprøve for §§ 19 A-19 C erstatter det
tidligere løse `ville_være_par17_uden_investeringsstatus`-flag. Resultatet
leverer en § 23-metodebro, den skatteyderafledte Kursgevinstloven
§ 32-kontraktrelation og Personskatteloven § 4's personlige
indkomstklassifikation for § 7-personer. Femten fokusscenarier passerer i
både interpreter og kompileret kode.

`aktieavancebeskatningsloven-par6-7.runa` gengiver den aktuelle ordlyd af §§
6-7 og afleder den relevante ABL-regelsti fra den underliggende lovs
skattepligtsgrundlag. Domænet skelner kun de to hjemmelsgrunde i § 6, person og
dødsbo i § 7 samt livsforsikringsselskabets nødvendige særstatus; det opfinder
ikke en udtømmende liste over selskaber, fonde og foreninger m.v. Resultatet
integritetskontrolleres ved komposition, og 11 fokusscenarier passerer i begge
backends.

`aktieavancebeskatningsloven-par5a.runa` gør tabsreduktionen før de almindelige
tabsregler til et selvstændigt, typet resultat. Stk. 1, nr. 1 og 2, stk. 2 og
stk. 3 opgøres særskilt, og både skatteydergrundlag, udbytteart,
koncernrelationer og personens kontrol over yder og modtager er lukkede
domænetyper. Reduktionen kan ikke overstige afståelsestabet. Virkningen fra
24. november 2010, LOV nr. 254/2011's særregel om tidligere statusskifter og
§ 22, stk. 6-undtagelsen er udtrykkelige udfald. De 20 fokusscenarier passerer
i begge backends og beviser også kompositionen før § 9's årsmodregning,
fuldstændig kobling mellem lagerafståelser og tabsbehandling samt afvisning af
både indre og ydre resultater med forfalskede beregningsfelter.

`aktieavancebeskatningsloven-par9.runa` gør stk. 1-7 til en typet
årsopgørelse med poster, § 23-principresultater og vedvarende tabspositioner.
Lager- og realisationsresultater holdes adskilt, stk. 3-4-tab kan først bruge
realisationsbeskattede gevinster, og stk. 5-6-tab kan bruge alle stk. 1-
gevinster. Ved et valideret skift til lagerprincippet bevares saldoen, men dens
anvendelsesgrundlag udvides efter lovteksten. Udelukkelserne efter §§ 8 og 10,
§ 7-personer, § 23 A-årsregulering over for stk. 7-afståelsestab, det
porteføljeomfattende stk. 6-valg, forældede fremførsler og ugyldige
principskift er særskilte udfald. De 18 fokusscenarier passerer i begge
backends. Modellen anvender den ældre og snævrere stk. 3-4-beholdning først,
når begge
tabsbeholdninger konkurrerer om samme realisationsgevinst; dette er markeret
som et fortolkningsvalg, ikke skjult som sikker lovtekst. Hver post validerer
nu sin typede § 5 A-behandling og anvender det reducerede tab før stk. 2-7;
årsresultatet bevarer både bruttotabet og reduktionen til auditsporet og
genberegner hver post fra dens input før aggregering.

Det importerede `aktieavancebeskatningsloven-par23-27.runa` vælger mellem
realisations-, lager- og tilladt anden opgørelsesmåde med lovens egne
undtagelser. Det håndterer det bindende § 23, stk. 2-valg, tvungne
lagergrene, stk. 6-valget og stk. 7's selskabsspecifikke syvårsperiode på
eksakte datoer, inklusive 2015-2024-overgangen og omstrukturering. Den årlige
lageropgørelse korrigerer for køb, salg og LL § 16 B-beløb og føder det
eksisterende ABL/Personskattelov-resultat. Afståelsessummerne afledes nu af
identificerede afståelsesposter i stedet for løse årsbeløb; den samme liste kan
derfor blive en relateret tabel i et genereret regneark og kobles entydigt til
§ 5 A-tabsbehandlingen. § 23 A's anskaffelsessumsgulv,
§ 24's principskift og næringsaktiernes overgang til anlægsbeholdning,
MTF-rettigheder erhvervet før 2024, § 26's 0-basis, handelsværdiregel og
adskilte § 7 N-beholdninger samt § 27's daterede anskaffelsessumtillæg er
eksekverbare. De fokuserede scenarier validerer nu 36 ordinære aktieudfald,
11 rettighedsudfald og 22 §§ 23-27-udfald i både interpreter og kompileret kode.

`aktieavancebeskatningsloven-par33a.runa` gør § 33 A's skattemæssige
statusskifter eksekverbare som en scoped regelkaskade. Den skelner mellem
stk. 2, nr. 1 og 2, beregner den fiktive afståelse og genanskaffelse til
handelsværdi, bevarer negativ anskaffelsessum og sender gevinst eller tab til
den udgående skattepligtige status' almindelige regel. Skift fra § 8 til en
skattepligtig status nulstiller derfor anskaffelsessummen uden at opfinde en
straksbeskatning under § 8. Stk. 3's skattefri omstruktureringer og stk. 4's
§ 33-undtagelse er særskilte typede udfald. § 24, stk. 3 modtager nu dette
resultat i stedet for et løst ja/nej-flag. Ni fokusscenarier passerer i både
interpreter og kompileret kode.

`aktieavancebeskatningsloven-par37-39.runa` bevarer det faktiske
anskaffelsestidspunkt og anvender handelsværdien som indgangsværdi efter § 37.
§ 38 modellerer ophør af dansk beskatningsret, døds- og § 44-undtagelserne,
100.000 kr.-grænsen og dens særbeholdninger, syvårsperioden og dens
aktiespecifikke undtagelser, nettogevinst, fradragsberettigede og afskårne tab
samt tegningsretsvalget. § 39 kobler den realisationsopgjorte skat til rettidig
indberetning, bistandsland, betryggende sikkerhed, videreflytning,
fristoverskridelse og tillægget på 0,4 procentpoint pr. påbegyndt måned. De 21
fokusscenarier passerer i både interpreter og kompileret kode. § 38 modtager et
typet nettobeløb fra de afhængige §§ 23-29 og 46; § 39 modtager den beregnede
skat for realisationsposterne. Dermed opfinder modulet hverken manglende
klassifikationsregler eller en skattesats, som bestemmelserne ikke selv angiver.

`aktieavancebeskatningsloven-par39a-40.runa` viderefører henstanden som en
typet, uforanderlig flerperiodetilstand. Den holder beholdningspartier,
henstandssaldo og forfaldsposter adskilt, anvender FIFO ved afståelser og
skelner mellem bruttosaldoen, allerede reserverede fordringer og den endnu
disponible saldo. Hver saldoafhængig fordring bærer nu sit indkomstår, så
årsprioriteten kan genfordele ubetalte fordringer i rækkefølgen stk. 4-tab,
afståelsesgevinster, stk. 3-tabsregulering og til sidst udlodninger og lån uden
at ændre tidligere års eller allerede betalte beløb. Regelkaskaden dækker
gevinst og tab, udenlandsk skat, udbytte, andre udlodninger og dispositioner,
lån og deres undtagelser, død, årlige oplysninger, dokumentation,
betalingsfrister, kildeskattelovens § 63 og det endelige bortfald. § 39 B
fordeler tilbageflytningens regulering af
indgangsværdier forholdsmæssigt og bruger Personskatteloven § 8 a's faktiske
grundbeløb, inklusive ægtefællers dobbelte grundbeløb. § 40 nedskriver den
disponible henstandssaldo med betalt skat uden at gå under nul. De 41
fokusscenarier passerer i både interpreter og kompileret kode og indeholder et
forløb, som beviser, at flere samtidige fordringer ikke kan reservere eller
opkræve mere end den samlede henstandssaldo, samt flerhændelsesforløb for
samme års, tidligere års og delvist betalte fordringer. Satsbaserede hændelser
accepterer kun skatteår med en kildebunden national parameterpakke, aktuelt
2024-2026; andre år afvises før opslaget i Personskatteloven § 8 a.

`aktieavancebeskatningsloven-par35g-35k.runa` gør den nye
medarbejderejeordning fra 1. januar 2026 eksekverbar som et andet, særskilt
flerperiodetilstandssystem. Valget kontrollerer fysisk overdrager, § 34-aktier,
dansk eller tilladt udenlandsk virksomhedsform, meddelelse, tilsagn og
sikkerhed. Negativ anskaffelsessum beskattes straks, mens den øvrige latente
gevinst danner en overdragerskattesaldo med selskabsskattelovens § 17-sats.
Beholdningen bruger FIFO pr. aktieparti; afståelser, årets 8 pct.-udbyttegrænse,
skattefradrag, forfaldsposter, betalinger og endeligt bortfald bevares som
adskilte typede hændelser. §§ 35 J-35 K håndterer tvangsafståelser,
værdinedgangsdispositioner, årsoplysninger, sikkerhed og hjemstedsflytninger.
Et samlet, identificeret kildeforløb genafspiller nu valget og en kronologisk
liste af typede hændelser fra overdragelsesåret til opgørelsesåret. Hver
hændelse har en stabil identifikation og en eksplicit rækkefølge inden for
indkomståret. Saldo, forfald, betaling, bortfald og ultimotilstand afledes fra
historikken; de kan ikke leveres som færdige input fra kalderen. En ugyldig
rækkefølge eller hændelse fejler lukket uden at mutere den afledte tilstand.
De 23 fokusscenarier passerer i både interpreter og kompileret kode, inklusive
virkningsgrænsen 2025/2026, tvangsmodning, oplysningssvigt, hjemstedsflytning,
straksindkomst fra negativ anskaffelsessum og et kædet forløb fra 55.000 kr.
startsaldo til endelig betaling og bortfald.

Selskabslovens § 47 tillader en kombination af kapitalandele med nominel værdi
og stykkapitalandele. Den nuværende ABL-position bærer nu det dokumenterede,
sammenlignelige kapitalandelsgrundlag i domænet og beregner anskaffelser samt
delvise og fulde afståelser på tværs af de to former. Selskabslovens § 47 og
2008/1 LSF 170's bemærkninger er knyttet til regelspændet som henholdsvis
`dependency_source` og `preparatory_work`. En hændelse med nominelle aktier og
udokumenterede stykkapitalandele afvises fortsat, når ingen af positionerne
leverer den manglende kapitalvægt; det er validering af et ufuldstændigt input,
ikke en dækningsgrænse for den lovlige kombination.
§§ 6-7 har nu en lukket, kildebundet grænse fra skattepligtsgrundlag efter
Selskabsskatteloven, Fondsbeskatningsloven, Kildeskatteloven og
Dødsboskatteloven til ABL's to regelspor. De underliggende loves fulde
personkredse hører fortsat til deres egne korpusser. De smallere ABL-grænser er
nu klassifikationerne efter §§ 19 A-20 A og 22 samt § 38's fulde
afhængige opgørelser efter §§ 23-29 og 46. Modulerne modtager juridiske
klassifikationer som typede resultater frem for rå sand/falsk-flags forklædt
som fuld dækning.

Afskrivningslovens aktuelle kilde- og regelkorpus omfatter
nu hele paragrafsekvensen §§ 1-69. § 3 kræver leveret, driftsbestemt og
driftsklart aktiv på en gyldig anskaffelsesdato. § 4 håndterer fiktivt salg og
køb ved benyttelsesændring, virksomhedsordningsoverførsel og nulværdi for en
omfattet ladestander. § 5 bærer både den almindelige saldo og selskabers
treårige udlejningsforløb. §§ 5 B-5 E dækker særskilt skibssaldo,
15/7 pct.-infrastruktursaldi samt de tidsafgrænsede 116/108 pct.-saldi og deres
sammenlægning med § 5. § 6 dækker nu både straksfradraget, salg af det
straksafskrevne aktiv, selskabers udskudte fradragsår for udlejningsaktiver,
dispensationen og § 9-henvisningen. Et negativt stk. 4-beløb bevares som et
selvstændigt fradragsudfald i stedet for at blive skjult i en positiv
indtægtsføring. §§ 7-10 dækker skade og erstatning, separat eller samlet
negativ saldo, ophør og senere salg samt 2026-grænsen på 965.800 kr. for dok- og
beddingsanlæg. §§ 11-13 dækker de blandet benyttede aktiver, salg og skade.
§§ 14-20 dækker den negative afgrænsning af bygninger, positive og negative
tilknytningsregler, installationer, anskaffelsestidspunkt, valgfrie 3/4 pct.- og
levetidsafskrivninger, valgfrit straksfradrag op til 5 pct. med vedligeholdelse
først, delvise bygninger og stopår ved salg, nedrivning eller ophørt
erhvervsbrug. § 19 bærer nu en vedvarende, valideret liste af særskilte
anskaffelsessumintervaller med faktisk og maksimalt mulig afskrivningshistorik.
Den gengiver Den juridiske vejlednings forøgelses-, reduktions- og senere
genforøgelseseksempler i både interpreter og kompileret kode. §§ 21-24
forbruger den typede historik ved salg, genvundne afskrivninger, tab, nedrivning,
skade og genopførelse. § 21 holder beregnede, faktisk foretagne og maksimalt
mulige afskrivninger adskilt; § 22 genbruger den nedskrevne værdi ved
nedrivning; § 23 reducerer både grundlag og § 19-intervaller efter skade; og §
24 viderefører grundlag og afskrivninger eller indtægtsfører genvundne
afskrivninger med lovens 5 pct.-tillæg ved fristsvigt. §§ 25-26 dækker
bygninger på lejet grund, nedlæggelses-/nedrivningsklausuler og
hjemfaldsforpligtelser med 3- og 4-pct.-satser, lejeperiodeloft, delvis anvendelse,
kontrolafskæring, den progressive hjemfaldsskala, ophørsfradrag, genvundne
afskrivninger, tabsafskæring og nedrivningsfradrag. § 27 bærer den oprindelige
udgift og foretagne afskrivninger videre ved ejer- og forpagtersuccession.
§§ 28-37 bærer en samlet, vedvarende forskudsafskrivningsposition pr.
skatteyder, virksomhed og bestillingsår. § 29-grundbeløbet reguleres gennem
Personskatteloven § 20; §§ 30-32 afleder tidsrum, anskaffelser, resterende
grundbeløb og 15/30-pct.-lofter fra historikken; § 33 fordeler
forskudsafskrivninger på de faktiske aktiver uden at tabe afrundingsresten; og
§§ 34-36 holder aktuel efterbeskatning, genåbning af tidligere indkomstår,
dødsbosuccession og myndighedsgodkendt fristforlængelse adskilt. § 37 er bevaret
som en eksplicit ophævet bestemmelse. § 38 bærer ejerens mineralforekomst,
anskaffelsessummens dokumenterede forekomstdel og tidligere afskrivninger som
en vedvarende position; årets valg kan ikke overstige hverken den dokumenterede
værdiforringelse eller restgrundlaget. § 39 modellerer lejeforhold, løbetid,
opsigelsesrisiko, nærtstående og selskabskontrol, køberet samt § 14-undtagelsen
som egne domæneobjekter. Den holder årlig afskrivning, nedrivningsfradrag,
overførsel til en erhvervet bygnings anskaffelsessum og fortjeneste/tab ved
afståelse adskilt. § 40 skelner tilsvarende mellem erhvervede immaterielle
aktiver og godtgørelser eller vederlag, mellem yder og modtager og mellem
årlig afskrivning, 5-pct.-straksfradrag og salg. LOV 749/2025 er kodet som en
eksplicit overgang: gensidigt bebyrdende aftaler før 2026 bevarer det tidligere
§ 40, stk. 7-regime, mens aftaler fra 2026 henvises til Ligningsloven § 12 B.
Den henviste § 12 B-ordning er nu implementeret fra LOV 749/2025 sammen med
den ikke-konsoliderede 2026-overgang. Modellen skelner aftaledatoens tre
regimer, de historiske saldo- og udelukkelsesregler og den nye henstand med
skat og arbejdsmarkedsbidrag. En vedvarende, dateret hændelsesposition holder
aktiv henstand adskilt fra hvert identificeret, afventende, forfaldent, betalt
eller frafaldet afdrag. Forholdsmæssige afdrag udledes kumulativt, så gentagne
realiseringer og delbetalinger ikke dobbeltopkræves. Betalingsfristen går
korrekt fra en decemberrealisering til 1./10. januar i næste kalenderår, mens
en fremtidig Opkrævningsloven-rente først kræves, når forsinkelsesrenten faktisk
skal beregnes. Modellen bærer desuden rente, rykkergebyr, misligholdelse, ophør
og reduktion af konto for opsparet overskud gennem samme hændelseslog.
Skattestyrelsens eksempel med 1.000.000 kr. goodwill, 515.000 kr. skat og
500.000 kr. kapitaliseret løbende ydelse giver 257.500 kr. skattehenstand i
både interpreter og kompileret kode. De 24 fokuserede scenarieregler skelner
desuden den samlede kapitalisering fra finansieringen af det enkelte aktiv,
kræver hjemmel i arbejdsmarkedsbidragslovens § 4 eller § 5, behandler
afståelse af retten som en typet realisation og bevarer § 2-kapitalisering,
selv om en selvejende institution afskærer stk. 3 og frem. Senere indtræden af
skattepligt reducerer den afledte saldoindgangsværdi med mellemliggende
betalinger. Den juridiske vejlednings virksomhedsordningseksempel reducerer
702.000 kr. opsparet overskud med 409.500 kr. til 292.500 kr.; reduktionen
begrænses ikke fejlagtigt til det frafaldne skattekrav på 115.500 kr.

§§ 40 A-40 D fortsætter samme kapitel med typede kvotepositioner frem for
løse beløbs-fixtures. § 40 A fordeler engangskvotens resterende
anskaffelsessum forholdsmæssigt ved anvendelse, salg og udløb. § 40 B bærer
aftaleår, udnyttelsesperiode og tidligere afskrivninger gennem et højst
syvårigt forløb og fordeler anskaffelsessum og tidligere afskrivninger på en
delvist solgt eller udløbet andel. Begge regler afleder FIFO fra lagerets
daterede kvotelots,
sætter vederlagsfri tildeling til nul og afskærer rettigheder knyttet til de
lovbestemte andelsbeviser; § 40 B afskærer desuden mælkekvoter og
sukkerroerettigheder. § 40 C modtager en typet liste af anskaffelses- og
afståelsesbevægelser i stedet for tre løse saldotal. En særskilt aktivsag
afleder bevægelserne fra daterede betalingsrettigheder, mælkekvoter og
sukkerroerettigheder, herunder 19. maj 1993-, 1. januar 2005- og 4. oktober
2006-grænserne, forholdsmæssige delafståelser, vederlagsfri tildeling,
forpagterreglen, udløb til nul og stk. 12's FIFO for et blandet mælkekvotelager.
Stk. 8-10 holder dokumenterede negative saldi, ejendomstab, 22 pct.
acontoskat, egen og ægtefælles slutskat, kontant udbetaling og fremførsel som
typede resultater. § 40 D producerer selv den
§ 40 C-anskaffelsesbevægelse, der svarer til handelsværdien ved indtræden af
dansk skattepligt eller dansk DBO-hjemsted. Dermed går § 40 A- og § 40 B-beløb
til Personskatteloven § 3, stk. 2, nr. 10, mens § 40 D kun når
kapitalindkomsten gennem § 40 C og Personskatteloven § 4, stk. 1, nr. 16.
Det brede kvotescenarie kontrollerer 27 betingelser i både interpreter og
kompileret kode, herunder delvis anvendelse, salg, udløb, afrunding, manglende
opsamling af fravalgte afskrivninger, forholdsmæssigt salg af en kvoteandel,
tidsrækkefølge, entydig FIFO, udelukkelser, tilflytningsårets saldoføring og
hele § 40 D -> § 40 C -> § 4-kæden.

Det fokuserede § 40 C-scenarie kontrollerer yderligere 30 betingelser i begge
backends: køb og vederlagsfri tildeling, gamle og nye mælkekvoter, delafståelse,
den blandede FIFO-grænse, sukkerroerettighedernes to anskaffelsesveje,
forpagterreglen, udløb, ordinær saldo og ophør samt hele stk. 8-10-kæden ind i
Kildeskattelovens slutopgørelse. Den gældende § 40 C er dermed modelleret
stykke for stykke.

§ 41 er bevaret som udtrykkeligt ophævet. § 42 bærer en vedvarende position
for ejerens eller forpagterens ombygnings- og forbedringsudgifter til
landboturisme med 20 pct.-loft, moms- og erhvervsbetingelser,
udlejningsindtægtsloft, straksafskrivning og fortjeneste/tab ved afståelse.
§ 43 holder erhvervsandelen af en fysisk engangstilslutning til et anlæg ejet
af andre, forfaldstidspunkt, årlige afskrivninger og restfradrag ved salg
adskilt. § 44 modellerer den lukkede tilskudskreds og dens to undtagelser for
fiskerfartøjers endelige ophør. §§ 44 A-44 B skelner kunst indføjet i en
bygning fra kunst, der hænges op eller opstilles, og dækker særskilt
afskrivningshistorik eller saldo, nærtståendeafskæring, skade, genopførelse,
ophør og salg. § 44 C opgør fortjeneste eller tab på leveringskontrakter og
leverede, men endnu ikke driftsklare aktiver uden at blande
forskudsafskrivninger ind i opgørelsen. §§ 45-49 normaliserer kontantværdi,
den skriftlige aktivfordeling, regulerede grundbeløb, andre afståelsesformer,
erstatningssummer og gave/arv/arveforskud. § 45 bruger en typet
`Al45DriftsmidlerOgSkibeUnderEt`-nøgle i stedet for en strengkonvention, og §
49 gør det eksplicit, når skattesuccession fortrænger både køb/salg og
værdiansættelse. §§ 50-52 modellerer handel med næringsaktiver, forsøgs- og
forskningsudgifter før erhvervsstart og tremånedersfristen for ansøgning om
dispensation. § 53 er bevaret som ophævet. §§ 54-62 og 68-69 gør de historiske
ikrafttrædelses-, overgangs-, kontantomregnings-, miljø-, udlejnings- og
territorialregler eksekverbare; §§ 63-67 er udtrykkeligt ophævede. LOV 615/2026
er lagt oven på konsolideringen, så landbrugs-, skov- og naturejendom er typede
kategorier, og 2027-virkningen i §§ 40 C og 42 kan prøves direkte. Den
fokuserede §§ 50-69-scenariofil validerer 23 grænser og overgangsudfald i begge
backends. Det samlede Afskrivningslov-indeks har 87 ankere, 84 kildehenvisninger
og ingen metadata-diagnostikker.

Personskatteloven § 3, stk. 2, nr. 10 modtager de typede kapitel 2-resultater
samt §§ 17-18-, §§ 21-27-, § 32-, § 34-, §§ 38-40 B-, §§ 42-44 C-, § 50-, § 55-,
§ 58-, § 58 A-, § 60- og § 62-resultaterne og
holder skattepligtige indtægtsføringer, afskrivninger, tab og andre fradrag
adskilt, før kun fysiske personers indtægter og selvstændige personers fradrag
føres til personlig indkomst. Det nye §§ 42-44 C-scenarie holder 150.000 kr.
indtægtsføring, 109.000 kr. afskrivninger, 10.000 kr. tab og 80.000 kr. andre
fradrag adskilt gennem hele § 3-kaskaden i begge backends. Den fokuserede
scenario-fil validerer bl.a. et
samlet kapitel 2-forløb med 78.000 kr. indtægtsføring og 96.000 kr. fradrag.
Den nye overgangskaskade afleder alle beløb fra Afskrivningslovens egne
domæner og holder 98.000 kr. indtægt adskilt fra 60.000 kr. afskrivning,
80.000 kr. tab og 25.000 kr. andet fradrag, i alt 165.000 kr. fradrag.
Den nye afståelses- og skadescenario holder tilsvarende 652.500 kr. i
genvundne afskrivninger og fristindtægt adskilt fra 100.000 kr. tab og 172.500
kr. nedrivnings- og skadefradrag. Scenarierne for §§ 25-27 validerer desuden
Skatterådets hjemfaldseksempel på 3.269 kr., 2,5 pct.-loftet ved 40 års leje,
den gennemsnitlige erhvervsandel ved nedrivning og en § 3-kaskade, der holder
200.000 kr. genvundne afskrivninger adskilt fra 225.000 kr. afskrivninger og
600.000 kr. andre fradrag. Kapitel 4-scenariet gengiver desuden Den juridiske
vejlednings fire-skibsforløb: 1.200.000 kr. oprindeligt forskudsgrundlag,
144.000/180.000/13.200 kr. i årlige forskudsafskrivninger, en § 33-fordeling på
154.643/50.557 kr. og 118.830 kr. i samlet § 34-forhøjelse. Både interpreter og
kompileret kode kontrollerer samme regelkæde. Scenariet for §§ 38-40
kontrollerer desuden 24 kildegrænser og kaskadeudfald i begge backends:
dokumenteret mineralværdiforringelse, 20-pct.- og lejeperiodelofter,
nærtståendeafskæring og § 14-undtagelse, tabs- og nedrivningsudfald,
goodwillgrundlag, korte og lange rettighedsperioder, 5-pct.-grænsen samt
adskilte yder-/modtagerposter og 2026-overgangen. De eksisterende
2026-parametre omfatter også 36.000 kr.-grænserne og forskningsfradragets
114/110 pct.-deling omkring loftet på 1.088,8 mio. kr. § 5 A-modellen begrænser
et valgt tab forholdsmæssigt, når hele den uafskrevne anskaffelsessum ellers
ville gøre saldoen negativ. Nr. 11 fører kun iværksætterkontoens fradrag til
personlig indkomst. Den kanoniske Personskat-sag fører etableringskontoens
fradrag ad den særskilte ligningsmæssige vej, så begge virkninger er
beregnelige uden at blive dobbeltklassificeret.
Etableringskontomodellen dækker også 60 pct./250.000 kr.-loftet, 5.000
kr.-minimum, § 29-forskudsafskrivning, kontoformen og beløb, der efter § 4,
stk. 2 behandles som indskud. Samme scenario-fil validerer de offentliggjorte
200.000/300.000/800.000 kr.-eksempler.

Afskrivningslovens §§ 11-13 er nu føjet til samme nr. 10-kaskade. Delvist
erhvervsmæssigt benyttede driftsmidler og skibe bærer en typet årsopgørelse med
både faktisk benyttelse, beregnet afskrivning og fradraget afskrivning. Dermed
kan § 11 beregne de særskilte 25/15/7 pct.-forløb og 2026-grænsen på 16.900 kr.,
mens § 12 genbruger den samme historik til at fordele fortjeneste eller tab ved
salg. Skattestyrelsens offentliggjorte eksempel med 47.000 erhvervskilometer ud
af 110.000 og henholdsvis 68.000 kr. i fortjeneste eller 37.000 kr. i tab giver
29.055 kr. i skattepligtig fortjeneste og 15.809 kr. i fradragsberettiget tab.
Den juridiske vejlednings særregel for anskaffelse og afhændelse i samme
indkomstår bruger i stedet det pågældende års erhvervsandel for både fortjeneste
og tab.
§ 13 genbruger både benyttelses- og afskrivningshistorikken ved skade,
reparation og forsikringsoverskud. Fortjenester fødes som særskilte § 3-
indkomstposter; afskrivninger, tab og andre fradrag fødes fortsat som nr. 10-
fradragsposter, så indtægt og fradrag ikke skjules i et nettobeløb.

Afskrivningslovens §§ 14-20 bruger særskilte domæner for bygninger,
installationer og årlige afskrivningshistorikker. § 17 holder den beregnede
afskrivning før erhvervsandel adskilt fra det faktiske fradrag for blandede
installationer. § 18 fører straksfradrag som andet fradrag, fordi stk. 4
udtrykkeligt siger, at beløbet ikke er en afskrivning. Valget kan være lavere
end 5 pct.-maksimum, hvorefter resten føres til særskilt afskrivningsgrundlag.
Accessoriske udgravninger, veje, gårdspladser, parkeringspladser og hegn
afskæres fra straksfradraget uden at miste deres § 14-status. Den fokuserede
bygningsscenario-fil validerer 15 regelkæder i både interpreter og kompileret
kode, herunder 6,25/5 pct. for nye 16/20-årige aktiver og 9,25/8 pct. for de
tilsvarende før-2023-aktiver.

Previous integration: Personskatteloven § 3, stk. 2, nr. 4-5 modtager nu typede
resultater fra Husdyrbeskatningsloven §§ 2 og 8 og Varelagerloven § 1.
Husdyrmodellen dækker normalhandelsværdi, handelsværdi efter indgående moms,
15 pct.-loftet fra 2003, særskilte forskelsbeløb for dyregrupperne, A-, B- og
C-fradrag, basisantal, restsaldo og den toårige tilbageregulering efter BEK nr.
543/1981. Ordinære reduktionsfradrag holdes adskilt fra de fulde fradrag, som
ikke tilbagereguleres. Både fradrag og § 8-tillæg føres videre til personlig
indkomst. Varelagergrenen dækker de tre opgørelsesmåder, varegrupper, indgående
moms og satsrækken 1993-1998+, herunder den aktuelle 0 pct.-virkning. Den
fokuserede scenario-fil validerer 13 betingelser og den fælles § 3-kaskade.

Previous integration: Pensionsbeskatningsloven §§ 16, 18 og 52 er nu ført gennem
en typet § 3, stk. 2, nr. 3-regelkaskade. § 16 leverer de regulerede brutto-
lofter, mens § 18 selv anvender arbejdsgiverreduktionen efter stk. 2,
tiårsfordeling, opfyldningsfradrag, selvstændiges 30 pct.-valg,
forfalds-/betalingsår, højst seks indekskontrakter og lovens fradragsafskæringer.
§ 52 kræver tilladt modtagerkreds, rent fondsformål, korrekt placering og
medarbejdervalgt bestyrelsesmedlem. § 3-bridgen modtager det eksisterende typede
§ 4 a-pensionsfradragsresultat og afskærer kun dobbeltfradrag for § 15 A-
ordninger. Den fokuserede scenario-fil validerer 16 betingelser og fører et
80.000 kr. § 15 A-fradrag til 20.000 kr. aktieindkomst og 60.000 kr. personlig
indkomst uden overlap.

Previous integration: Ligningsloven § 9 B er nu en kildebaseret regelkaskade for
60-dages-reglen, ny periode efter 60 sammenhængende arbejdsdage, stk. 3's
formodning og kørselsregnskabspålæg, kørsel mellem eller inden for
arbejdspladser, Skatterådets 2026-kilometersatser, kontrol- og bilagskrav,
aconto/fast godtgørelse, bruttolønsmodregning, firmabil, godtgørelse over
satsen, § 9 C-henvisning og kundeopsøgende kørsel for flere arbejdsgivere.
Selvstændige og andre ikke-lønmodtagere kan vælge faktiske udgifter eller
kilometersatser; den særlige lønmodtagergren bruger satserne og medregner en
eventuel godtgørelse i indkomsten. Ligningsloven § 8 O har en tidsafgrænset
ydelseskreds før og fra 2026, faktisk tilbagebetaling, resterende tidligere
beskattet bruttobeløb og et eksplicit dobbeltfradragsværn. De to resultater
føres gennem `Par3Stk2Nr8Sag` og `Par3Stk2Nr9Sag` til Personskatteloven § 3.
Den fokuserede scenario-fil validerer 20 betingelser og en samlet regelkaskade,
hvor 3.410 kr. § 9 B-fradrag og 30.000 kr. § 8 O-fradrag reducerer 100.000 kr.
personlig bruttoindkomst til 66.590 kr.

Earlier integration: Personskatteloven § 3 modtager nu både typede
henlæggelsesresultater og senere indtægtsføringsresultater fra
Virksomhedsskatteloven §§ 22 b og 22 d. En fælles, ordningsafgrænset
regelmodel håndterer ældste henlæggelse først, frivillig indtægtsføring,
tiårsfristen, § 22 b-underskud, ophør, overgang til virksomhedsordningen,
skattepligtsophør og § 22 d-konkurs. Den tilsvarende udligningsskat fordeles
til egen slutskat, eventuelt ægtefællens slutskat og fremførsel efter
successionsreglen; almindelige overskydende beløb udbetales kontant.
Personskatteloven § 3, stk. 1 medregner den indtægtsførte bruttohenlæggelse i
personlig indkomst, mens § 3, stk. 2, nr. 7 fortsat bærer fradraget ved selve
henlæggelsen. Lovtekst og regler er kildeankret til både Retsinformation og de
relevante afsnit i Den juridiske vejledning gennem gentagne, typede
`source`-referencer.

Ligningsloven § 8 M er nu implementeret som en typet,
kildeindekseret regelkaskade for arbejdsmarkedsbidrag efter AM-bidragslovens
§ 2, stk. 1, nr. 1-2, og §§ 4-5, obligatoriske udenlandske sociale bidrag for
fuldt skattepligtige under EU-regler eller mellemfolkelig aftale samt
udenlandske arbejdsgiverbidrag for begrænset skattepligtige. Resultatet føres
gennem en `Par3Stk2Nr6Sag` til Personskatteloven § 3's personlige
indkomstfradrag, så § 3 ikke længere accepterer denne post som et ubetinget råt
beløb. Scenarierne dækker også indeholdelsespligt, DBO-hjemsted og kravet om en
aftale, der lægger arbejdsgiverbidraget på lønmodtageren.

Kommuneskatteloven § 16 a er nu implementeret fra LOV nr.
720 af 20/06/2025 som en scoped regelkaskade. Den opgør den enkelte
selvbudgetterende kommunes stk. 1-beløb, den nationale ramme på 1,5 mia. kr.,
den årlige regulering fra 2027 med et loft på 5 pct., det samlede
korrektionsbeløb og kommunens forholdsmæssige andel. Andelen føres tilbage til
§ 16, stk. 2 og fratrækkes efterreguleringen før januar/februar/marts-raterne.
LOV 720/2025's § 2 og virkningsbestemmelsen fra tilskudsåret 2026 er bevaret
ordret i en `--@source`-blok, hvis regelsymboler kan aflæses med `runa meta`.
Den officielle metadata-refresh validerer nu 41 kilder uden drift.

Arbejdsmarkedsbidragsloven § 3 er nu en typet
`ArbejdsmarkedsbidragPar3UdelukkelseResultat` med særskilte beløb for nr. 1-5.
Det almindelige lønmodtagergrundlag bærer resultatet med sig, og det hidtidige
samlede udelukkelsesbeløb delegerer til dets kildeformede sum. Den almindelige
lønmodtagerberegner bærer også et typet `LønmodtagerPensionsfradrag`-domæneobjekt
og udleder standardfradragene gennem
det samlede Ligningsloven §§ 9 J/9 K/9 L-resultat. 2026-fixturen for København,
600.000 kr. løn og 100.000 kr. § 18-pensionsindbetaling udstiller nu
63.300 kr. beskæftigelsesfradrag, 3.100 kr. jobfradrag, 10.536 kr. ekstra
pensionsfradrag, 76.936 kr. samlede standardfradrag, 475.064 kr. almindelig
skattepligtig indkomst og 206.262 kr. samlet skat inkl. AM efter
personfradrag. Eksisterende lønmodtagerscenarier bruger en eksplicit
ingen-ekstra-pensionsfradrag-default. Kildeskatteloven § 48, stk. 11 har nu en
selvstændig 40 pct. A-skat-regel uden skattekortfradrag for indbetalinger fra
pensionsinstitutter efter § 46, stk. 6, med konkret `.scenario.runa`-dækning.
Arbejdsmarkedsbidragsloven § 2-lønmodtagersnittet er også udvidet fra anonym
felt-sum til kildeformede regelposter for nr. 1-6 samt stk. 3 med et samlet
grundlagsresultat og scenariedækning, og § 2, stk. 2 har nu et typet
naturalia-resultat for fri kost/logi, fri bil, fri telefon, sommerbolig,
lystbåd, helårsbolig, aktie-/tegnings-/køberetter og arbejdsgiverbetalt
sundhedsbehandling. Resultatet kræver både en nævnt naturalia-art og et stk. 1
vederlag, før den skattepligtige værdi føres ind i AM-grundlaget. § 3's fem
udelukkelser er tilsvarende eksponeret som navngivne regelposter og samlet i et
typet resultat, der følger med § 2-grundlagsresultatet.
Kommuneskatteloven dækker nu også første
afregningsslice for §§ 7, 15 og 16: kommunens valg mellem eget skøn og
statsgaranteret grundlag, § 15's månedlige tolvtedel og § 16, stk. 2's
efterreguleringsbeløb med januar/februar/marts-tredjedele tre år senere samt
stk. 3's 3 pct.-tærskel og diskontoafledte tillæg. § 16, stk. 4 dækker nu også
kommunens andel af virksomhedsskat, konjunkturudligningsskat,
indkomstudligning og afskrivningslovens § 40 C-acontoskat med særskilt
stat-til-kommune, kommune-til-stat og nettoafregning. § 16 a dækker desuden
selvbudgetteringskorrektionen fra 2026, herunder national ramme, 5 pct.-loft
fra 2027, positiv forholdsmæssig kommunefordeling og fradrag i § 16, stk. 2's
efterregulering. §§ 2-3 dækker nu også
skattekommunevalg pr. 5. september, institution-/skibsundtagelser,
Københavner-beregning ved KSL § 1, nr. 4-udrejse og forholdsmæssig
tilflytningskommuneandel ved fraflytning fra skattekommunen. § 7, stk. 4 har
nu en statsgaranti-beregning fra udskrivningsgrundlaget to år før
beregningsåret plus fremskrivningsprocenten og de tilhørende ministerielle
meddelelses-/tilslutningsposter. Kildeskatteloven § 62 A har nu et
datoeksakt udbetalingsfrist-resultat for ændrede årsopgørelser, hvor nedsat
restskat eller ny/yderligere overskydende skat skal udbetales inden udgangen af
måneden efter udskrivningsdatoen. Opkrævningsloven § 7 har nu også en
source-ankeret Nationalbank/Statbank DNRUUPI-inputvej for 2025 og 2026, hvor
juli/august/september-kassekreditrenter i milliprocent afrundes til
basispoint, føres gennem lovens gennemsnit/nedrunding/division, og matches
mod Skattestyrelsens offentliggjorte SKM-satser. Kommuneskatteloven § 5,
stk. 3 er nu også en kildeankeret delårsregel, hvor den kommunale
indkomstskat bruger Personskatteloven § 14's tilsvarende helårsomregnings- og
stk. 2-valgmekanik i et typed `KommunalPar5DelårResultat`.

Latest mainline slices: § 1/§ 2 now compose ordinary taxable income as an
amount-level result from personal income, capital income, excluded share income,
excluded CFC income and ligningsmæssige fradrag, and the wage-earner calculator
delegates its taxable-income base to that result; § 3, stk. 2, nr. 1 now has
amount-level personal-income deduction filtering for self-employed business
expenses with the statutory § 4, stk. 1, nr. 1/2/7/8 and Ligningsloven
§§ 9 G/13 carve-outs; § 3, stk. 2, nr. 6 now consumes the complete typed
Ligningsloven § 8 M result for AM contributions, foreign mandatory social
contributions and limited-taxpayer foreign employer contributions; § 3,
stk. 2, nr. 7 now consumes typed Virksomhedsskatteloven § 22 b/§ 22 d new
reserve results instead of a raw amount, including reserve tax, bound-account
and § 20-regulated limit calculations; § 4,
stk. 1, nr. 1 now consumes typed Ligningsloven
§ 6 and § 6 A deduction results together with ordinary interest income and
interest expenses, including the § 6 under-100-kr. lapse, stk. 3 reduction,
stk. 5 debtor-day split, stk. 6 Kursgevinstloven overlap block and § 4,
stk. 3 personal-income reclassification posture; § 4, stk. 1, nr. 2 now
consumes a typed Kursgevinstloven result for ordinary personal claims, selected
debt cases and basic financial contracts, including the § 14/§ 23 2.000 kr.
threshold, § 14 stk. 2/§ 15/§ 18 loss blocks, § 17 fordringstab posture,
§ 32 contract-loss limitation and § 4 stk. 3 personal-income reclassification
posture; § 4, stk. 1, nr. 3 and nr. 3 a now consume typed
Virksomhedsskatteloven § 7, § 22 a, § 22 c and § 23 a results for business
capital return, including positive/period-proportional § 7 return capped by
taxable surplus, § 22 a election and stk. 3 ceiling, § 22 c acquisition
conditions and proportional ownership period, and the § 23 a personal-income
election reducing the capital-income amount, while § 4, stk. 1, nr. 3's
transfer deadline now flows through a typed Skattekontrolloven §§ 10/11/13
result instead of a raw boolean; § 4, stk. 1, nr. 5 a now consumes
a typed Selskabsskatteloven § 1, stk. 1, nr. 6 result for membership
certificates in taxable associations etc., preserving the LBK 279/2025 source
line, the investment-association carve-out and the § 3/fondsbeskatningsloven
exclusions before any amount enters capital income; § 4, stk. 1, nr. 5 b now
uses typed statutory financial-intermediary categories for banks, mortgage
credit institutions, investment firms, investment-management companies,
alternative-investment-fund managers, financial advisers and investment
advisers, while keeping the ABL § 19 C/§ 22 asset boundary and § 4, stk. 6
personal-income reclassification; § 4, stk. 1, nr. 7 now consumes a typed LL § 8, stk. 3 result for
running loan provisions/premiums, running guarantee premiums and one-off
provisions/premiums when the loan/guarantee period is under two years, with the
deductible amount flowing as negative capital income; § 4, stk. 1, nr. 8 now
consumes a typed Virksomhedsskatteloven § 11 result where stk. 1 negative
indskudskonto correction is capped by negative afkastgrundlag and net
financing costs, stk. 2 transfers/indskud are capped by the indskud, and the
same correction is added to personal income while deducted from capital income;
§ 4, stk. 1, nr. 9 and
stk. 9 now derive passive
self-employed business capital-income treatment from owner-count thresholds,
the LL § 8 K personal-owner branch, substantial-participation exclusion, and
LL § 8 P renewable-energy owner exclusion; § 4, stk. 1, nr. 10 now consumes a
typed LL § 14 A result for stk. 1 borrower payments to the named mortgage/farm
financing institutions, flowing as negative capital income only in the
payment year while LL § 14 A stk. 2 fund payouts stay outside § 4 nr. 10;
§ 4, stk. 1, nr. 11 and stk. 8 now
derive leasing income from depreciable operating assets and ships through the
substantial-participation condition and Skatterådet's pre-19 May 1993
permission carve-out; § 4, stk. 1, nr. 17 now derives tenant/shareholder
subletting and letting surplus under LL § 15 Q stk. 1/3 as positive
capital-income surplus while excluding owners/others and non-LL15Q cases;
§ 4, stk. 1, nr. 12 now consumes a typed LL § 5 C result for compensation
for accrued/credited interest and § 8, stk. 3 provision/præmie amounts,
including the § 5, stk. 5 carve-out and stk. 3 double-tax-treaty deduction
block; § 4, stk. 1, nr. 13 now consumes a typed
Pensionsbeskatningsloven § 53 A result for taxable pension-return capital
income, including PAL-method return, alternative capital-value return, taxable
share allocation, negative-return carry-forward and stk. 4 exclusion posture;
§ 4, stk. 1, nr. 14 now consumes a typed Ejendomsavancebeskatningsloven result
for taxable real-property gains, including ordinary disposals, deemed disposals
for insurance/compensation and gifts/advances, the basic § 4 gain formula,
§ 4, stk. 8 artistic-decoration exclusions, næring exclusion and § 11
expropriation-style exemptions;
§ 4, stk. 1, nr. 15 now consumes a typed LL § 12 B result for running
payment saldo taxation and deductions under stk. 4-7 and stk. 9, including
negative-saldo years, later-year payments, termination balances, right
assignments, obligation transfers, acquisition-cost adjustments and statutory
application/exclusion posture;
§ 4, stk. 1, nr. 16 now consumes a typed
Afskrivningsloven § 40 C result for payment-right/milk-quota/sugar-beet
delivery-right saldo treatment, including positive-saldo non-deduction,
negative-saldo income recognition and final-year gain/loss; and the LL § 15 P dependency now calculates long-term private-home letting
bundfradrag from 2/3 annual rent/boligafgift or 1 1/3 pct. property value,
the 24.000 kr. owner minimum, the four-month condition, actual-expense branch
and later-method lock, while LL § 15 Q now calculates the regulated low/high
bundfradrag, stk. 4 proportional coordination from a typed LL § 15 P result,
rounded 40 pct. deduction on excess rent, actual-expense branch, and resulting
surplus before § 4 consumes it; the 2026 § 7/§ 7 a/§ 8 reform
parameters for mellemskat, topskat and toptopskat now carry LOV nr. 482/2024
source provenance and derive statutory 2010-level thresholds through § 20
regulation, with § 7 a topskat and § 8 toptopskat exposed as personal-income
amount result objects; § 6 bundskat rates now carry the
2021 base text and the 2022/2023/2024 amendment source chain as a typed
rate result; § 7 spouse positive-net-capital tax now has an
executable allocation layer for stk. 10-11 and a stk. 12 equal-basis tie-break
rule; § 7 a now has post-level amount rules for included and excluded
pension-like payments; § 13, stk. 2 now has amount-level spouse transfer where
remaining deficit is first deducted from the spouse's taxable income and then
converted to tax value against the spouse's §§ 6, 7, 7 a, § 8 and § 8 a, stk. 2
tax basket, wired through the wage-earner calculator, and the spouse's own
prior-year deficits now have amount-level priority over the taxpayer's
transferred deficit in both spouse-income deduction and spouse-tax offset paths;
§ 13, stk. 4 now has
amount-level spouse and carry-forward offset ordering for negative personal
income; § 13 a now has amount-level debt-settlement reduction of carried
deficits, typed ABL/KGL/EBL loss results, negative share-income tax and a
cohabiting spouse's business deficit, while future-year loss results are
rejected before reduction; § 8 a, stk. 2
high-layer share-income tax now flows through the wage-earner § 5/§ 9
state-tax path, § 8 a, stk. 6 now has a pair-level both-negative spouse
share-income threshold allocation, and § 8 a negative share-income annual
settlement now offsets the taxpayer's slutskat, spouse slutskat, and then
carries the remainder forward; § 9
now has amount-level state-personfradrag reduction ordering for the split
§ 8/sundhedsbidrag and § 6/bundskat tax values plus non-state § 8 c,
municipal-tax and church-tax personfradrag reductions, wired through the wage-earner
calculator; § 10, stk. 3 now has amount-level spouse transfer of unused
personfradrag state-tax value into the receiving spouse's § 9 state-tax basket,
and the public wage-earner model now delegates to `LønmodtagerBeregningSag` so
ordinary fixtures and special tax postures both use the same scoped § 10
eligibility rules; § 11 negative net-capital relief now runs through the
ordinary wage-earner calculator as a named `LønmodtagerPar11NedslagResultat`,
with the public input carrying net capital income while the model derives
positive and negative capital views for the relevant statutory branches;
§ 14 now has a reusable statutory skatteberegning result that chooses between
stk. 1/stk. 3 helårsomregning and stk. 2 period reduction, and the partial-year
wage-earner path delegates its final § 14 amount to that object;
§ 10 now reflects LOV 1564/2023's removal of the separate under-18 basis from
income year 2023 onward, and § 4, stk. 1, nr. 6 now consumes a typed
Ejendomsskatteloven § 3 result, preserving the LOV 679/2023 move from
Ejendomsværdiskatteloven to Ejendomsskatteloven and LOV 615/2026's 2027
category renumbering, with covered property surplus/deficit flowing into
capital income and excluded or commercially rented categories staying out;
§ 4, stk. 7 and § 4 a, stk. 2 now consume a typed Ligningsloven § 7 N result
for medarbejderinvesteringsselskab shares, where the LL § 7 N contribution cap
and company boundary stay in the dependency law while covered payouts/gains are
reclassified to personal income and kept out of share income, and covered losses
remain negative share-income posts;
§ 26, stk. 7 now composes § 7 spouse capital-threshold and
capital-tax allocation rules into the transition-compensation nr. 3 amount, and
§ 26 now has an annual compensation-settlement result that composes
source-derived yearly parameters with the statutory tax-offset order plus
pair-level stk. 4 spouse difference results, pair-level stk. 5 and stk. 8
net-capital offset results, and a pair-level annual path where stk. 6
bundfradrag transfer feeds the nr. 2 line item; § 27 now has a typed
ministerial-delegation result for implementation and administration authority,
and § 28 now has a typed territorial-scope result with explicit Faroe Islands
and Greenland exclusion hjemmel;
§ 8 c's 2023-2026 published limited-taxpayer rate now has a
`Par8cSatsResultat` result that keeps the statutory rounded-down municipal
average method, the Skatteministeriet source posture, and the applied
basispoint rate together; § 8 b's CFC tax rate now delegates through a
`SelskabsskattelovPar17Stk1SatsResultat` that keeps the tracked
Selskabsskatteloven source line for 2024 and 2025+, ordinary 22 pct.
selskabsskat, 3 percentage-point kulbrinte supplement, and applied CFC rate
together; § 4 b now consumes typed Ligningsloven § 16 H and § 16 I, stk. 6-7
CFC dependency results, including § 16 H control, low-tax/CFC-share conditions,
EU/EØS real-establishment exemption, ownership/period share, carried-loss
proportion and stk. 10 cap, plus § 16 I controlled-company merafkast with
negative-merafkast carry-forward.

Ligningsloven § 9 C/§ 9 D dependency coverage now includes ordinary
befordringsfradrag with 2025/2026 rates, low-income supplement, bridge
passages, documented special transport, the 2026 SU outer-area branch and the
§ 9 D disability/chronic-illness displacement route. § 9 D now models the
source-backed normal-cost/factual-cost formula, including the 2026
Skatterådet normalfradrag rate for own transport, the business-driving rate
path for factual own-car expenses, and Den juridiske vejledning's 6.000 km and
25.000 km examples. The § 9 D workplace route now also preserves documented
bridge-passage deductions under § 9 C, stk. 9 while keeping the ordinary
§ 9 C, stk. 1-8 distance deduction displaced. Remaining § 9 D dependency work
is broader rate-table coverage across years and transport modalities, not the
2026 own-car examples or workplace bridge carve-out.

Distance to full implementation: the Personskatteloven corpus is broad and the
ordinary wage-earner/slutopgørelse path is already calculation useful.
Afskrivningsloven now has a contiguous executable source corpus for §§ 1-69,
including repealed and transitional provisions, and § 3, stk. 2, nr. 2-11 now
have typed amount paths for their principal named dependencies. Nr. 3-11 feed
one closed aggregate that carries both deductions and the personal-income
additions from nr. 4 and nr. 10 into the canonical § 3 calculation; generic
tagged amounts cannot enter that calculation. The remaining full-corpus work
is in Personskatteloven's other posture-only clauses, dependent statutes,
annual parameters and edge cases that still need amount-level, source-backed
rules. This is well past the structural phase, but it is not yet the complete
Danish income-tax system.

## Source Status

Primary prompt source:

- Retsinformation: `https://www.retsinformation.dk/eli/lta/2019/799`
- XML endpoint checked: `https://www.retsinformation.dk/eli/lta/2019/799/dan/xml`
- Title: `Bekendtgørelse af lov om indkomstskat for personer m.v. (personskatteloven)`
- XML status on 2026-07-30: `Historic`
- XML end date observed on 2026-07-30: `2026-06-23`
- Historic mark in XML: `2021-06-16`

Current working source:

- Retsinformation: `https://www.retsinformation.dk/eli/lta/2021/1284`
- XML endpoint checked: `https://www.retsinformation.dk/eli/lta/2021/1284/dan/xml`
- Title: `Bekendtgørelse af lov om indkomstskat for personer m.v. (personskatteloven)`
- XML status on 2026-07-30: `Valid`
- Signed: `2021-06-14`
- In force from: `2021-06-16`
- XML end date observed on 2026-07-30: `2026-07-29`
- Tracked amendment sources now include `2022/252`, `2023/610`,
  `2023/1564`, `2024/108`, `2024/482`, `2024/1691` and `2026/615`.

Current source-refresh finding:

- The tracked Retsinformation XML sources were re-fetched on 2026-07-30.
- The official XML `Status` fields remained unchanged: the working/dependency
  sources still report `Valid`, while `2019/799` reports `Historic`.
- Every tracked `Valid` source now has an XML `EndDate` horizon before
  2026-07-30, so `source-status.runa` distinguishes formal legal validity from
  current-day automation freshness.
- `AktuelSkatteberegning` still accepts formally valid sources; the new
  `DagsaktuelAutomatiskBeregning` purpose rejects sources whose metadata horizon
  does not cover `20260730`.
- `scripts/refresh-danish-tax-source-status.py --today 20260730 --fail-on-drift`
  fetches official XML for every `Retskilde(...)` record and reports semantic
  drift between Retsinformation and the encoded source model. On 2026-07-30 it
  checked 43 records with 0 drift and 0 fetch/parse errors.

Current Personskatteloven amendment sources:

- Bundskat 2022 amendment:
  `https://www.retsinformation.dk/eli/lta/2022/252`
  - XML status on 2026-07-18: `Valid`
  - § 1, nr. 1 replaces § 6, stk. 2 with 12,09 pct. and 4,09 pct. for
    income year 2022 and later.
- Bundskat 2023 amendment:
  `https://www.retsinformation.dk/eli/lta/2023/610`
  - XML status on 2026-07-18: `Valid`
  - § 1, nr. 1 replaces § 6, stk. 2 with 12,06 pct. and 4,06 pct. for
    income year 2023 and later.
- Personfradrag under 18:
  `https://www.retsinformation.dk/eli/lta/2023/1564`
  - XML status on 2026-07-18: `Valid`
  - § 1, nr. 2 removes the separate § 10, stk. 2 under-18 basis and inserts a
    single 39.350 kr. 2010-level basis; the rule model keeps the old basis only
    for pre-2023 historical queries.
- Bundskat 2024 amendment:
  `https://www.retsinformation.dk/eli/lta/2024/108`
  - XML status on 2026-07-18: `Valid`
  - § 1, nr. 1 replaces § 6, stk. 2 with 12,01 pct. and 4,01 pct. for
    income year 2024 and later.
- Person-tax reform amendment:
  `https://www.retsinformation.dk/eli/lta/2024/482`
  - XML status on 2026-07-18: `Valid`
  - § 1, nr. 14 repeals Personskatteloven § 13, stk. 5, 4. pkt.
  - § 8, stk. 4 gives § 1 effect from income year 2026.
- Iværksætterpakken amendment:
  `https://www.retsinformation.dk/eli/lta/2024/1691`
  - XML status on 2026-07-18: `Valid`
  - § 4 updates § 8 a share-income thresholds for income years 2025-2027 and
    later.
- Property-tax transition amendment:
  `https://www.retsinformation.dk/eli/lta/2023/679`
  - XML status on 2026-07-18: `Valid`
  - § 12 changes § 4, stk. 1, nr. 6 from the historic
    Ejendomsværdiskatteloven § 4 categories to Ejendomsskatteloven § 3,
    stk. 1, nr. 1-5, 9 and 10, and stk. 2.
  - § 45, stk. 3 brings §§ 10-13, including the Personskatteloven § 12
    amendment, into force on 2024-01-01.
- Property-category amendment:
  `https://www.retsinformation.dk/eli/lta/2026/615`
  - XML status on 2026-07-18: `Valid`
  - § 12 changes § 4, stk. 1, nr. 6's Ejendomsskatteloven § 3 reference from
    nr. 1-5, 9 and 10 to nr. 1-4, 8 and 9.
  - § 16, stk. 5 gives the § 12 change effect from income year 2027.

Current § 3, stk. 2, nr. 2 dependency sources:

- Ligningsloven:
  `https://www.retsinformation.dk/eli/lta/2025/1500`
  - XML rechecked on 2026-07-30; the current official print was last checked on
    2026-07-18. Retsinformation still marks LBK nr. 1500/2025 as current; the
    print lists amendments through LOV nr. 1775 of 29/12/2025.
  - §§ 8, stk. 1-4, 8 B, 8 K, 8 L, 8 N, 14, 14 F and 30 A now have
    source-linked typed result objects for sales and representation expenses,
    research and raw-material exploration, planting, Landsbyggefonden
    payments, employment expenses, property-related charges, employee funds
    and treatment/cessation expenses.
  - The implementation preserves the relevant timing rules, elections,
    percentage limits, regulated caps, asset-law referrals, employee and
    property exceptions, medical-documentation requirements and explicit
    non-deduction outcomes.
- Kildeskatteloven:
  `https://www.retsinformation.dk/eli/lta/2024/460`
  - XML rechecked on 2026-07-30; the current official print was last checked on
    2026-07-18. Retsinformation still marks LBK nr. 460/2024 as current; the
    print incorporates amendments through LOV nr. 615 of 30/06/2026, and § 25
    A, stk. 3-8 is unchanged.
  - § 25 A, stk. 3-8 now supplies the typed spouse-transfer result consumed by
    Personskatteloven: corrected business profit, the 50 pct. limit, the
    § 20-regulated 2010-level cap, the work-effort ceiling, cohabitation,
    salary-agreement and equal-participation exclusions, mirrored recipient
    income and operating-spouse reductions.

Current § 3, stk. 2, nr. 4-5 dependency sources:

- Husdyrbeskatningsloven:
  `https://www.retsinformation.dk/eli/lta/2025/1099`
  - XML status on 2026-07-18: `Valid`.
  - § 2 supplies the current valuation and 15 pct. write-down rules; § 8
    supplies the historical difference-amount transition and taxable
    adjustment consequences.
- Difference-amount regulation:
  `https://www.retsinformation.dk/eli/lta/1981/543`
  - XML status on 2026-07-18: `Valid`.
  - §§ 1-5 supply the A-, B- and C-deduction ordering, group allocation,
    basis-count ladder, full close-out and recapture mechanics.
- Varelagerloven:
  `https://www.retsinformation.dk/eli/lta/2025/1088`
  - XML status on 2026-07-18: `Valid`.
  - § 1 supplies the valuation methods, VAT exclusion, eligible inventory
    groups and the historical rate schedule ending at 0 pct. from 1998.

Current Selskabsskatteloven dependency source:

- Historic Selskabsskatteloven source:
  `https://www.retsinformation.dk/eli/lta/2022/1241`
  - XML status on 2026-07-18: `Historic`
  - Used for the 2024 § 8 b rate path that predated LBK nr. 279/2025.
- Current Selskabsskatteloven source:
  `https://www.retsinformation.dk/eli/lta/2025/279`
  - XML status on 2026-07-18: `Valid`
  - § 1, stk. 1, nr. 6 supplies the typed § 4, stk. 1, nr. 5 a dependency for
    taxable associations etc., including the § 3 and fondsbeskatningsloven
    exclusions and the investment-association carve-out consumed by
    Personskatteloven.
  - § 17, stk. 1 sets the ordinary selskabsskat rate at 22 pct. and the
    kulbrinte supplement at 3 percentage points; Personskatteloven § 8 b uses
    the ordinary 22 pct. rate for CFC income.

Current § 4 b dependency source:

- Ligningsloven:
  `https://www.retsinformation.dk/eli/lta/2025/1500`
  - XML status on 2026-07-18: `Valid`
  - §§ 16 H and 16 I, stk. 6-7 are modeled as the Personskatteloven § 4 b
    dependency for source-derived CFC income amounts.

Current § 4 and § 13 amendment/dependency sources:

- Kursgevinstloven:
  `https://www.retsinformation.dk/eli/lta/2025/1176`
  - XML status on 2026-07-18: `Valid`
  - §§ 1, 12-15, 17-18, 19-21, 23, 25-26 and 29-33 are modeled as the first
    Personskatteloven § 4, stk. 1, nr. 2 dependency slice for taxable gains and
    deductible losses on ordinary personal claims, selected debt cases and basic
    financial contracts, including thresholding, statutory loss blocks, debt
    forgiveness, foreign-currency debt and contract-loss limitation.
  - §§ 1, 12, 14, 17, 25 and 26 also anchor the typed seller-note lifecycle
    used with EBL § 6 D. The model releases cash-value basis proportionally on
    repayments and partial disposals, releases the exact residual basis on the
    final disposition, applies the annual § 14 threshold and keeps § 17
    business-claim loss treatment explicit.
  - Official guidance for valuation, the course-80 repayment example and the
    separate EBL/KGL treatment of disposal or extraordinary redemption:
    `https://info.skat.dk/data.aspx?oid=2292757`
  - `kursgevinstloven-par32.runa` contains the complete current § 32 text and a
    typed annual ledger for stk. 1-5. Its share-gain basis covers ABL § 12 and
    § 25, § 20, stk. 2, § 21, §§ 19 B-19 C and § 22 through typed annual
    results. It applies ABL § 3, § 19 D and § 13 A before KGL contract losses,
    excludes nonqualifying classes, and preserves the regulated-market/MTF
    distinction. Allocation order, taxpayer election, spouse transfer, carry
    continuity, real-estate basis adjustments, the pre-2010 transition and
    both sides of the 2024 MTF transition are covered by 26 interpreted and
    compiled scenarios.
  - MTF amendment and effective date:
    `https://www.retsinformation.dk/eli/lta/2023/1563`, § 4, nr. 1, and § 8,
    stk. 1.
  - Official guidance for the explicit full/partial share-offset election and
    loss-priority choice:
    `https://info.skat.dk/data.aspx?oid=1946050`
- Aktieavancebeskatningsloven:
  `https://www.retsinformation.dk/eli/lta/2025/1098`
  - Medarbejderejeændringen og dens virkning fra 1. januar 2026:
    `https://www.retsinformation.dk/eli/lta/2025/1755`, § 2 og § 8, stk. 1.
  - XML status on 2026-07-30: `Valid`
  - `aktieavancebeskatningsloven-par6-7.runa` contains the exact current §§ 6-7
    text and derives one integrity-checked result from the liability ground
    supplied by Selskabsskatteloven, Fondsbeskatningsloven, Kildeskatteloven or
    Dødsboskatteloven. § 17, § 23 and § 9 consume that result instead of raw
    § 6/§ 7 labels. Eleven focused scenarios pass interpreted and compiled
    execution.
  - `aktieavancebeskatningsloven-par5a.runa` contains the exact current § 5 A
    text, the § 22, stk. 6 exclusion and LOV nr. 254/2011 § 14, stk. 5 and 11.
    Its typed result calculates each reduction component, caps the reduction at
    the disposal loss and feeds § 9 before annual loss use. Twenty focused
    scenarios pass interpreted and compiled execution, including one-to-one
    lager-disposal treatment and rejection of caller-constructed inner and
    outer results whose calculated fields do not match their inputs.
  - Original § 5 A amendment and transition:
    `https://www.retsinformation.dk/eli/lta/2011/254`, § 1, nr. 7, and § 14,
    stk. 5 and 11.
  - Official guidance for § 5 A and its ordering before § 9:
    `https://info.skat.dk/data.aspx?oid=1950044` and
    `https://info.skat.dk/data.aspx?oid=1946340`.
  - `aktieavancebeskatningsloven-par9.runa` implements the complete current
    § 9 text as an annual ledger with direct lager losses and two dated
    realization-loss positions. Each post now consumes a validated § 5 A
    treatment before stk. 2-7. Its 18 focused scenarios continue to pass
    interpreted and compiled execution.
  - §§ 12-15, § 24, stk. 1-2, § 25, § 26, stk. 1-5, and § 30 supply the
    ordinary personal-share and rights paths for homogeneous holdings with or
    without nominal value: realization, average/FIFO basis, partial disposals,
    main-shareholder market-value allocation, listed/unlisted loss treatment,
    the § 14 information condition, the § 15 housing-right exemption, and
    § 13 A spouse transfer/carry-forward into Personskatteloven § 4 a.
  - `aktieavancebeskatningsloven-par23-27.runa` implements § 23, stk. 2-9,
    § 23 A, § 24, stk. 3-5, the pre-2024 MTF-rights transition, § 26's employee
    basis/deemed-price and separated § 7 N holdings, and § 27's basis addition.
    Its 22 focused scenarios pass interpreted and compiled execution and route
    applicable lager and later realization results into the existing PSL
    category bridge.
  - `aktieavancebeskatningsloven-par33a.runa` implements § 33 A's exact
    taxable-to-exempt and exempt-to-taxable status sets, deemed disposal and
    reacquisition at market value, ordinary-rule routing, tax-free transaction
    override and § 33 exclusion. Its 9 focused scenarios pass interpreted and
    compiled execution, and § 24, stk. 3 consumes the typed result.
  - `aktieavancebeskatningsloven-par37-39.runa` implements § 37 entry basis,
    § 38 exit-tax scope, portfolio threshold, seven-year exceptions, gain/loss
    netting and warrant election, and § 39 deferral eligibility, reporting,
    security, country changes and late-filing consequences. Its 21 focused
    scenarios pass interpreted and compiled execution. The exact current legal
    text, Skattestyrelsen guidance and post-consolidation amendment checks are
    connected through three typed meta-comment spans.
  - `aktieavancebeskatningsloven-par39a-40.runa` implements § 39 A's persistent
    portfolio and deferred-tax ledger, FIFO disposals, immediate reductions,
    reserved claims, payments, distributions, loans, death, reporting,
    documentation and final cancellation. § 39 B adjusts re-entry basis
    proportionally and cancels the remaining available balance; § 40 applies
    paid-tax reductions with a zero floor. Its 41 focused scenarios pass
    interpreted and compiled execution, including annual priority across
    pending claims and a proof that the ledger cannot reserve or collect more
    than its gross balance. Exact current legal text and the relevant
    Skattestyrelsen guidance are connected through three typed meta-comment
    spans.
  - `aktieavancebeskatningsloven-par35g-35k.runa` implements the election,
    immediate negative-basis gain, 22% transferor-tax balance, FIFO inventory,
    8% annual dividend threshold, credits, payments, deemed disposals,
    value-reducing dispositions, annual reporting, security, residence moves
    and final lapse. Its 17 focused scenarios pass interpreted and compiled
    execution. The enacted text, commencement clause and preparatory work are
    attached through typed meta-comment spans.
  - `aktieavancebeskatningsloven-par17.runa` implements the typed taxpayer,
    acquisition, instrument and amount classification for stk. 1-4. It blocks
    group-internal convertible losses, includes all minimum-taxation
    certificates for a share trader, gives every stk. 4 exclusion priority,
    derives the investment-company counterfactual, and feeds typed § 23,
    KGL § 32 and PSL § 4 bridges. Its 15 focused scenarios pass interpreted
    and compiled execution.
  - §§ 18, 19 B, 19 C, 21 and 22 are modeled as the first
    Personskatteloven § 4, stk. 1, nr. 5 dependency slice for share and
    investment-instrument gain/loss classification, including the § 22
    2.000 kr. threshold, § 18 pre-22 May 1987 bond-exempt loss branch, and
    § 19 C-if-§ 17 personal-income reclassification.
  - Remaining ABL depth includes the remaining dependent classifications.
- Virksomhedsskatteloven:
  `https://www.retsinformation.dk/eli/lta/2021/1836`
  - XML status on 2026-07-18: `Valid`
  - §§ 7-9 a are modeled as the Personskatteloven § 4, stk. 1, nr. 3
    dependency for business capital return. § 8 derives the signed basis from
    typed assets, debts, accounts, transfers and family-housing facts. §§ 9
    and 9 a derive the 2025/2026 annual rates from six official Nationalbank
    observations with the statutory rounding. The § 7 source wrapper consumes
    those results without caller-supplied basis or rate.
  - §§ 22 a, 22 c and 23 a provide the remaining § 4, stk. 1, nr. 3/3 a
    capital-return branches, including the § 23 a personal-income election
    before the remaining capital return can flow into capital income.
  - § 11, stk. 1-3 is modeled as the Personskatteloven § 4, stk. 1, nr. 8
    dependency for rentekorrektion, including the negative indskudskonto
    basis, afkastgrundlag and net-financing caps, the stk. 2
    transfer/indskud cap and the mirrored personal-income addition plus
    capital-income deduction under stk. 3.
  - The new-reserve side of §§ 22 b and 22 d is modeled as the
    Personskatteloven § 3, stk. 2, nr. 7 dependency: adjusted-profit and
    creative-income eligibility, percentage/minimum limits, § 20-regulated
    thresholds, Selskabsskatteloven § 17 reserve tax, bound-account
    requirements and Skattekontrolloven deadline validation. Withdrawal,
    forced recognition and corresponding final-tax credit remain to be added.
- Skattekontrolloven:
  `https://www.retsinformation.dk/eli/lta/2024/12`
  - XML status checked on 2026-07-18: `Valid`
  - §§ 10, stk. 2, 11 and 13 are modeled as the Personskatteloven § 4,
    stk. 1, nr. 3 deadline dependency, so timely transfer is derived from a
    source-backed oplysningsfrist result rather than passed as a loose boolean.
- Pensionsbeskatningsloven:
  `https://www.retsinformation.dk/eli/lta/2024/1243`
  - XML status on 2026-07-18: `Valid`
  - § 16, stk. 1, 4. pkt. is the historic PBL cap reference used by
    Personskatteloven § 13 through income year 2025.
  - § 53 A, stk. 1-6 is modeled as the Personskatteloven § 4, stk. 1,
    nr. 13 dependency for taxable pension-return capital income, with stk. 3
    taxable return, negative-return carry-forward and stk. 4 exclusions.
- Ejendomsavancebeskatningsloven:
  `https://www.retsinformation.dk/eli/lta/2019/132`
  - XML status on 2026-07-18: `Valid`
  - §§ 1, 1 A, 2, 4 and 11 are modeled as the Personskatteloven § 4,
    stk. 1, nr. 14 dependency for taxable real-property gains, with næring
    exclusion, deemed-disposal treatment, the basic gain formula, § 4, stk. 8
    artistic-decoration exclusion and § 11 expropriation-style exclusions.
    Section 11(2) derives the separately taxable adjusted historical gain,
    lapse of the corresponding basis reduction and optional new §§ 6 A/6 C
    reinvestment from typed source facts while preserving the current
    property's own exemption under § 11(1).
- Ejendomsskatteloven:
  `https://www.retsinformation.dk/eli/lta/2023/678`
  - XML status on 2026-07-18: `Valid`
  - § 3 is modeled as the Personskatteloven § 4, stk. 1, nr. 6 dependency for
    owner-occupied-property surplus and deficit, with year-sensitive category
    mapping before and after LOV 615/2026, the stk. 2 foreign/Faroe/Greenland
    extension, and the stk. 3 commercial-rental exclusion.
- Historic Ejendomsværdiskatteloven:
  `https://www.retsinformation.dk/eli/lta/2020/1590`
  - XML status on 2026-07-18: `Historic`
  - Kept as historical source context for the pre-2024 § 4, stk. 1, nr. 6
    wording after LOV 679/2023 moved the operative dependency to
    Ejendomsskatteloven.
- Ligningsloven:
  `https://www.retsinformation.dk/eli/lta/2025/1500`
  - XML status on 2026-07-18: `Valid`
  - § 8 M is modeled as the Personskatteloven § 3, stk. 2, nr. 6 dependency:
    AM contributions under the enumerated AM-law branches, full-taxpayer
    foreign mandatory social contributions under EU or international-agreement
    coverage, and limited-taxpayer foreign employer contributions where an EU
    agreement places the contribution on the employee.
  - § 33 A is the foreign-wage relief exception in § 13, stk. 5.
  - § 5 C is modeled as the Personskatteloven § 4, stk. 1, nr. 12
    dependency for accrued/credited-interest compensation and equivalent
    § 8, stk. 3 provision/præmie amounts, including the § 5, stk. 5 carve-out
    and stk. 3 deduction block.
  - § 8, stk. 3 is modeled as the Personskatteloven § 4, stk. 1, nr. 7
    dependency for running provision/premium amounts and one-off
    provision/premium amounts when the loan or guarantee period is under two
    years.
  - § 7 N is modeled as the Personskatteloven § 4, stk. 7 and § 4 a, stk. 2
    dependency for medarbejderinvesteringsselskab shares, including the
    7.5 pct./30.000 kr. employer-contribution cap, the Danish registered-company
    branch, the EU/EØS approval branch, the withdrawal-value branch and the
    downstream personal-income/share-income routing posture.
  - § 16 A is modeled as the Personskatteloven § 4, stk. 1, nr. 4 dividend
    dependency, with ordinary taxable dividends flowing to capital income when
    a typed § 4 a single-post result places them outside share income, and the
    § 4, stk. 4 personal reclassification branches kept explicit.
  - § 12 B is modeled as the Personskatteloven § 4, stk. 1, nr. 15
    dependency for taxable and deductible running-payment saldo amounts under
    stk. 4-7 and stk. 9, including stk. 10 application posture and stk. 11
    exclusion posture. Because LBK nr. 1500/2025 expressly omits the provisions
    that took effect on 1 January 2026, the current calculation source is the
    combination of that consolidation and LOV nr. 749/2025. The latter source
    also supplies the 19 March 2025 balance-rule transition and the 2026
    henstand rules for tax and arbejdsmarkedsbidrag.
  - § 9 C is modeled as the ordinary befordringsfradrag slice used by
    wage-earner scenarios, with 24 km daily floor, 120 km split, 2025/2026
    rates, 2026 LOV 616 uplift, yderkommune/small-island rates,
    low-income supplement, documented special transport actual-expense
    branch, bridge deductions, reimbursement exclusion, free employer-paid
    transport value posture, and the 2026 SU-student outer-area rule with
    education-transport rebate/godtgørelse exclusion. § 9 D now has the
    disability/chronic-illness special-transport formula and explicitly
    displaces § 9 C, stk. 1-8 and § 9 C, stk. 10 where applicable. Its 2026
    own-transport path derives normal cost from Skatterådets normalfradrag
    kilometre rate, derives factual own-car expenses from the business-driving
    rate tiers, and validates Den juridiske vejledning's 6.000 km and 25.000
    km examples. The workplace path preserves § 9 C, stk. 9 bridge-passage
    deductions even though § 9 C, stk. 1-8 are displaced. The focused
    validation files are
    `ligningsloven-par9c-befordring.audit.runa` and
    `loenmodtager-befordring.scenario.runa`; future work should broaden
    § 9 D beyond the current 2026 own-car slice.
  - §§ 9 J and 9 K are the ordinary employment/job-deduction slice used by the
    wage-earner calculator; § 9 L is modeled for extra pension deductions and
    § 26 nr. 5 transition-compensation input; § 15 P is modeled for
    long-term private-home letting with skematisk and regnskabsmæssig results;
    § 15 Q is modeled for subletting/letting income with regulated 2025/2026
    low/high bundfradrag, stk. 4 proportional coordination from a typed
    LL § 15 P result, rounded 40 pct. deduction on excess rent, actual-expense
    deduction, and the surplus fed into Personskatteloven § 4, stk. 1, nr. 17.
    The § 26 path now has
    2012-2019 Ligningsloven deduction parameter coverage for the first
    transition-compensation calculation layer, and § 26 year packs now derive
    their § 20 regulation number, § 7 top-tax threshold and § 8 health
    contribution rate instead of taking those legal-year facts as fixture
    literals.
  - SKM rates page used for current basis points and caps:
    `https://skm.dk/tal-og-metode/satser/satser-og-beloebsgraenser-i-lovgivningen/ligningsloven`
  - § 9 C 2026 rate sources:
    `https://www.retsinformation.dk/eli/lta/2025/1333`,
    `https://www.retsinformation.dk/eli/lta/2026/616`, and
    `https://skat.dk/borger/fradrag/koerselsfradrag/koerselsfradrag-befordringsfradrag`
- Afskrivningsloven:
  `https://www.retsinformation.dk/eli/lta/2025/1222`
  - XML status checked on 2026-07-30: `Valid`; the LBK version window ends on
    2026-07-25, so current source posture also includes LOV 749/2025 § 2 from
    2026 and LOV 615/2026 § 3 from 2027 under that law's § 16, stk. 5. The first
    transition is modeled through § 40 and Ligningsloven § 12 B; the second uses
    typed landbrugs-, skov- and naturejendom categories in §§ 40 C and 42.
  - The contiguous Afskrivningsloven source corpus covers §§ 1-69, including
    the expressly repealed §§ 37, 41, 53 and 63-67. Personskatteloven § 3,
    stk. 2, nr. 10 consumes typed income, depreciation, loss and other-deduction
    outcomes through § 62 without raw bridge amounts. Procedural, territorial
    and historical transition outcomes remain typed even where they do not
    create a current-year amount.
  - Current rates and limits come from the Ministry's 2026 rates page; the
    separate-balance, negative-balance, cessation and mixed-use interpretations
    are cross-checked against Den juridiske vejledning.
  - § 19 beregner den aktuelle forholdsmæssige anskaffelsessum og bevarer
    gentagne arealforskydninger som en vedvarende liste af særskilte intervaller
    med faktisk og maksimalt mulig afskrivningsprocent. Den juridiske vejlednings
    forøgelses-, reduktions- og senere genforøgelsesforløb er selvstændige
    interpreter- og kompilerede scenarier. §§ 21-24 forbruger nu historikken
    direkte uden rå aggregerede interval-fixtures. Den juridiske vejlednings
    salgs-, nedrivnings- og skadeeksempler samt samme/anden ejendom og fristsvigt
    efter § 24 er valideret i begge backends.
  - § 40 C is modeled as the Personskatteloven § 4, stk. 1, nr. 16 dependency
    for taxable/deductible saldo amounts from EU agricultural payment rights,
    milk quotas and sugar-beet delivery rights. Its input is a typed movement
    list, and § 40 D can feed a market-value acquisition movement into that
    saldo when Danish tax liability or treaty residence begins.
  - SKM `Skatteberegning - hovedtrækkene i personbeskatningen` pages for
    2014, 2016, 2017 and 2018 are used for the historical § 26-relevant
    Ligningsloven deduction rates and pre-2018 absence of §§ 9 K/9 L.
- Sømandsbeskatningsloven:
  `https://www.retsinformation.dk/eli/lta/2023/1181`
  - XML status on 2026-07-18: `Valid`; §§ 3-4 were rechecked on 2026-08-04.
  - LOV nr. 333 af 09/04/2024 was reviewed; it does not amend §§ 3-4.
  - The § 3 implementation also uses BEK nr. 940 af 20/06/2022 § 3 and the
    2026-1 legal guidance for full-time-equivalent sea days and § 4 composition.
  - Den juridiske vejledning om sømandsfradraget og Skattestyrelsens offentlige
    vejledning om perioder med andet arbejde understøtter den typede
    arbejdstilknytning: `https://info.skat.dk/data.aspx?oid=1976791` og
    `https://skat.dk/borger/udlandsforhold/fradrag-for-soefarende-soemaend`.
  - SKM2025.442.LSR supports reclassifying LL § 9 B reimbursement for transport
    in the same seafarer period from tax-free reimbursement to taxable pay:
    `https://info.skat.dk/data.aspx?oid=2459127`.
  - §§ 5-8 are the seamen relief exception in § 13, stk. 5.

Current AM-contribution dependency sources:

- Arbejdsmarkedsbidragsloven:
  `https://www.retsinformation.dk/eli/lta/2020/121`
  - XML status on 2026-07-18: `Valid`
  - §§ 1-7 cover the first ordinary and special-case AM-contribution slice:
    ordinary wage remuneration/naturalier, § 2 stk. 1 nr. 2-6 wage-earner
    amount posts, § 2 stk. 2 naturalia categories as typed source-backed
    category routing, § 2 stk. 3 employer common pension payments, § 3 exclusions
    as a typed nr. 1-5 result carried by the wage-earner basis,
    self-employed bases with and without virksomhedsordning, library-fee
    compensation, and collection-reference posture.
- AM youth exemption amendment:
  `https://www.retsinformation.dk/eli/lta/2025/96`
  - XML status on 2026-07-18: `Valid`
  - § 1 adds 0 pct. AM contribution through the income year in which the
    person turns 17, with effect from January 1, 2026 under § 7, stk. 4.

Current municipal/church-tax and withholding dependency sources:

- Kommuneskatteloven:
  `https://www.retsinformation.dk/eli/lta/2019/935`
  - XML status on 2026-07-18: `Valid`
  - §§ 1, 5 and 6 are the first ordinary municipal-income-tax slice used by
    the wage-earner calculator. § 5, stk. 3 now also has a typed partial-year
    municipal-income-tax result that delegates to the corresponding
    Personskatteloven § 14 calculation form for annualisation and stk. 2
    no-annualisation election cases.
  - §§ 2-3 now cover tax-municipality selection at 5 September, institution and
    Danish-ship exceptions, expatriate Copenhagen-basis tax under
    Kildeskatteloven § 1, nr. 4, tax-liability entry after 5 September, and
    quarter-based move allocation to a new municipality when the residence is
    kept for at least 3 months.
  - §§ 7, 15 and 16 are the first municipal/church settlement slice:
    own-estimate versus state-guaranteed budget basis, monthly provisional
    twelfths, own-estimate after-regulation in three instalments, and the
    § 16, stk. 3 3 pct. threshold/supplement formula with supplied Nationalbank
    discount-rate input. § 7, stk. 4 now calculates the statsguaranteed
    municipal/church tax basis from the basis two years before the calculation
    year and the fremskrivningsprocent. § 16, stk. 4 now covers the municipal
    share of business tax, conjuncture equalisation tax, income equalisation and
    Afskrivningsloven § 40 C acconto-tax repayment bases.
- Kommuneskatteloven self-budgeting amendment:
  `https://www.retsinformation.dk/eli/lta/2025/720`
  - XML status on 2026-07-18: `Valid`
  - § 2 amends § 16, stk. 2 and inserts § 16 a. The model calculates each
    self-budgeting municipality's amount against the state-guaranteed
    alternative, the national correction frame and its annual regulation, and
    the municipality's positive proportional share before deducting that share
    from the § 16 settlement. The amendment applies from grant year 2026.
- Folkekirkens økonomi:
  `https://www.retsinformation.dk/eli/lta/2023/424`
  - XML status on 2026-07-18: `Valid`
  - § 18 is the first ordinary church-tax membership/rate slice used by the
    wage-earner calculator.
- Folkekirkens økonomi amendment:
  `https://www.retsinformation.dk/eli/lta/2025/1772`
  - XML status on 2026-07-18: `Valid`
  - § 2, nr. 4-6 touches § 18 and is tracked as a dependency for current
    church-tax wording.
- Kildeskatteloven:
  `https://www.retsinformation.dk/eli/lta/2024/460`
  - XML status on 2026-07-18: `Valid`
  - §§ 41, 43, 46 and 48 are the first ordinary A-income and A-tax withholding
    slice used to distinguish final annual tax from payroll withholding. § 48
    now covers e-skattekort retrieval posture, main-card period allowances,
    bikort with no allowance, frikort/no-card behavior, optional higher
    withholding percentage, base rounding to whole 10-kroner amounts, and
    § 48, stk. 11 pension-institution withholding at 40 pct. without allowance.
    §§ 58, 60-62, 62 A, 62 C and 67 now cover the first final-settlement slice:
    B-skat installment calendar projection, crediting, restskat/overskydende
    skat balance, spouse offsetting, restskat percentage supplement and timing
    posture, system-date-driven § 61 stk. 4/stk. 6 restskat rateplans,
    overskydende skat compensation and refund posture, amended annual statement
    interest posture, minimum-rate thresholds, and dividend-tax credit posture.
- Bekendtgørelse om kildeskat:
  `https://www.retsinformation.dk/eli/lta/2025/839`
  - XML status on 2026-07-18: `Valid`
  - §§ 2, 5, 8, 9, 12 and 13 are the first forskudsopgørelse-to-skattekort
    generation slice used to turn a forskudsskat and unrounded withholding
    percentage into card allowance, rounded withholding percentage, and
    possible B-tax overflow.
- Forskudsregistrering/indeholdelsesprocent 2026:
  `https://www.retsinformation.dk/eli/lta/2025/1094`
  - XML status on 2026-07-18: `Valid`
  - § 6 is the first annual source-backed derivation of the 2026
    indeholdelsesprocentsats: skattekommunens laveste skatteprocentsats plus
    positive mellemskat/topskat/toptopskat rates computed with two decimals.
- Forskudsregistrering/indeholdelsesprocent 2026 amendment:
  `https://www.retsinformation.dk/eli/lta/2025/1828`
  - XML status on 2026-07-18: `Valid`
  - Amends BEK 1094 § 1, stk. 2, and is tracked as a current dependency;
    it does not change § 6.
- Opkrævningsloven:
  `https://www.retsinformation.dk/eli/lta/2024/1040`
  - XML status on 2026-07-18: `Valid`
  - §§ 1, 2, 4, 5 and 7 are the first payment-deadline/remittance/rate slice
    for withheld A-skat and AM-bidrag: ordinary monthly deadline,
    large-withholder deadline, region/municipality exception, provisional
    assessment posture, corrected underpayment, late-payment interest posture,
    and the § 7 stk. 2 annual-rate formula from Nationalbank July/August/
    September kassekreditrente inputs.
  - The 2025 and 2026 § 7 stk. 2 settlement-rate fixtures now use
    Skattestyrelsen's published `SKM2024.619.SKTST` and
    `SKM2025.720.SKTST` rate sources:
    `https://info.skat.dk/data.aspx?oid=2436822` and
    `https://info.skat.dk/data.aspx?oid=2459995`.
  - The same 2025 and 2026 rate fixtures now also use Danmarks Nationalbank's
    `DNRUUPI` Statbank table (`AL20`/`EFFR`/`1100`/`Z01`/`ALLE`/`ALLE`) for
    July, August and September in the preceding year. The model stores the
    fetched effective interest rates as thousandths of a percent, rounds them
    into the law's two-decimal basispoint input, and proves that the derived
    § 7, stk. 2 rate matches the SKM-published annual rate:
    `https://api.statbank.dk/v1/tableinfo/DNRUUPI?lang=da`.
  - The § 7, stk. 1 late-payment supplement source drift is resolved through
    LOV 1694/2024, LOV 1783/2025 and BEK 1793/2025: the live supplement is
    0,85 procentpoint from January 1, 2026, so the 2026 late-payment monthly
    rate fixture is 0,95 pct. before daily accrual mechanics.
  - Den juridiske vejledning 2026-1, A.B.4.7.2, supplies the administrative
    daily-interest convention: the renteår is the calendar year, so the day
    divisor is 365 or 366 in leap years:
    `https://info.skat.dk/data.aspx?oid=2168585&chk=220619`.

Current external validation sources:

- Skattestyrelsen calculator:
  `https://www.tastselv.skat.dk/fskbrgn2/Skprofil.aspx?indkomstaar=2026`
  - Supporting public page:
    `https://skat.dk/en-us/individuals/preliminary-income-assessment/calculate-your-pay`
  - Retrieved on 2026-07-18.
  - Calculator version observed in the profile form: `26.2.2.3`.
  - First fixture: enlig Copenhagen taxpayer, born 01.01.1980, no church tax,
    no spouse/children/self-employment, 600.000 kr. in `Lønindkomst mv.`
    (`tbAFYfnr201`), all other tax-information fields blank/default.
  - Observed result used in `skatdk-2026-ekstern.scenario.runa`: final tax
    including AM contribution 208.725,64 kr., forskudsskat to A/B-tax
    collection 160.725,64 kr., trækprocent 36 pct., and monthly tax-card
    allowance 8.164 kr.
- Den juridiske vejledning 2026-1, C.F.1.6.2.1:
  `https://info.skat.dk/data.aspx?oid=1977388`
  - Used in `omregning-skatteloft-ekstern.scenario.runa` for the official
    § 14 annualisation example: 27-day tax-liability period, one-off income not
    annualised, recurring items annualised and rounded to whole kroner, yielding
    444.077 kr. personal income, -38.052 kr. capital income, 60.865 kr.
    ligningsmæssige fradrag, and 345.160 kr. taxable income.
  - Used in `kapitel-04-omregning-skatteloft.runa` for the § 14 stk. 2
    election posture that the oplysningsskema election belongs to the year
    where full tax liability ceases or begins, and that reversal must be stated
    by 30 June in the second calendar year after the income year.
- Skatteministeriet, "Oversigt over kommuneskatter":
  `https://skm.dk/tal-og-metode/satser/oversigt-over-kommuneskatter`
  - Used in `omregning-skatteloft-ekstern.scenario.runa` for the
    `kommuneskattesatser_2026.xlsx` Langeland row: 26,30 pct. municipal tax and
    1,24 pct. published `Nedslag pct.`.
- Beskæftigelsesministeriet, "Boligstøtte", satser for 2026:
  `https://bm.dk/satser/satser-for-2026/boligstoette`
  - Used in `husholdning-benefit-cliffs.audit.runa` for boligsikring § 22/§ 23
    rates: 170.300 kr. income threshold, 44.900 kr. child-threshold increment
    for the 2nd-4th child, 28.700 kr. minimum own payment, and 50.412 kr.
    annual maximum boligsikring.
- Styrelsen for Arbejdsmarked og Rekruttering, "Boligsikring":
  `https://star.dk/ydelser/boligstoette-boernetilskud-og-hjaelp-i-saerlige-tilfaelde/boligstoette/boligsikring`
  - Used in `husholdning-benefit-cliffs.audit.runa` for the official
    calculation posture that only children from the 2nd through 4th child
    increase the § 22 boligsikring income threshold.

Working decision: use `2021/1284` as the current consolidated source for live
encoding, while preserving `2019/799` as source lineage because the valid
consolidation explicitly builds on it. The 2019 source remains useful for
historical audit and diffing, but it should not be treated as the live basis for
calculating a current taxpayer's tax. For provisions modified by later valid
amendment acts, such as § 13's 2026 PBL § 16 repeal, the amendment act must be
encoded as a temporal rule on top of the consolidation.

## Current Implementation Status

- Typed meta anchors support generic `role:binding` references, repeated roles,
  pure ground values, definition locations, and type/role query filters while
  preserving the legacy `--@source` syntax.
- Folder created at `examples/danish-income-tax/`.
- `source-status.runa` exists and checks/runs with `runa run`; it now models
  Retskilde records with named metadata fields and separates formal legal
  validity from current-day XML metadata freshness.
- `scripts/refresh-danish-tax-source-status.py` exists and self-tests; the live
  run checks all `Retskilde(...)` records against official Retsinformation XML
  before source metadata is refreshed by hand.
- `kapitel-01-indkomst.runa` findes og passerer `runa check`. Sporet for
  kapitalomkostninger efter personskattelovens § 4, stk. 2-3 afleder nu den
  lovbestemte anvendelse, afskæringen efter ligningslovens § 17 C og en mulig
  omklassifikation til personlig indkomst fra typede kildefakta frem for at
  modtage et ubestemt beløb.
- `ligningsloven-par17c-formueadministration.runa` bevarer den gældende
  officielle § 17 C-tekst ordret og modellerer de nævnte
  formueadministrationsudgifter som typede afskæringer. Det fokuserede
  `personskatteloven-par4-kapitalomkostninger.scenario.runa` dækker alle tre
  formål i § 4, stk. 2, alle fem nævnte § 17 C-kategorier, begge
  næringsomklassifikationer i § 4, stk. 3 og ugyldige fakta i begge
  udførelsesveje.
- `kapitel-02-statsskat.runa` exists and checks with `runa check`.
- `kapitel-03-personfradrag.runa` exists and checks with `runa check`.
- `kapitel-04-omregning-skatteloft.runa` exists and checks with `runa check`.
- `kapitel-05-afsluttende-bestemmelser.runa` exists and checks with
  `runa check`.
- `arbejdsmarkedsbidragsloven.runa` exists and checks with `runa check`.
- `arbejdsmarkedsbidrag-loenmodtager.scenario.runa` exists and checks/runs
  with `runa run`.
- `kommuneskatteloven.runa` exists and checks with `runa check`.
- `kommuneskattelov-afregning.scenario.runa` exists and checks/runs with
  `runa run`.
- `folkekirkens-oekonomi.runa` exists and checks with `runa check`.
- `kildeskatteloven.runa` exists and checks with `runa check`.
- `kildeskattebekendtgoerelsen.runa` exists and checks/runs with `runa run`.
- `forskudsregistrering_2026.runa` exists and checks/runs with `runa run`.
- `slutopgoerelse.runa` exists and checks/runs with `runa run`.
- `opkraevningsloven.runa` exists and checks/runs with `runa run`.
- `ligningsloven_fradrag.runa` exists and checks with `runa check`; it covers
  §§ 9 J/9 K/9 L wage-earner and pension deductions plus § 15 Q
  subletting/letting income deductions.
- `ligningsloven_kapitalindkomst.runa` exists and checks/runs with `runa run`;
  it covers the LL § 5 C dependency consumed by Personskatteloven § 4,
  stk. 1, nr. 12, the LL §§ 6/6 A dependencies consumed by § 4, stk. 1,
  nr. 1, the LL § 8, stk. 3 dependency consumed by § 4, stk. 1, nr. 7, and
  the LL § 12 B dependency consumed by § 4, stk. 1, nr. 15, plus the LL
  § 14 A dependency consumed by § 4, stk. 1, nr. 10 and the LL § 16 A
  dividend slice consumed by § 4, stk. 1, nr. 4 through a typed § 4 a
  share-income classification result.
- `ligningsloven_cfc.runa` exists and checks/runs with `runa run`; it covers
  the LL § 16 H and § 16 I, stk. 6-7 CFC dependency consumed by
  Personskatteloven § 4 b.
- `personskatteloven-par4b-cfc.runa` keeps the verbatim § 4 b source text and
  its amount-level aggregation rules in a focused importable legal module.
  `personskat-cfc-kildefakta.runa` adapts canonical § 16 H/§ 16 I source facts
  to that module without caller-supplied legal conclusions, and
  `personskat-cfc.scenario.runa` exercises the complete path through § 8 b.
- `kursgevinstloven.runa` exists and checks with `runa check`; it covers the
  first Kursgevinstloven dependency slice consumed by Personskatteloven § 4,
  stk. 1, nr. 2 for ordinary personal claims, selected debt cases and basic
  financial contracts.
- `kursgevinstloven-aarsnetto.runa` og
  `personskat-kursgevinst-aarsnetto.scenario.runa` findes og passerer både
  fortolket og kompileret udførelse. Personskat modtager nu identificerede
  fordringer, obligationer og obligationsbaserede minimumsbeviser med typede
  kildefakta, anskaffelses-, afståelses- og ultimohændelser samt en udtrykkelig
  position fra det foregående indkomstår. Reglerne afleder selv KGL §§ 1 og
  12-18, opgørelsen efter §§ 25-26, ABL § 22 og den fælles 2.000-kr.-grænse
  sammen med gæld og sælgerpantebreve. Det tidligere offentlige, allerede
  nettede restbeløb findes ikke længere i Personskat-kontrakten.
- `kursgevinstloven-saelgerpantebrev.runa`, its `.scenario.runa` file,
  `kursgevinstloven-ebl-par6d.runa` and its `.scenario.runa` file exist and pass
  interpreted and compiled execution. They model proportional cash-value basis,
  ordinary repayments, partial/full disposal, extraordinary redemption and the
  EBL § 6 D event bridge. Lifetime spouse succession, estate succession,
  direct succession by a surviving spouse and distribution from an estate now
  preserve or reset the KGL position according to the typed statutory facts.
  Partial spouse transfers split principal and basis exactly between stable
  claim-part identities; unsupported beneficiaries and indivisible whole-krone
  allocations fail closed. Same-year payments consume those identities in
  explicit source order, and EBL/KGL realization outcomes are cross-checked.
  Canonical Personskat matches notes to EBL ledgers and the target taxpayer by
  identity and emits § 4, stk. 1, nr. 2 posts only for person realizations when
  the complete chain is valid.
- `kursgevinstloven-par32.runa`, `kursgevinstloven-par32.scenario.runa` and
  `kursgevinstloven-par32-aktieklasser.scenario.runa` exist and pass interpreted and compiled
  execution. They implement the complete current § 32 annual contract-loss
  ledger, including own and spouse offsets, ABL § 13 A priority, explicit
  full/partial share-offset elections, dated carry, the pre-2010 transition,
  the 2024 MTF transition for both contracts and share gains, and real-estate
  seller/buyer basis adjustments. The typed share-gain basis covers ABL § 12
  and § 25, § 20, stk. 2, § 21, §§ 19 B-19 C and § 22, including ABL § 3,
  § 19 D, exclusions and spouse ordering. Twenty-six focused scenarios cover
  these branches in both runtimes.
- `aktieavancebeskatningsloven.runa` exists and checks/runs with `runa run`;
  it covers the ordinary ABL §§ 12-15/23/24/26 nominal-share calculation path
  consumed by Personskatteloven § 4 a, plus the §§ 17/18/19 B/19 C/21/22
  dependency slice consumed by § 4, stk. 1, nr. 5. The ordinary path includes
  persistent average-basis positions, realization events, listed/unlisted loss
  treatment, spouse transfer/carry-forward and the § 14/§ 15 conditions. Each
  taxable loss now derives a § 5 A result from typed per-disposal source facts
  before §§ 13, 13 A and 14 consume the reduced loss.
- `aktieavancebeskatningsloven-par5a.runa` and
  `aktieavancebeskatningsloven-par5a.scenario.runa` exist and pass interpreted
  and compiled execution. Twenty focused scenarios cover every reduction
  component, the loss cap, invalid amounts, the § 22, stk. 6 exclusion, both
  transition branches, complete lager-disposal composition, forged-result
  rejection and composition before § 9.
- `aktieavancebeskatningsloven-par6-7.runa` and
  `aktieavancebeskatningsloven-par6-7.scenario.runa` exist and pass interpreted
  and compiled execution. Eleven focused scenarios cover both § 6 liability
  grounds, person, estate, life-insurer status, the outside result, forged
  result rejection, and composition into § 23.
- `aktieavancebeskatningsloven-par19b-22.runa` and
  `aktieavancebeskatningsloven-par19b-22.scenario.runa` exist and pass
  interpreted and compiled execution. They derive annual-average asset tests,
  KGL §§ 29-33 underlying assets, the exact 25% ownership look-through,
  § 19 B election/reporting deadlines, and effective § 19 B/§ 19 C or
  § 21/§ 22 status. Forty focused scenarios also cover the neutral
  § 23 fact boundary, wrong-year rejection, forged-product rejection, and the
  downstream Personskatteloven and KGL § 32 routes.
- `aktieavancebeskatningsloven-par19-20a.runa` and
  `aktieavancebeskatningsloven-par19-20a.scenario.runa` exist and pass
  interpreted and compiled execution. They derive § 19 UCITS,
  repurchase-company and collective-investment-company status from typed
  facts, including participant grouping, the exact 10% and 15% tests,
  controlling-owner look-through and the employee-company exception. The
  statute's non-numeric `hovedsagelig` subsidiary test remains a closed legal
  assessment with warning metadata rather than an invented percentage. The
  resulting taxpayer classification routes through §§ 19 A, 20 and 20 A;
  § 20 A's related-claim loss restriction reaches the § 23 annual result.
  Twenty-seven focused scenarios cover boundary values, wrong-year and
  wrong-taxpayer rejection, and both allowed and denied § 20 A losses.
- `investeringsklassifikation.calculate.runa` exposes four derived investment
  classifications as typed calculation contracts. Schema generation, XLSX
  generation and XLSX `runa call` round-trip pass, including the recursive
  >=25% look-through and the nested § 19 controlling-owner graph. XLSX input
  schema v3 gives payload-bearing variants discriminator dropdowns and typed,
  conditionally active columns. Direct assets, owner positions, participants,
  participant companies and claims become keyed related sheets; inactive
  variant fields fail closed instead of being accepted as stray JSON. This is
  the generic workbook foundation for the complete citizen contract, not a
  separately maintained tax form.
- `aktieavancebeskatningsloven-par9.runa` and
  `aktieavancebeskatningsloven-par9.scenario.runa` exist and pass interpreted
  and compiled execution. Eighteen focused scenarios cover the current § 9
  annual ledger, including § 5 A-reduced post losses.
- `personskatteloven-par4a-ordinaere-aktier.scenario.runa` exists and
  checks/runs in both backends. Focused scenarios cover average basis, partial
  disposals, main-shareholder allocation, listed/unlisted losses, missing
  acquisition information, housing rights, invalid disposals, § 5 A reduction,
  rejection of a taxable loss without § 5 A facts, own § 4 a net share income
  and the spouse's transferred § 4 a loss post. The canonical
  `personskat-calculate.scenario.runa` also proves the reduced loss through
  `beregn_personskat`.
- `virksomhedsskatteloven.runa` exists and checks/runs with `runa run`; it
  covers the Virksomhedsskatteloven §§ 7-9 a/22 a/22 c/23 a capital-return
  dependency consumed by Personskatteloven § 4, stk. 1, nr. 3 and nr. 3 a,
  including a source-derived § 8 basis and §§ 9/9 a rates, and the § 11
  rentekorrektion dependency consumed by § 4, stk. 1, nr. 8. It also covers
  the §§ 22 b/22 d new-reserve calculation consumed by § 3, stk. 2, nr. 7 plus
  FIFO recognition, mandatory recognition events, account release,
  corresponding-tax settlement and the § 3, stk. 1 gross-income bridge.
- `virksomhedsskatteloven-par8-par9a.scenario.runa` exists and passes
  interpreted and compiled execution. It covers every § 8 asset valuation
  family, signed deductions, family-housing debt allocation, invalid duplicate
  identities and the pre-1987 valuation boundary, both 2025/2026 rate chains,
  unsupported-year failure, publication posture and the source-derived § 7
  result.
- `personskatteloven-par3-indtaegtsfoering.scenario.runa` exists and
  checks/runs with `runa run`; it covers voluntary partial FIFO recognition,
  the ten-year and deficit rules, ordinary cessation, transition to the
  business tax scheme, § 22 d bankruptcy and permanent-establishment exit,
  succession settlement, and the resulting personal-income amount.
- `afskrivningsloven.runa` exists and checks/runs in both backends; it covers
  the contiguous Afskrivningsloven §§ 1-69 source corpus. Typed amount outcomes
  feed Personskatteloven § 3, stk. 2, nr. 10, while the § 40 C dependency feeds
  § 4, stk. 1, nr. 16.
- `statsskatteloven.runa` exists and checks with `runa check`; it exposes the
  source-bounded ordinary-depreciation fallback under § 6, litra a.
- `etableringskontoloven.runa` exists and checks with `runa check`; it covers
  §§ 1-4 and keeps establishment-account and entrepreneur-account deductions
  as separate typed amounts.
- `personskatteloven-par3-afskrivning-ivaerksaetter.scenario.runa` exists and
  checks/runs with `runa run`; it validates the § 3, stk. 2, nr. 10-11 amount
  cascade, the principal 2026 boundaries, negative-saldo and cessation paths,
  and the combined chapter 2 income/deduction result.
- `pensionsbeskatningsloven.runa` exists and checks/runs with `runa run`; it
  covers the Pensionsbeskatningsloven § 53 A dependency consumed by
  Personskatteloven § 4, stk. 1, nr. 13.
- `ejendomsavancebeskatningsloven.runa` exists and checks/runs with
  `runa run`; it covers the Ejendomsavancebeskatningsloven gain dependency
  consumed by Personskatteloven § 4, stk. 1, nr. 14.
- `skatteaar-parametre.runa` exists and checks with `runa check`.
- `loenmodtager_beregning.runa` exists and checks with `runa check`.
- `loenmodtager-fixtures.scenario.runa` exists and checks/runs with `runa run`.
- `loenmodtager-par11.audit.runa` exists and checks/runs with `runa run`.
- `loenmodtager-par13-spouse.audit.runa` exists and checks/runs with
  `runa run`.
- `loenmodtager-par13-priority.audit.runa` exists and checks/runs with
  `runa run`.
- `personskatteloven-par3-fradrag.audit.runa` exists and checks/runs with
  `runa run`.
- `personskatteloven-par4-renter-ligningslov6.audit.runa` exists and
  checks/runs with `runa run`.
- `personskatteloven-par4-kursgevinst.audit.runa` exists and checks/runs with
  `runa run`.
- `personskatteloven-par4-virksomhedsskattelov-kapitalafkast.audit.runa`
  exists and checks/runs with `runa run`.
- `personskatteloven-par4-aktie-kapital.audit.runa` exists and checks/runs
  with `runa run`.
- `personskatteloven-par4a-aktieavance.audit.runa` exists and checks/runs
  with `runa run`.
- `personskatteloven-par4-ligningslov8stk3.audit.runa` exists and checks/runs
  with `runa run`.
- `personskatteloven-par4-virksomhedsskattelov11.audit.runa` exists and
  checks/runs with `runa run`.
- `personskatteloven-par4-passiv-virksomhed.audit.runa` exists and checks/runs
  with `runa run`.
- `personskatteloven-par4-udlejning-driftsmidler.audit.runa` exists and
  checks/runs with `runa run`.
- `personskatteloven-par4-ligningslov5c.audit.runa` exists and checks/runs
  with `runa run`.
- `personskatteloven-par4-ligningslov12b.audit.runa` exists and checks/runs
  with `runa run`.
- `personskatteloven-par4-ligningslov12b.scenario.runa` exists and checks/runs
  with `runa run`; it validates the 1999, 19 March 2025 and 2026 regime
  boundaries, the official 257.500 kr. henstand example, proportional
  installments on payments and right assignments, asset-specific financing,
  AMBL §§ 4-5 authority, 2026 interest and fee, default, cessation, the
  official retained-profit adjustment, typed historical/current henstand
  bases, later tax-liability entry, institutional exclusion and § 4
  classification.
- `personskatteloven-par4-pensionsbeskatningslov53a.audit.runa` exists and
  checks/runs with `runa run`.
- `personskatteloven-par4-ejendomsavance.audit.runa` exists and checks/runs
  with `runa run`.
- `personskatteloven-par4-afskrivningslov40c.audit.runa` exists and
  checks/runs with `runa run`.
- `personskatteloven-par4-fremleje.audit.runa` exists and checks/runs with
  `runa run`.
- `personskatteloven-par13a-gaeldsordning.audit.runa` exists and checks/runs
  with `runa run`.
- `skatdk-2026-ekstern.scenario.runa` exists and checks/runs with `runa run`.
- `delaar-scenarier.scenario.runa` exists and checks/runs with `runa run`.
- `omregning-skatteloft-ekstern.scenario.runa` exists and checks/runs with
  `runa run`.
- `husholdning-scenarier.scenario.runa` exists and checks/runs with `runa run`.
- `husholdning-benefit-cliffs.audit.runa` exists and checks/runs with
  `runa run`.
- `aktieindkomst-pension.audit.runa` exists and checks/runs with `runa run`.
- `aktieindkomst-slutopgoerelse.runa` exists and checks with `runa check`.
- `aktieindkomst-slutopgoerelse.scenario.runa` exists and checks/runs with
  `runa run`.
- `slutopgoerelse.scenario.runa` exists and checks/runs with `runa run`.
- `indeholdelse-afregning.scenario.runa` exists and checks/runs with
  `runa run`.
- `kildeskat-pension-indeholdelse.scenario.runa` exists and checks/runs with
  `runa run`.
- `personskatteloven-bomber.audit.runa` exists and checks/runs with `runa run`.
- `personskatteloven-konfiskatorisk.audit.runa` exists and checks/runs with
  `runa run`; its bounded year/municipality grid is now declared as
  constructor-shaped `|` facts and enumerated with `findall`.
- `personskatteloven.audit.runa` exists and checks with `runa check`; focused
  `.audit.runa` entrypoints are preferred for per-slice execution while the
  umbrella audit stays broad. Its compiled `runa run` currently reproduces the
  tracked idle condition in `td-ff8eef`, so it is a check-only aggregate until
  that runner issue is resolved; the focused scenarios remain dynamic gates.
- `pengebeloeb.runa` exists and checks/runs with `runa run`.
- Website research page exists at `/research/personskatteloven` and renders
  source status, milestone status, selected audit signals, and the checked
  `.runa` corpus.
- The current `.runa` slices encode source validity, source lineage, the
  §§ 1-4 b income taxonomy including amount-level § 1 ordinary taxable-income
  composition across the separate § 2 categories, amount-level § 3 personal-income inclusion
  and deduction totals, amount-level § 4 net capital-income inclusion,
  deductible capital costs, ordinary interest/LL §§ 6/6 A deduction
  classification under stk. 1, nr. 1, Kursgevinstloven gain/loss classification
  under stk. 1, nr. 2 with claim thresholding, loss blocks, selected debt
  treatment, contract loss limitation and personal-income reclassification,
  Virksomhedsskatteloven §§ 7/22 a/22 c/23 a capital-return classification
  under stk. 1, nr. 3 and nr. 3 a, including § 23 a personal-income election
  reduction, LL § 16 A dividend classification under stk. 1, nr. 4,
  Aktieavancebeskatningsloven §§ 12-15/23/24/26 ordinary nominal-share
  calculation into § 4 a and §§ 17/18/19 B/19 C/21/22 gain/loss
  classification under § 4, stk. 1, nr. 5, direct stk. 1, nr. 5 a membership
  certificate classification, and stk. 1, nr. 5 b investment-intermediary
  amount classification with stk. 4-7 personal-income reclassification,
  Ejendomsskatteloven § 3 owner-occupied-property classification under
  stk. 1, nr. 6 with LOV 679/2023 transition provenance, LOV 615/2026
  category renumbering, foreign/Faroe/Greenland inclusion and commercial-rental
  exclusion,
  Virksomhedsskatteloven § 11 rentekorrektion under stk. 1, nr. 8
  with a capital-income deduction and separate personal-income addition,
  passive self-employed business owner-count
  classification under stk. 1, nr. 9 and stk. 9, LL § 14 A stk. 1 payment
  classification under stk. 1, nr. 10 with stk. 2 payouts kept outside nr. 10,
  leasing-income classification
  under stk. 1, nr. 11 and stk. 8, LL § 5 C compensation classification under
  stk. 1, nr. 12, Pensionsbeskatningsloven § 53 A return classification under
  stk. 1, nr. 13, Ejendomsavancebeskatningsloven real-property-gain
  classification under stk. 1, nr. 14, LL § 12 B running-payment saldo
  classification under
  stk. 1, nr. 15, Afskrivningsloven § 40 C saldo classification under
  stk. 1, nr. 16, subletting/letting surplus classification
  under stk. 1, nr. 17 derived from LL § 15 Q regulated bundfradrag, stk. 4
  § 15 P coordination, rounded 40 pct. excess-rent deduction,
  actual-expense branch, and positive surplus,
  positive/negative net-capital projections
  and personal-income reclassification, plus amount-level § 4 a share-income
  inclusion, stk. 2 exclusions, stk. 3 personal-income reclassification,
  ABL ordinary-share and § 19 B/§ 21 share-income bridges, § 13 A spouse
  loss transfer, negative share-income preservation and pension deduction from
  positive share income, and amount-level § 4 b CFC-income aggregation with positive § 8 b
  tax base projection, the §§ 5-9 state-tax skeleton including amount-level
  § 6 spouse negative net-capital offset and § 7 stk. 5 spouse
  positive-capital threshold/negative-capital offset, § 12 unused
  personfradrag tax-value allocation across the § 9 state-tax basket, the §§ 10-13
  personfradrag/underskud slice, the §§ 14-20 omregning/skatteloft/regulering
  slice, the §§ 21-28 concluding provisions slice, ordinary and special-case
  AM-law,
  Arbejdsmarkedsbidragsloven § 2 nr. 1-6 and stk. 3 wage-earner base
  composition plus § 2, stk. 2 naturalia-category amount routing,
  municipal-income-tax, church-tax, Kildeskatteloven A-income/withholding,
  Kommuneskatteloven § 5, stk. 3 partial-year municipal income tax through the
  corresponding Personskatteloven § 14 calculation result,
  Kildeskatteloven § 48 stk. 11 pension-institution 40 pct. withholding,
  BEK 839 forskudskort generation, BEK 1094 2026 indeholdelsesprocent,
  Kildeskatteloven §§ 60-62/62 A/62 C/67 slutopgørelse balance,
  restskat timing, date-derived B-skat rate windows, B-skat minimum-rate
  completion plans, system-date-driven § 61 stk. 4/stk. 6 restskat rateplans
  with exact and mixed large/small installment splits, date-derived § 62 A
  interest spans and payout deadlines, and overskydende-skat compensation
  posture,
  Opkrævningsloven payment deadlines and § 7 late-payment rate posture,
  shared money/rounding posture for whole kroner, ten-kroner floors,
  basispoint rounding, and øre-level fractions,
  a source-backed pre-2026 § 7 topskat amount result with regulated
  bundfradrag, regulated positive-capital grundbeløb, PBL § 16 additions,
  personal/capital split and wage-earner calculator reuse,
  a source-backed 2026 reform parameter/result layer deriving mellemskat,
  topskat, toptopskat, and the mellemskat positive-capital grundbeløb from the
  amendment's 2010-level amounts through § 20 while preserving the LOV nr.
  482/2024 source branch for each layer,
  Ligningsloven ordinary wage-earner deduction, LL §§ 5 C/6/6 A/7 N/8/12 B/14 A
  capital-income dependencies, Virksomhedsskatteloven §§ 7/22 a/22 c/23 a
  capital return and § 11 rentekorrektion,
  Pensionsbeskatningsloven § 53 A return
  dependency, Ejendomsavancebeskatningsloven real-property-gain dependency,
  Afskrivningsloven § 40 C saldo dependency, and
  § 15 Q subletting/letting dependency slices,
  § 26 historical year-parameter derivation for 2012-2019,
  2024/2025/2026 tax-year parameter packs, grouped
  wage-earner calculation-domain records, first wage-earner scenarios, a first
  § 14 partial-year wage-earner scenario, a first fictional household scenario,
  a first § 8 a share-income final-settlement scenario with § 67 dividend-tax
  credit splitting and § 8 a stk. 2 composition through the § 9/§ 12 state
  personfradrag allocation slot,
  § 14 stk. 2 election/reversal control flow for full-tax-liability entry or
  exit, including the 30 June second-calendar-year reversal deadline and the
  continued mandatory annualisation path for § 10 stk. 6 limited-taxability
  cases,
  calculator-level nonzero § 8 b CFC tax and § 8 c municipal-equivalent
  limited-taxpayer tax through a grouped `LønmodtagerSkatteforhold` path,
  a source-backed external Skat.dk 2026 wage-earner scenario, complex § 13
  calculator fixtures, and first audit signals.
- The chapter files follow the repeating structure: official legal text in a
  multiline block, then the corresponding Futuruna rules.
- Existing Danish Constitution examples show the intended style: original legal
  text in multiline source blocks, followed by Futuruna types, constants, and
  typed `|` legal rules.
- Typed `|` rule heads, `under` conditions, and `exception` rules are already
  present in the language test corpus and should be used for legal formulations.
- Website integration is active and should be updated whenever a checked
  Personskatteloven `.runa` slice becomes part of the displayed corpus.
- Executable scenario tests use `.scenario.runa` filenames. Cross-cutting audit
  suites use `.audit.runa` filenames.
- New source-law modules should avoid embedding scenario assertions where the
  test facts are better expressed as `.scenario.runa` files. Existing local
  smoke fixtures can be migrated as their surrounding legal slices are revised.

## Implementation Completion Snapshot

As of 2026-08-04, the corpus should be treated as a source-backed first-slice
full-statute implementation plus an ordinary-taxpayer calculator prototype, not
as a complete Personskatteloven calculator.

- Structural/source coverage is high: §§ 1-28 are represented in chapter files,
  and the core dependency laws needed for ordinary wage-earner calculation have
  executable first slices.
- Ordinary wage-earner calculation coverage is useful but not complete: current
  scenarios exercise wage income, AM contribution, ordinary wage-earner
  deductions, municipal/church tax, state-tax components, personfradrag,
  selected § 13 deficit paths including spouse/current-year negative personal
  income, a typed annual ledger for externally assessed or prior-calculated
  deficits, carried-forward negative-personal-income ordering through the
  reusable § 13 complex calculator, and § 13 a debt-settlement reduction
  ordering, § 14 annualisation/election cases, § 19 cases,
  withholding/card generation, and first final-settlement paths.
- Full legal calculation coverage is still materially incomplete: some rules are
  still posture/category coverage rather than amount-level calculations, several
  dependent statutes are first-slice only, and special regimes or edge cases are
  represented by selected scenarios rather than comprehensive calculation paths.
- Working estimate: about 90% complete as an executable research corpus, and
  about 80% complete as a production-grade calculator for Personskatteloven
  plus its necessary dependencies. These are deliberately separate measures:
  represented legal structure is further ahead than exact amount-level support
  for every special taxpayer, transition, cross-year history and dependency.
- Current priority: close source-backed calculation gaps in the law itself.
  Audits should validate newly implemented slices; deeper exploratory "bomb"
  audits, including source-derived confiscatory restskat search expansion in
  `td-f318b1`, are deferred until the main implementation is substantially
  complete.
- Completion posture: the next sessions should prefer converting remaining
  posture/category rules into amount-level legal calculations over adding new
  exploratory audit search spaces.

## File Layout

The corpus is intentionally split across multiple `.runa` files. The split is
legal and operational, not arbitrary: Personskatteloven is grouped by chapter,
tax-year data lives in parameter modules, executable normal-person calculations
live in calculator/fixture modules, and dependent statutes such as
Arbejdsmarkedsbidragsloven live in their own source-cited files.

Each legal file should remain a repeating source sequence:

1. official legal text in a multiline block,
2. an optional note only when the code cannot make the legal choice clear on
   its own,
3. idiomatic Futuruna rules, preferably typed `|` rules with `under` and
   `exception` where the law has conditions or carve-outs.

Imports are preferred over large monolithic files. This lets each slice be
checked independently, lets audit modules compose across laws, and keeps the
website integration able to show verified progress without waiting for the
whole statute to be calculation-complete.

## Domain Model Review

Wide records are not automatically a problem. Parameter packs and result
breakdowns are expected to be wide because they represent a table row or a
reporting surface. A record becomes suspect when unrelated facts are passed down
only so subrules can project one or two fields from it.

Current decision:

- Futuruna supports named arguments (`name = value`) for named-field records,
  scoped-rule constructors, ordinary functions, rule calls, and scoped-rule
  member calls. Wide legal/domain records and boolean-bearing legal predicates
  should use named calls at fixture and boundary-assembly points when
  positional arguments would hide legal meaning.
- `Par1AlmindeligSkattepligtigIndkomstSag` uses product-scoped `|` rules for
  § 1/§ 2 taxable-income composition. It keeps personal income, capital income,
  share income outside ordinary taxable income, CFC income outside the §§ 6-8 a
  taxable-income base, and ligningsmæssige fradrag in one result; the ordinary
  wage-earner calculator now delegates its taxable-income base to that result
  instead of carrying a local formula.
- `Pbl18Input` composes payment timing, allocation history, index elections,
  deduction limits and legal exclusions as named subdomains. The § 3, stk. 2,
  nr. 3 bridge consumes `Pbl18Resultat` or `Pbl52Resultat` together with the
  typed § 4 a pension-deduction result, rather than accepting loose pension and
  share-income amounts that could encode an impossible double deduction.
- `AktieavanceOrdinærHændelsesSag` keeps one company's persistent share
  position together with a typed acquisition or disposal event. The event
  result carries both the updated average-basis position and the distinct
  §§ 12-15 tax outcomes, while `AktieavanceOrdinærÅrssag` owns § 13 A's own,
  spouse and carry-forward allocation. This avoids passing acquisition basis,
  market status and information-condition booleans down separate call chains.
- The confiscatory audit work tightened Futuruna's language/runtime support:
  typed `|` rule-head parameters that name a `RuleScope` type now keep that
  receiver type through checking, and named constructors inside nested
  collection lambdas no longer leak the internal named-argument marker into
  generated closure captures.
- Futuruna now treats constructor-shaped rule facts as proper ground facts for
  `findall` in both interpreted and compiled execution. This lets audit search
  spaces be declared as legal/domain facts instead of duplicated hand-written
  lists.
- The confiscatory audit now records the current distinction between
  current-year annual tax and Kildeskatteloven payment burden: the bounded
  search finds no current-year tax over 100% of the positive income base, but
  does find payment-burden cases over 100% when transferred restskat m.v. is
  included.
- A readability sweep now uses named construction and named function/rule calls
  for the broad executable Danish-income-tax records and boolean-heavy calls
  found by scan, including statutory rate rows, remittance calendar/history
  facts, household scenario assembly, and audit inputs; short date-like triples
  and compact arithmetic helpers can remain positional where that is still
  idiomatic.
- `opkraevningsloven.runa` now splits the former 11-field remittance input into
  `OpkrævningAfregningsperiode`, `OpkrævningTilsvarHistorik`,
  `OpkrævningBankkalender`, `OpkrævningBetaling`, and a small composed
  `OpkrævningASkatAmAfregningInput`.
- `indeholdelse-afregning.scenario.runa` owns the executable remittance facts
  and assertions. The source-law module keeps the original legal text and the
  corresponding rules.
- `Par13KompleksBeregningInput` now composes named subdomains instead of a
  25-field positional record: income basis, tax-value rates, offset-tax pools,
  spouse-transfer facts, negative-personal-income facts, stk. 5 limitation
  facts, and same-business loss facts. The calculator rules project from those
  domain objects, and the scenario/audit fixtures name those facts before
  composing the calculator input.
- `Par13VirksomhedsUnderskudSag` now models § 13, stk. 6-7 as a legal case
  with an explicit source home (`Par13VirksomhedsUnderskudHjemmel`), the
  same-business lock, the same-business offset/carry-forward amounts, and the
  narrower stk. 6 advance-depreciation lock. This keeps the stk. 7
  transparent-company deficit lock from silently inheriting the stk. 6
  advance-depreciation clause.
- `Par27BemyndigelseSag` and `PersonskattelovTerritorialSag` keep the closing
  § 27/§ 28 public booleans backed by typed legal results, so ministerial
  delegation and territorial exclusion retain their source hjemmel instead of
  collapsing into isolated truth values.
- `Par13ModregningSag` uses product-scoped `|` rules for the § 13 ordered
  tax-value offset chain, keeping the carried remainder after § 6, § 7, § 7 a,
  and § 8 a, stk. 2 inside the same legal case while preserving public wrapper
  rule names for downstream calculator/audit files.
- `Par13ÆgtefælleSkatModregningSag` uses product-scoped `|` rules for the
  § 13, stk. 2 spouse tax-value step after spouse taxable-income deduction. It
  keeps the remaining transferred deficit, tax-value rate, spouse tax basket,
  used tax value, deficit amount covered by that tax value, and remaining
  carry-forward amount together instead of passing a loose remainder through the
  wage-earner path.
- `Par13NegativPersonligModregningSag` and
  `Par13FremførtNegativPersonligModregningSag` keep the § 13, stk. 4
  negative-personal-income offset order inside explicit legal cases: current
  year spouse personal income first, then own positive capital and spouse
  positive capital, while carried-forward negative personal income starts in
  the spouses' positive capital income before own and spouse personal income.
  `beregn_par13_kompleks` now returns both result objects instead of only the
  old single-person rest amount.
- `Par13aNedsættelseSag` keeps § 13 a's debt-settlement reduction chain inside
  one legal case: covered debt arrangement, debt reduction after release
  income, debtor deficit, typed carried losses from ABL, KGL and EBL, 40 pct.
  negative share-tax reduction, remaining amount after the debtor, spouse
  non-business netting under stk. 3, and cohabiting spouse business-deficit
  reduction under stk. 2. `Par13aFremførbareTabSag` owns the three-branch
  aggregate and rejects future-year results. `EjendomsavancePar6TabsårSag`
  owns property exclusions, the acquisition-basis cap, own and spouse gain
  offset, and the annual carry-forward ledger. Each year consumes the previous
  year's typed result instead of a caller-supplied loss and eligibility flag.
- `AktiePensionsFradragValgSag` and `Par4aPensionsfradragSag` use
  product-scoped `|` rules for the § 4 a, stk. 4 election and amount layer. The
  election scope keeps income year, requested pension deduction, notice date,
  statutory reversal deadline, timely reversal, and personal-income deduction
  status together. The amount scope consumes that typed result with positive
  share income to derive capped deduction, remaining share income, disallowed
  amount and double-deduction blocking.
- `Par4Stk1Nr9Sag` uses product-scoped `|` rules for § 4, stk. 1, nr. 9 and
  stk. 9. It keeps the business type, total owner count, personal owner count,
  LL § 8 P owner exclusion, substantial-participation condition, covered
  capital-income amount, and noncovered amount together before the § 4 capital
  aggregate consumes the derived capital post.
- `Par4Stk1Nr11Sag` uses product-scoped `|` rules for § 4, stk. 1, nr. 11 and
  stk. 8. It keeps the depreciable asset/ship category, acquisition timing,
  Skatterådet permission, council assessment, substantial-participation
  condition, covered capital-income amount, and noncovered amount together
  before the § 4 capital aggregate consumes the derived capital post.
- `Ligningslov14ASag` uses product-scoped `|` rules for LL § 14 A. It keeps
  stk. 1 borrower-payment coverage, payment-year timing, stk. 2
  restgældsreguleringsfond payouts and payout-year timing together.
  `Par4Stk1Nr10Sag` consumes only the stk. 1 payment deduction as negative
  Personskatteloven § 4, stk. 1, nr. 10 capital income, leaving stk. 2 payouts
  outside that § 4 nr. 10 bridge.
- `Ligningslov5CSag` uses product-scoped `|` rules for LL § 5 C. It keeps the
  income year, compensation role, covered compensation type, settlement-year
  timing, corresponding-interest due-year timing, § 5, stk. 5 carve-out, and
  stk. 3 deduction block together. `Par4Stk1Nr12Sag` consumes the typed result
  as Personskatteloven § 4, stk. 1, nr. 12 capital income rather than taking a
  bare scalar.
- `Ligningslov6KurstabSag` and `Ligningslov6ASag` use product-scoped `|`
  rules for LL §§ 6 and 6 A. The § 6 scope keeps the pre-19 May 1993
  kontantlån conditions, per-term kurstab allocation, the 100 kr. floor,
  stk. 3/stk. 4 extraordinary-redemption reduction, stk. 5 debtor-day split
  and stk. 6 Kursgevinstloven block together. The § 6 A scope keeps the paid
  arbejderboliger and statshusmandsbrug/jordrente amounts together.
  `Par4Stk1Nr1Sag` consumes those typed results together with ordinary interest
  income and expense as Personskatteloven § 4, stk. 1, nr. 1 capital income
  rather than taking a bare interest scalar.
- `KursgevinstlovSag` uses product-scoped `|` rules for the first KGL person
  slice. It keeps the § 14/§ 23 threshold basis, claim opgørelse, selected debt
  opgørelse, financial-contract opgørelse, loss blocks and reclassification
  posture in typed domain objects. `Par4Stk1Nr2Sag` consumes the typed result as
  Personskatteloven § 4, stk. 1, nr. 2 capital income rather than taking a bare
  Kursgevinst scalar.
- `AktieavancebeskatningslovSag` uses product-scoped `|` rules for the first
  ABL person slice. It keeps the source asset type, acquisition/disposal
  amounts, § 17 næring posture, § 18 old-bond loss branch, § 19 B/§ 19 C
  investment-company classification, § 21/§ 22 listed/unlisted treatment,
  taxable gain, deductible loss and Personskatteloven category together.
  `Par4Stk1Nr5Sag` consumes that typed result for § 4, stk. 1, nr. 5 instead
  of passing a loose share-gain scalar. `Par4aAktieavancebeskatningslovSag`
  consumes the same typed ABL result for § 4 a, routing § 19 B/§ 21 amounts to
  share income, § 19 C to the stk. 2 exclusion, and § 19 B-if-§ 17 amounts to
  personal income under stk. 3.
- `EjendomsskattelovPar3Resultat` keeps semantic property categories separate
  from the legal numbering that changes in 2027 under LOV 615/2026.
  `Par4Stk1Nr6Sag` consumes that typed § 3 result plus the property
  surplus/deficit amount for Personskatteloven § 4, stk. 1, nr. 6, avoiding a
  fragile boolean/category scalar at the Personskatteloven boundary.
- `Ligningslov16ASag` uses product-scoped `|` rules for the LL § 16 A dividend
  slice consumed by `Par4Stk1Nr4Sag`. `Par4aAktiepostResultat` gives § 4,
  stk. 1, nr. 4 a typed answer to whether the same dividend is already § 4 a
  share income, replacing the former loose `ikke_aktieindkomst_efter_par4a`
  input flag. `Par4Stk1Nr4Sag`, `Par4Stk1Nr5aSag` and `Par4Stk1Nr5bSag` keep
  the original amount, capital-income classification and personal-income
  reclassification together so the § 4 aggregate can move amounts between net
  capital income and personal income without losing the source reason.
  `Par4Stk1Nr5aSag` now
  gets its statutory entity boundary from `Selskabsskattelov1Stk1Nr6Resultat`
  instead of raw booleans, so the § 1, stk. 1, nr. 6 source line, the
  investment-association carve-out and the § 3/fondsbeskatningsloven exclusions
  remain auditable. `Par4Stk1Nr5bSag` now models the enumerated intermediary
  list as `Par4Stk1Nr5bFormidler`, so a bank, mortgage-credit institution,
  investment firm, investment-management company, alternative-investment-fund
  manager, financial adviser or investment adviser is distinguished from a
  non-listed intermediary before the amount can enter capital income.
- `SkattekontrollovOplysningsfristSag` keeps the income year, § 10 stk. 2 or
  § 11 deadline source, ordinary July 1 deadline, § 13 first-weekday extension
  and actual transfer date together. `Par4Stk1Nr3Sag` consumes that typed
  result for the "overført ... inden oplysningsfristen" condition instead of
  accepting a bare boolean.
- `Ligningslov7NSag` uses product-scoped `|` rules for the LL § 7 N
  medarbejderinvesteringsselskab slice. It keeps the statutory employer
  contribution cap, Danish registration branch, EU/EØS approval branch,
  withdrawal-value branch and Personskatteloven-facing share boundary together.
  `Par4Stk7Ligningslov7NSag` consumes that typed result for the § 4, stk. 7
  personal-income override, while `Par4aLigningslov7NSag` consumes the same
  result for the § 4 a, stk. 2 payout/gain share-income exclusion and leaves
  covered losses as negative share-income posts.
- `Ligningslov8Stk3Sag` uses product-scoped `|` rules for LL § 8, stk. 3. It
  keeps the provision/premium branch, expense amount and loan/guarantee period
  together, so litra a/b running amounts are deductible and litra c one-off
  amounts are deductible only when the period is under two years.
  `Par4Stk1Nr7Sag` consumes the typed result as negative Personskatteloven
  § 4, stk. 1, nr. 7 capital income rather than taking a bare provision scalar.
- `Pensionsbeskatningslov53ASag` uses product-scoped `|` rules for PBL § 53 A.
  It keeps the covered ordning, § 53 B exclusion, stk. 4 carve-outs, PAL-method
  or alternative capital-value return, taxable share allocation, prior
  negative-return use and carry-forward together. `Par4Stk1Nr13Sag` consumes
  the typed result as Personskatteloven § 4, stk. 1, nr. 13 capital income
  rather than passing a loose return amount.
- `EjendomsavancebeskatningslovSag` uses product-scoped `|` rules for EBL
  §§ 1, 1 A, 2, 4 and 11. It keeps disposition type, næring exclusion,
  § 11 expropriation-style exclusion and typed stk. 2 treatment, acquisition
  cash value, § 5/§ 5 A regulation input, § 4, stk. 8 exclusions, disposal cash
  value, gain/loss and taxable gain together. The stk. 2 treatment keeps the
  current property's exempt gain, adjusted historical gain, lapsed basis
  reduction and any new §§ 6 A/6 C reinvestment separate.
  `Par4Stk1Nr14Sag` consumes the typed result as
  Personskatteloven § 4, stk. 1, nr. 14 capital income rather than passing a
  loose real-property gain amount.
- `Ligningslov12BSag` uses product-scoped `|` rules around nested agreement,
  party, reporting and event objects for LL § 12 B. It derives the applicable
  date regime instead of accepting it as a fixture, and keeps the
  running-payment role, application posture, saldo before and after payments or
  cash-converted consideration, negative-saldo amount, later-year payments,
  termination balance, statutory exclusion amounts, acquisition-cost
  adjustments, and obligation-transfer opening value together. Separate typed
  henstand positions distinguish the historical Afskrivningsloven § 40 basis
  from the 2026 LL § 12 B basis. A dated event ledger carries the grant,
  active deferred tax and arbejdsmarkedsbidrag, individually identified
  installments, arrears, payments, interest, fees, default, cessation and
  virksomhedsordning effects without a flat parameter chain or fabricated
  annual closing balance.
  `Par4Stk1Nr15Sag` consumes the typed result as Personskatteloven § 4,
  stk. 1, nr. 15 capital income rather than taking a bare scalar.
- `Afskrivningslov40CAktivSag`, `Afskrivningslov40CSag`,
  `Afskrivningslov40CEjendomstabSag` and `Afskrivningslov40CAcontoskatSag` use
  product-scoped `|` rules for Afskrivningsloven § 40 C. They keep dated asset
  positions and FIFO inventory, typed acquisition/disposal movements, ordinary
  and final-year saldo treatment, negative-saldo history, qualifying property
  loss, 22 pct. advance tax, own/spouse settlement and carried positions
  together. `Afskrivningslov40DResultat` emits the typed acquisition movement
  from the entry-date market value, and `Par4Stk1Nr16Sag` consumes the resulting
  § 40 C result as
  Personskatteloven § 4, stk. 1, nr. 16 capital income rather than taking a
  bare scalar.
- `Afskrivningslov40ASag` and `Afskrivningslov40BSag` keep each quota's
  acquisition mode, legal classification, dated lot, remaining quantity and
  tax basis together. Their FIFO rule is derived from the lot inventory;
  scenarios do not supply a boolean saying whether FIFO happened to hold.
- `Par4Stk1Nr17Sag` uses product-scoped `|` rules for § 4, stk. 1, nr. 17.
  It keeps the taxpayer's housing role, LL § 15 Q branch, gross rent income,
  LL § 15 Q deduction amount, positive surplus, covered capital-income amount,
  and noncovered amount together before the § 4 capital aggregate consumes the
  derived capital post. `Par4Stk1Nr17FraLigningslov15QSag` now bridges from
  the LL § 15 Q amount result into this § 4 classifier, so callers do not need
  to supply the statutory surplus by hand.
- `Ligningslov15QSag` uses product-scoped `|` rules for LL § 15 Q. It keeps
  housing role, letting form, helårsbolig status, reporting branch, deduction
  method, regulated low/high bundfradrag, stk. 4 same-home § 15 P
  coordination from a typed `Ligningslov15PResultat` with rounded day
  percentages and explicit cap choice, rounded 40 pct. excess-rent deduction,
  actual-expense deduction, and resulting surplus together before
  Personskatteloven § 4 consumes the result.
- `Ligningslov15PSag` uses product-scoped `|` rules for LL § 15 P. It keeps
  taxpayer housing role, rooms-vs-whole-home letting form, the skematisk or
  regnskabsmæssig method, 2/3 annual rent/boligafgift, 1 1/3 pct. property
  value with the 24.000 kr. owner minimum, the four-month same-tenant
  condition, ownership/lease-day proportionality, actual-expense deduction,
  taxable letting result, and the stk. 3 lock against later bundfradrag
  together. LL § 15 Q stk. 4 now consumes this result object instead of a bare
  § 15 P deduction scalar.
- `PersonfradragPar10Sag` uses product-scoped `|` rules for § 10 eligibility.
  It keeps tax year, age status, tax-liability posture, partial-year election
  date, and reversal date together so the Kildeskatteloven § 2 full-year case,
  partial-year election deadline, omvalg deadline, sailor-tax exclusion,
  residence-permit applicant exclusion, and researcher-tax exclusion are derived
  from one legal case.
- `PersonfradragStatsskatNedsættelseSag` uses product-scoped `|` rules for
  § 12 stk. 2. It keeps the § 9 state-tax basket and state personfradrag tax
  value together so the unused value after § 6, § 7, § 7 a, § 8, and
  § 8 a stk. 2 is derived inside one legal case instead of passed through a
  chain of loose remainders. The ordinary wage-earner calculator now projects
  its state-tax component fields through this allocator, using tax-year-aware
  mapping because § 7 is topskat before 2026 and mellemskat from 2026 onward.
- `Par9StatsskatPersonfradragSag` uses product-scoped `|` rules for the
  amount-level § 9 reduction order. It keeps the state-tax basket, the § 8
  personfradrag tax value, and the § 6 personfradrag tax value together so
  each value first reduces its own tax component and then falls through the
  remaining § 9 taxes in the statutory order.
- `Par9IkkeStatPersonfradragResultat` covers § 9's "tilsvarende måde" sentence
  for § 8 c tax, municipal income tax, and church tax. The wage-earner
  calculator now delegates those final reductions to this result instead of
  subtracting scalar personfradrag values in place.
- `Par10Stk3ÆgtefælleStatsskatSag` uses product-scoped `|` rules for unused
  personfradrag tax-value transfer to a spouse. It keeps the unused § 8 and
  § 6 tax values, the receiving spouse's § 9 state-tax basket, and the
  year-end cohabitation condition together, then delegates the reduction order
  to `Par9StatsskatPersonfradragResultat` so the amount actually transferred,
  used, left unused, or barred by missing cohabitation stays in one domain
  result without duplicating § 9 mechanics.
- `Par7ReformMellemskatSag` uses product-scoped `|` rules for the 2026
  mellemskat amount layer. It keeps the § 20-regulated personal threshold, the
  § 20-regulated positive-capital grundbeløb, personal income, net-capital
  income, personal/capital split, and resulting tax together so wage-earner
  calculator rules no longer duplicate reform thresholds as loose parameters.
- `ReformPersonligStatsskatSag` uses product-scoped `|` rules for the 2026
  personal-only reform layers. It keeps the § 7 a topskat/§ 8 toptopskat lag,
  statutory 2010-level threshold, § 20-regulated threshold, personal income,
  excess personal base, rate, and kroner tax together so the wage-earner
  calculator and audits can inspect those taxes as named legal results instead
  of opaque scalar calls.
- `Par26Stk7TopskatÆgtefællerSag` uses product-scoped `|` rules for the
  § 26, stk. 7 transition-compensation bridge. It keeps both spouses'
  personal/PBL bases, net-capital incomes, regulated § 26 nr. 3 threshold,
  ordinary § 7 threshold, and § 7 capital allocation together so the old-vs-new
  top-tax difference is derived as a named result instead of a loose scalar.
- `Par26KompensationAfregningSag` composes the annual § 26 line-item
  calculation with the § 26, stk. 1 tax-offset order, so fixtures can prove the
  whole compensation-settlement path instead of separately calling
  `par26_forskelsbeløb_beregning_for_skatteår` and `par26_modregning_resultat`.
- `Par26ÆgtefælleForskelsbeløbParSag` applies the § 26, stk. 4 spouse rule in
  both directions, so a married couple's positive/negative transition
  differences produce named post-stk. 4 amounts and compensation for each
  spouse without caller-side direction selection.
- `Par26Stk5KapitalParSag` applies the § 26, stk. 5 spouse net-capital rule in
  both directions, so a couple's positive/negative net-capital income produces
  named post-stk. 5 amounts and offset totals before the nr. 2 transition
  amount is calculated.
- `Par26Stk6BundfradragParSag` applies the § 26, stk. 6 spouse bundfradrag
  transfer in both directions, so a couple's personal-plus-positive-capital
  income produces named missing-threshold amounts, threshold increases, and
  effective nr. 2 thresholds before the transition amount is calculated.
- `Par26ForskelsbeløbParÅrsSag` composes the annual § 26 year parameters with
  the stk. 6 pair threshold result and stk. 4 spouse difference offset, so
  fixtures can prove the effective nr. 2 threshold changes the actual annual
  line item before the post-stk. 3 spouse offset is applied.
- `Par26Stk8KapitalParSag` applies the § 26, stk. 8 spouse net-capital rule in
  both directions, so a couple's positive/negative net-capital income produces
  named post-stk. 8 amounts and offset totals before the nr. 8 transition
  amount is calculated.
- `Par11NegativKapitalNedslagSag` uses product-scoped `|` rules for § 11.
  It keeps tax year, the taxpayer's net-capital income, the spouse's
  net-capital income, samliv status, and municipal/§ 8 c tax-liability posture
  together so the own negative amount, spouse positive offset, spouse unused
  threshold, effective threshold, reduction base, § 11 stk. 2 rate result, and
  final reduction are derived from one legal case.
  `Par11NedslagModregningSag` separately keeps
  the statutory reduction order across §§ 6, 7, 7 a, 8, § 8 a stk. 2, § 8 c,
  municipal tax, and church tax.
- `Par6ÆgtefælleKapitalModregningSag` uses product-scoped `|` rules for the
  § 6, stk. 3 amount layer. It keeps marriage/samliv status, one spouse's
  positive net-capital income, and the other spouse's negative net-capital
  income together so the offset amount, reduced positive capital basis, and
  residual negative amount are derived from one legal case rather than passed as
  loose scalars.
- `Par7ÆgtefælleKapitalStk5Sag` uses product-scoped `|` rules for the § 7,
  stk. 5 capital-threshold layer. It keeps the taxpayer's net-capital income,
  the spouse's positive net-capital income, the regulated grundbeløb, and
  samliv status together so negative capital is offset before the spouse's
  effective threshold is increased.
- `Par7KapitalskatFordelingSag` keeps the § 7, stk. 10-11 allocation of
  spouse positive-net-capital tax together as a result object. It derives
  whether one or both spouses are over the grundbeløb and assigns the combined
  capital tax to the single over-threshold spouse or splits it by the statutory
  ratio. `par7_højeste_beregningsgrundlag` separately models the stk. 8/stk. 12
  identity rule, including the equal-basis tie-break by largest
  ligningsmæssige deductions.
- `Par8cSkatSag` uses product-scoped `|` rules for the § 8 c municipal
  equivalent tax. It keeps tax year, limited-taxpayer posture, taxable ordinary
  income, and the § 10 stk. 5 personfradrag amount together so coverage,
  taxable base, published rate, personfradrag tax value, and final § 8 c tax
  are derived from one legal case. `Par8cSatsResultat` separately keeps the
  statutory method, published source posture, source rate, and applied
  basispoint rate together so the calculation does not depend on a bare 25 pct.
  constant.
- `LønmodtagerSkatteforhold` groups non-ordinary taxpayer facts for the
  wage-earner calculator: share income and spouse share-income posture, CFC
  income, § 8 c posture, and § 10 personfradrag tax-liability/election facts.
  `LønmodtagerBeregningSag` uses product-scoped `|` rules to compose that tax
  position with the ordinary wage-earner base, so nonzero § 8 a high-layer
  share-income tax, § 8 b CFC tax, and § 8 c municipal-equivalent tax now flow
  through § 5 state-tax aggregation, § 10 personfradrag eligibility, § 13
  deficit tax-value posture, and § 19's municipal-or-§ 8 c rate input without
  adding more scalar fields to `LønmodtagerInput`. The public
  `beregn_lønmodtager` path now uses this scoped model with standard tax
  conditions, so standard fixtures and special tax-condition fixtures no longer
  diverge through separate model constructors.
- `slutopgoerelse.runa` keeps the year-end balance as `KildeskatSlutopgørelseInput`
  plus a statutory `KildeskatPar60Kreditter` credit basket. This is a better
  domain boundary than passing A-skat, AM-bidrag, B-skat, dividend-tax credits,
  voluntary payments, and special credit categories through every subrule.
- `KildeskatRestskatOpkrævningInput` now uses
  `KildeskatRestskatUdskrivningspostur` instead of a cluster of timing booleans.
  The enum models the statutory issuance posture directly and rules derive the
  § 61 branches from it, so impossible flag combinations cannot become fixtures.
- `KildeskatBSkatRateVindue` now groups the B-skat installment calendar
  projection for restskat collection. The preferred restskat input now composes
  `KildeskatDato` and `KildeskatRestskatSystemdatoer`, deriving both the
  statutory issuance posture and the first remaining B-skat rate instead of
  passing those as scenario literals. The lower-level input remains as a
  compatibility helper for focused edge-case audits.
- `KildeskatRestskatMinimumsplan` is the date-aware boundary for the § 61,
  stk. 5 command that January-or-later restskat paid over remaining B-skat rates
  must still be paid in at least three rates. It keeps the ordinary B-skat count,
  missing supplemental rate count, divisible fixture amount, supplemental due
  dates, total rate count, and § 58 last-timely-payment day together instead of
  scattering late-year calendar assumptions across audit rules.
- `KildeskatRestskatRateplan` and `KildeskatRestskatBeløbsdeling` now expose
  the executable installment schedule layer for § 61. The model records
  statutory branch, first/last due date, rate count, last-timely-payment day,
  and exact-vs-mixed large/small installment splits. `KildeskatRestskatSystemdatoer`
  now carries separate source-backed system-start dates for late stk. 4
  three-rate collection and stk. 6 residual collection, including the January
  18.300 kr. over nine remaining B-skat rates case with three 2.034 kr. rates
  and six 2.033 kr. rates.
- `KildeskatRestskatTreRateSag` uses product-scoped `|` rules for the § 61
  stk. 4/stk. 6 three-rate case. The scope keeps the shared derived
  `opkrævning`, system-start dates, installment count, and amount splitting
  together while the public wrapper rules keep the surrounding file stable.
- `aktieindkomst-slutopgoerelse.runa` now owns `AktieindkomstSlutopgørelseCase`
  as reusable product-scoped `|` rules for the § 8 a/§ 67 annual-settlement
  case. The scope keeps the wage-earner breakdown, monthly A-skat, share
  income, spouse share-income threshold facts, and withheld dividend tax
  together while methods derive the effective progression threshold, final
  low-layer share tax, high-layer tax entering final tax before and after § 12
  state personfradrag allocation, § 60 credit basket, and final annual-settlement
  result. The low-wage/high-share scenario now audits that unused state
  personfradrag tax value can reduce the
  § 8 a, stk. 2 amount before Kildeskatteloven final settlement.
  The source-law module now keeps § 8 a stk. 1/stk. 2 share-income tax rates,
  source paragraph, final-tax posture, and slutskat-entry posture in one typed
  rate result.
- `AktieindkomstNegativSlutopgørelseCase` keeps § 8 a, stk. 5-6 negative
  share-income settlement separate from the positive-share case. It derives
  spouse positive-share offset, negative tax, whole dividend-tax credit for
  negative share income, own-slutskat offset, spouse-slutskat offset, and the
  carried-forward remainder while crediting only the amount actually usable in
  the taxpayer's current-year § 60 basket.
- `AktieindkomstUdbytteskatStk3Sag` uses product-scoped `|` rules for
  Personskatteloven § 8 a, stk. 3. It keeps tax year, total share income, and
  dividend tax withheld under Kildeskatteloven § 65 together while defaults and
  exceptions derive the low-rate comparison amount, any over-withheld amount
  credited in slutskatten, the negative-share-income full credit, and the
  remaining final dividend-tax payment.
- `Par7aUdligningsskatPostOpgørelseSag` and
  `Par7aUdligningsskatBeløbsSag` use product-scoped `|` rules for
  Personskatteloven § 7 a. They keep the enumerated pension and pension-like
  payments as amount-carrying posts, exclude invalidity pension, efterløn,
  fleksydelse, førtidspension, and mandatory foreign security schemes before
  calculating the stk. 1 amount, and feed that source-derived amount into the
  existing stk. 3, stk. 4-5, and stk. 6 udligningsskat calculation. The
  historical phase-out rate now carries the § 7 a, stk. 5 source paragraph and
  phased-out posture in one typed result.
- `AktieindkomstÆgtefællerBeggeNegativeSag` uses product-scoped `|` rules for
  Personskatteloven § 8 a, stk. 6. It keeps both spouses' negative share income
  and samliv status together so the double share-income threshold is split
  proportionally when both spouses are negative, and each spouse's negative tax
  is calculated from the allocated threshold instead of granting the double
  threshold twice.
- `KonfiskatoriskCase` uses product-scoped `|` rules for the effective-rate
  audit. It keeps tax year, municipality, wage, positive net-capital income,
  share income, spouse share-income posture, church-tax membership, and
  transferred restskat m.v. together while deriving the wage-earner input,
  § 8 a final-settlement result, positive income denominator, current-year
  tax basis points, and broader payment-burden basis points from one case.
- `KildeskatPar62ARenteDatoInput` and
  `KildeskatPar62AForsinketUdbetalingsdatoInput` keep § 62 A issue and payout
  scheduling date-based. They derive the old "påbegyndte måneder" helper input
  from legal dates, which keeps scenario files from smuggling calendar math in
  as precomputed integers.
- `OpkrævningPar7OffentliggjortSats` is the annual published-rate domain row
  for settlement examples. The Nationalbank July/August/September formula stays
  executable, while live Kildeskatteloven fixtures use the Skattestyrelsen
  source row instead of synthetic monthly-rate literals.
- `opkrævning_par7_stk1_forsinkelsestillæg_basispoint` now captures the
  2026 source-chain amendment from 0,7 to 0,85 procentpoint as a temporal
  exception, and `opkrævning_par7_stk1_månedlig_forsinkelsesrente_basispoint`
  derives the combined monthly late-payment rate from the published § 7, stk. 2
  row plus that supplement.
- `OpkrævningDato`, `OpkrævningPar7DagligRenteInput`,
  `OpkrævningPar7DagligRenteKontekst`, and
  `OpkrævningPar7DagligRenteBeregning` are now the right-sized domain boundary
  for § 7 daily late-payment interest. The context carries principal, latest
  timely payment date, actual payment date, calendar-year rate row, supplement,
  day-count convention, and rentedage together, avoiding loose date/rate
  parameters being passed down merely so subrules can project them.
- `OpkrævningPar7DagligRenteÅrsdel` and
  `OpkrævningPar7DagligRenteTværårBeregning` extend that boundary across
  New Year. Each segment carries its own calendar-year published-rate row,
  supplement, year divisor, rentedage, and `PengeØreBeregning`, so a cross-year
  payment cannot accidentally price 2025 days with a 2026 rate or divisor.
- `pengebeloeb.runa` is now the shared precision boundary for positive clamps,
  minimum amounts, ten-kroner floors, basispoint rounding, basispoint-to-kroner
  multiplication, and øre-fraction rounding. Statutory modules retain their
  local legal names but delegate the common arithmetic posture to this file.
- `Par19SkatteloftNedslagResultat` is now the source-backed § 19 calculation
  boundary. It carries the derived `Par19SkatteloftInput`, the typed
  `Par19SkatteloftResultat` source branch, total tax percentage, excess basis
  points, progressiv tax rate after ceiling relief, and kroner relief for both
  personal and positive-capital skatteloft paths. `LønmodtagerSkatteloftResult`
  wraps that result while preserving the calculator's existing `.input`,
  `.overskydende_basispoint`, and `.nedslag_kroner` fields. The § 19 personal
  and positive-capital ceilings preserve whether the rate comes from LBK nr.
  1284/2021 § 19 stk. 1/stk. 2 or the 2026 LOV nr. 482/2024 § 1 nr. 15 rewrite.
- `LønmodtagerBeregning` now composes the ordinary wage-earner calculation from
  named domain records: income basis, Ligningsloven deductions, tax before
  person allowance, § 5 state-tax aggregate before person allowance, person
  allowance tax value, tax after person allowance before skatteloft, and final
  tax after § 19 relief. The existing flat
  `LønmodtagerBreakdown` remains as the reporting/API projection so website and
  scenario consumers do not have to learn every internal calculation layer.
- `LønmodtagerPar13Sag` is the current right-sized § 13 boundary inside the
  ordinary wage-earner calculator. It keeps the taxpayer input and immediate
  spouse-transfer facts together while internal `|` rules derive the taxable
  deficit, tax-value rate, statutory tax-value offset order, deficit amount
  covered by own tax offset, remaining deficit, spouse deduction, spouse's own
  prior-year deficit priority, spouse tax value offset after the income
  deduction, and carry-forward remainder. The
  focused `loenmodtager-par13-spouse.audit.runa` case verifies that the
  remaining 8.283 kr. deficit after spouse income deduction is further reduced
  by 1.000 kr. of spouse tax-value offset to 4.028 kr. carry-forward. The
  focused `loenmodtager-par13-priority.audit.runa` case verifies that 10.000 kr.
  of the spouse's own prior deficits consumes spouse income before the
  taxpayer's transferred deficit, reducing the spouse-income deduction to
  5.000 kr. and leaving 14.028 kr. to carry forward after spouse tax-value
  offset. Scenario fixtures stay as plain taxpayer/spouse facts plus assertions.
- `LønmodtagerPar11NedslagResultat` is the current right-sized § 11 boundary
  inside the ordinary wage-earner calculator. `LønmodtagerInput` now carries
  `nettokapitalindkomst_kroner`; helper rules derive positive net-capital income
  for progressive positive-capital branches and negative net-capital income for
  § 11. The focused `loenmodtager-par11.audit.runa` case verifies that a 2026
  København wage-earner with -40.000 kr. net capital receives a 3.200 kr. § 11
  reduction before final tax.
- `loenmodtager_beregning.runa` now separates state income tax, municipal/church
  income tax, total income tax, and the final total including AM contribution.
  This keeps Personskatteloven § 14's "helårsskat efter §§ 6-9" from
  accidentally consuming an AM-inclusive cash-flow total.
- `LønmodtagerPar14Input` is the current right-sized § 14 boundary for ordinary
  wage-earner partial-year cases. It carries tax-liability change status, the
  delårs wage-earner input, and tax-liability days together, instead of passing
  those scalars through every helper rule.
- `Par14SkatteberegningResultat` is now the reusable statutory § 14 amount
  boundary. It carries helårsindkomst, helårsskat efter §§ 6-9, the stk. 1/stk. 3
  proportional delårsskat, the stk. 2 period-reduced tax, the governing election
  posture, and the final `skat_efter_par14_kroner`.
- `Par14Beløbspost` now captures whether a § 14 amount is recurring or one-off.
  This keeps the official annualisation example from forcing one-off income
  through the same annualisation path as wage, interest, or A-kasse amounts.
- `KildeskattebekendtgørelseForskudskortInput` now composes a named
  `KildeskattebekendtgørelseForskudskortIndkomstgrundlag` instead of passing
  annual basis, period basis, and excluded basis as loose scalars. The source
  backed Skat.dk fixture showed why this matters: generated tax-card values
  must use the AM-reduced A-tax basis for ordinary wages, not the gross wage
  amount. `KildeskatESkattekortInput` now names that field
  `a_skat_grundlag_kroner` so downstream rules do not smuggle gross A-income
  semantics into withholding calculations.
- `parameterpakke_komplet` now depends on a year+municipality coverage rule
  rather than a broad municipality predicate. This keeps the parameter-pack
  domain honest as new municipalities are added for selected years; Langeland is
  currently source-backed for 2026 only.
- Domain review pass: exact duplicate scalar helpers for positive amounts,
  minimum amounts, and kroner-by-basispoint calculations now route through
  `pengebeloeb.runa`. Wider statutory input records stay explicit where their
  fields are enumerated legal facts rather than accidental parameter plumbing.
- The household scenario helpers are a future candidate for a compact scenario
  input object if more household scenarios are added. With one scenario, the
  current explicit facts remain easier to audit than a new wrapper layer.

Review candidates to revisit deliberately, not as broad churn:

- The §§ 19 B-22 investment domain now separates factual input from legal
  output. `AktieavanceInvesteringsaktivmasse` owns direct annual-average
  assets and owner positions, while § 23 accepts neutral
  `AblInvesteringsselskabsaktiv` or `AblMinimumsinstitutsbevis` values carrying
  those classification inputs. This is the preferred workbook boundary:
  genuine lists can become related sheets, enum facts can become dropdowns,
  and no user is asked to choose the legal status that the rules calculate.
  The recursive >=25% look-through should remain a domain relationship rather
  than being flattened into caller-supplied percentages or totals.

- `KildeskatESkattekortInput` and the generated-card result may eventually
  share smaller card-period and withholding-percentage objects, but the current
  BEK 1094 slice keeps the annual 2026 percentage derivation as its own domain
  object rather than forcing a broader card refactor prematurely.
- `ArbejdsmarkedsbidragUdvidetLønmodtagerInput` and
  `ArbejdsmarkedsbidragVirksomhedsordningInput` are wide, but they still mirror
  dense statutory enumerations closely enough that premature grouping could hurt
  source traceability. § 2, stk. 2 naturalia now has a smaller domain object
  because the statute enumerates closed benefit categories separately from the
  ordinary wage-post list. § 3 likewise has a result object, while retaining the
  five explicit input amounts because they map directly to nr. 1-5.

## Now

- ABL § 39 A's årlige oplysningspligt kan nu skelne mellem en manglende
  indgivelse og en kildehistorik, der endnu ikke rækker forbi fristen. En typet
  opgørelsesdato og højst én dokumenteret fristudsættelse pr. indkomstår
  bestemmer den effektive frist; fredag, lørdag og søndag flyttes efter lovens
  kalenderregel. Forløbet kontrolleres år for år, så den første manglende
  indgivelse udløser det eksisterende § 39 A-forfald, før senere hændelser
  genafspilles. Den afledte hændelse og årets kontrol bevares i resultatsporet,
  mens selve fristudløbet eller en juridisk konklusion ikke kan indsendes som
  input. Fjorten fokusscenarier dækker blandt andet fristdagen, dagen efter,
  individuel udsættelse, eksplicit for sen indgivelse, flerårshuller,
  tilbageflytning og dødsfald. Den kanoniske JSON/XLSX-grænse udstiller
  opgørelsesdatoen og et særskilt underark for fristudsættelser.
- Personskattelovens § 8 a, stk. 5-6, har nu et typet flerårsledger for
  negativ aktieindkomstskat. Åbningen bevarer ejer, oprindelsesår og enten et
  valideret foregående Personskat-resultat eller myndighedsproveniens; årets
  negative skat anvendes før tidligere fremførsel, og ældste tranche anvendes
  først. Single-, ægtefælle- og rolleombytningsforløb samt ugyldige år,
  dubletter og ubalancerede resultater er dækket i interpreter og compiler.
  Beregningsmetadata giver danske labels til åbningsvalg, trancher og
  dokumentreferencer. En vedvarende JSON/XLSX-regression viser, at en ekstern
  1.000-kr.-tranche fra 2024 reducerer 2026-skatten med præcis 1.000 kr. og
  efterlader en tom ultimoliste.
- Ligningslovens § 9, stk. 1, er nu et typet årsdomæne for dokumenterede
  lønmodtagerudgifter. Hver udgift har stabil identitet, arbejdsforhold,
  indkomstår, egenbetaling og en kildefaktabåret art; reglerne klassificerer
  kursus, faglitteratur, særligt arbejdstøj, erhvervstelefoni, arbejdsværelse,
  driftsmiddel, repræsentation og andre arbejdsudgifter uden et råt
  fradragsberettigelsesfelt. Den officielle 7.300-kr.-grænse for 2025 og
  7.600-kr.-grænse for 2026 anvendes én gang på årets samlede grundlag, mens
  § 9, stk. 3, begrænser repræsentation til 25 pct. før grænsen.
  Den kanoniske Personskat-graf samordner hver identificeret udgift med
  Sømandsbeskatningslovens § 4 før årsgrænsen og afviser manglende eller
  modstridende arbejdstilknytninger. Fokusscenarierne passerer i både
  interpreter og compiler, og beregningsskemaet udstiller de relaterede
  udgiftsrækker med danske spørgsmål og feltnavne.
- Sømandsbeskatningslovens §§ 5-8 og bekendtgørelse nr. 940 af 2022 er nu
  modelleret som typede person-, skibs-, anvendelses-, arbejdsrolle- og
  lønfakta. Reglerne afleder DIS frem for at modtage en juridisk konklusion,
  skelner ordinær DAS-løn og uafklaret nettoløn, anvender 50 pct.-grænsen for
  bugsering og bjærgning og håndterer personkredsene i §§ 7-8. DIS-løn indgår
  i personlig indkomst og progression uden AM-bidrag; lempelsen fordeles på
  skattekomponenterne før personfradrag og trækkes fra den kanoniske slutskat.
  PSL § 13, stk. 5, og LL § 9 A-udelukkelsen afledes fra samme resultat.
  Fortolkede og kompilerede fokusscenarier passerer, og den komplette
  JSON/XLSX-rundtur bevarer en 500.000-kr.-DIS-post og giver samme resultat på
  begge grænser.
- Validerede `@ calculate`-kontrakter har nu en vedvarende,
  indholdsadresseret cache. Nøglen omfatter compilerbinæren, prelude-valget,
  rodfilen og alle transitive almindelige, kvalificerede og hash-baserede
  importer. Fejl cachelagres ikke, korrupte poster ignoreres, og miljøvariable
  kan deaktivere eller spore cachen. På den aktuelle Personskat-graf faldt
  skemakontrollen fra 115,43 s koldt til 12,31 s varmt, en varm XLSX-skabelon
  tog 16,45 s, og den komplette arbejdsbogstest faldt fra 1.251,35 s til
  557,11 s og passerede. Den resterende hovedomkostning er initialisering og
  fortolket udførelse, som er afgrænset i `td-783a9c`.
- `check`, native `run` og native `build` har nu en særskilt,
  afhængighedskomplet artefaktcache over rodfilen, alle transitive importer,
  importopløsning, manifest, compiler og Rust-værktøjskæde. På den kanoniske
  Personskat-graf tog en kold kontrol 248,6 s efter en grafændring, mens den
  uændrede kontrol tog 2,6 s; et uændret kompileret LL § 9-scenarie tog 2,89 s.
  Næste målte trin er modulvis frontendcache for ændrede grafer i `td-60a9d6`
  og sikker memoisering af rene globale værdier i interpreterens rodevaluering
  i `td-38f074`.
- Store beregningsbatches udfører nu uafhængige sager på isolerede workers og
  samler resultater og diagnostik i den oprindelige rækkefølge. En særskilt
  vedvarende `corpus`-profil optimerer compiler, interpreter og valgte
  integrationstests uden at gøre den almindelige Rust-udviklingsprofil dyrere
  at genbygge. På den kanoniske Personskat-rod faldt en ukachet frontendkontrol
  fra 8,91 s til 2,20 s internt, og den varme komplette XLSX-regression faldt
  fra 189,69 s til 50,54 s. Det næste strukturelle trin er fortsat modulvise
  FIR/Rust-enheder for ændrede importgrafer i `td-170e07` og `td-60a9d6`.
- Personskattelovens § 13-fremførsel modtager ikke længere et råt beløb sammen
  med skatteyderens egen juridiske konklusion om, at underskuddet ikke kunne
  rummes tidligere. Åbningsgrundlaget er nu enten neutralt, det konsistente
  årsresultat fra det umiddelbart foregående Personskat-år eller et eksternt
  fastsat underskud med år og typet proveniens fra årsopgørelse,
  Skatteforvaltningens afgørelse eller en anden navngiven myndighed. Reglerne
  afviser fremtidige og oversprungne år, tom dokumentreference og ubalancerede
  årsresultater. De anvender først fremførslen i egen skattepligtig indkomst,
  derefter årets nye underskud efter § 13's egen skatte- og
  ægtefællerækkefølge, og udleder selv ultimo til næste år. Et fortolket og
  kompileret flerårsscenarie overfører 79.700 kr. til ægtefællen, fremfører
  33.283 kr. fra 2025 og anvender hele beløbet i egen indkomst i 2026.
  Kontraktens direkte XLSX, direkte JSON og JSON-hydrerede XLSX giver identiske
  resultater for både et eksternt 40.000-kr.-grundlag og et tidligere
  30.000-kr.-årsresultat. Arbejdet afdækkede samtidig, at den ugyldige form
  `? assert(udtryk)` tidligere blev læst som et ukendt bevismål og tavst sprang
  kontrollen over. Futuruna afviser nu ukendte `?`-mål under typekontrol og ved
  ukontrolleret fortolkning, kræver boolske invariantprædikater og lader
  `assert(Falskt)` fejle hårdt i både fortolket og kompileret kørsel;
  scenarierne bruger sprogets egentlige navngivne `|`-invarianter.
- Ligningslovens § 13 og Pensionsbeskatningslovens § 49, stk. 2, modtager nu
  identificerede udbetalingsfakta i den ordinære personlige indkomst. Hver post
  bevarer udbetalerens retlige art, indkomstår, beløb og den relevante
  medlems- eller skatteyderrelation efter PBL § 55. Ligningslovens §§ 30-31 er
  en fælles typet undtagelseskaskade med eksplicitte betingelser for behandling,
  lægeerklæring, medicinperiode, uddannelsesformål, udgiftstype, fri kost,
  transport og AUB-ændringens virkning fra 1. juli 2026. Ordinære beløb går
  præcis én gang til personlig indkomst uden et nyt AM-bidrag; gyldige
  undtagelser bliver skattefri, og uidentificerede sammensatte produkter fejler
  lukket. Beregningsmetadata udstiller udbetalingstabellen og 48 genbrugelige
  feltdefinitioner med danske spørgsmål, enheder og sporbare lov-, ændrings-,
  sats- og vejledningskilder. Fokus- og slutscenarier dækker skattepligtige og
  skattefri grene, forkert yderkreds, PBL § 55 og dubletbeskyttelse. En
  hydreret projektmappe med en skattepligtig foreningsydelse går tabsfrit
  gennem JSON-XLSX-kaldet og giver samme kanoniske Personskat-resultat.
- Personskattelovens § 4, stk. 2-3 modtager nu hver kapitalomkostning som et
  identificeret kildefaktum med anvendelsesår, lovbestemt anvendelse,
  omkostningsart efter Ligningslovens § 17 C, egen næringsstatus og beløb.
  Reglerne afleder selv, om omkostningen vedrører årets kapitalindkomst, om den
  tjener erhvervelse, sikring og vedligeholdelse af kapitalindkomst, om § 17 C
  afskærer fradraget, og om § 4, stk. 3 flytter det til personlig indkomst.
  Afviste og ugyldige beløb bevares særskilt i resultatsporet; blanke eller
  dobbelte identifikationer kan ikke lække ind i beregningen. Den kanoniske
  Personskat-kontrakt og dens relationelle projektmappe udstiller de samme seks
  felter med danske spørgsmål og kildehenvisninger. Det fokuserede scenarie
  dækker alle tre formål, alle fem § 17 C-kategorier, næringsomklassifikation og
  ugyldige fakta. Det kanoniske scenarie passerer alle 27 invarianter i både
  fortolket og kompileret udførelse, og skemaudtrækket bevarer felterne og deres
  kildeankre.
- Denne model afdækkede to generelle sprogfejl, som nu er rettet med
  regressionstest: RuleScope-kald skelner mellem lokale og globale regler efter
  aritet, og postfix-kald binder korrekt inden for præfiksoperatorerne `!`, `-`,
  `&` og `&mut`. Rettelserne er generelle og ikke særlige for skattelovgivning.
- Kursgevinstlovens § 32 er nu ført ind i den kanoniske Personskat-beregning
  som kildedata frem for færdigklassificerede skattebeløb. Identificerede
  kontrakter, ABL-aktiver og ordnet årshistorik afleder årets gevinst, tab,
  aktiemodregning, ægtefællevirkning og fremførsel præcis én gang. De samme
  kildedata og fulde resultater afstemmes mellem den relationelle XLSX-kontrakt
  og JSON-kontrakten. Blandede ABL-klasser kræver fortsat en særskilt,
  kildebelagt allokeringsregel og fejler derfor lukket. Det samme gælder KGL
  § 33-kontrakter med negative dagsværdier, indtil `td-89a28a` har erstattet
  den nuværende ikke-negative kildeadapters afgrænsning.
- Ejendomsavancebeskatningslovens § 10, stk. 5-9 bruger nu identificerede
  genopførelsesejendomme frem for et løst antal. Reglerne validerer grund,
  meddelelse, entydige identifikationer og positive udgifter, fordeler
  stk. 7- og stk. 8-beløbene efter de faktiske udgiftsandele og afstemmer
  afrundingsresten. Et ejendomsspecifikt genanbringelsesresultat sender kun den
  valgte ejendoms aktive anskaffelsessumsnedslag videre til et senere salg.
  Stk. 9 holder ejerboligens anvendte bruttosum uden for både den ordinære
  avance og § 6-tabsmodregningen, men fører den særskilt til § 4-kapitalindkomst
  uden anskaffelsessumsfradrag. Ejerboligstatussen gælder udtrykkeligt ved
  genopførelsen, og medregningsåret afledes af genopførelsesåret. Et senere
  klassifikationsskifte bærer i stedet ændringsdatoen og, for § 9, boligandelen
  før og efter ændringen. §§ 8, stk. 5, og 9, stk. 4 afleder virkningen,
  boligandelsforøgelsen og overgangsgrænsen den 15. december 2005 fra disse
  fakta. AL §§ 21/24-sporet og den kanoniske Personskat-integration bevarer
  begge beløb. De fokuserede fordelings-, stk. 9-, klassifikationsskifte-,
  afskrivnings-, audit- og kanoniske scenarier passerer; den relationelle
  XLSX/JSON-grænse afstemmer de nye dato- og promillefelter.
- Pensionsbeskatningslovens § 53 A har nu et kildefaktabåret omfang og et
  dateret flerårsforløb. Reglerne afleder alle ni hjemler i stk. 1,
  udelukkelsen efter § 53 B, undtagelserne i stk. 4 og
  statsstøttebegrænsningen i stk. 6 fra produkt-, ejer-, oprettelses-,
  finansierings- og indbetalingsfakta. De afleder også delårsperioder ved ind-
  og udtræden af skattepligt, depotgrænser ved etablering og ophør af
  direktørsikkerhed, eksakte andele for flere berettigede, det historiske
  metoderegime til og med 2009, det bindende moderne metodevalg fra 2010,
  dateret negativ fremførsel med femårsgrænsen for saldi fra 2001 og tidligere
  samt fristen for dokumenterede udbetalinger til afkastskat. Ufuldstændige
  eller modstridende fakta fejler lukket. De 18 omfangsinvarianter og 11
  tidsinvarianter passerer både interpreter og compiler. Ni særskilte
  overgangsinvarianter dækker 2001, 2002, 2009, 2010 og dokumenteret
  åbningshistorik i begge udførelsesveje. Elleve yderligere invarianter afleder
  moderne eller historisk regime fra oprettelse, almindelige erhvervelser og
  kvalificerende arv og afviser ugyldige erhvervelsesforløb. Fjorten yderligere
  invarianter dækker bindende valg af § 53 A eller § 53 B, ikrafttrædelsen den
  22. december 2004, den ordinære 2006-frist, særvirkningen fra 1. januar 2004, senere fuld skattepligt, første
  valgmulighed ved arv, modtager og modstridende valg. Fem yderligere
  invarianter dækker den tidligere blanket 49.020 uden målinput, afleder både
  § 53 A og § 53 B fra ordningens øvrige fakta, respekterer en påberåbelse af
  intet valg og afviser både manglende påberåbelse og den nye blanket i det
  historiske spor. Ti fokuserede invarianter
  dækker erhvervelse 1. januar og midt i året, tre på hinanden følgende
  rettighedshavere, halvåbne perioder, dubletter, brudte kæder og en fuld
  beregning, hvor DKK 130.000 i erhvervelsesværdi og betalinger efter
  erhvervelsen giver DKK 29.000 i skattepligtigt afkast. Den tidligere ejer
  får samtidig en separat fuld beregning på DKK 20.000 frem til det eksakte
  overdragelsestidspunkt. En typet reference til erhvervelsen fordeler desuden
  DKK 29.000 mellem samtidige berettigede med 40.000/160.000 og giver DKK
  7.250 til den valgte skatteyder. En omvendt oplyst
  grænsehændelsesliste fejler desuden lukket, selv om de afledte
  rettighedsgrænser sorteres ved sammenfletningen. Den kanoniske kontrakt
  har 1.605 feltmetadata-poster i alt, heraf 261 § 53 A-poster, og fjorten
  relationelle § 53 A-ark; XLSX og JSON afstemmer samme tre ordninger, en senere
  erhvervelse med en fuld rettighedsovergang, et dateret overgangsvalg, en
  afvist ny blanket i det historiske spor og fem årsoptegnelser. Den kanoniske
  JSON-grænse dækker desuden en accepteret tidligere blanket, hvor § 53 A
  afledes uden et målinput. XLSX-grænsen dækker derudover en særskilt
  overdragersag, der afleder perioden og kapitalposten fra de relationelle
  erhvervelses-, års- og betalingsfakta.
- Seksten fokuserede kontraktændringsinvarianter dækker hele og delvise nye
  ordninger, den historiske rest, uændrede kontrakter, bindende
  forhåndsaftaler, overførsler, genoptagelser, ukendte ændringsarter,
  dubletter og kronologi i både interpreter og compiler. Den kanoniske
  projektmappe bærer en uvæsentlig dateret kontraktændring som relationelle
  kildefakta. JSON-grænsen accepterer en materiel overgang til
  markedsrentevilkår og afviser en ikke-understøttet anden ændringsart. Syv
  yderligere invarianter beregner nu en hel ordning og en særskilt ny
  kontraktdel fra kontraktændringens eksakte virkningstidspunkt. DKK 125.000 i
  kapitalværdi for hele ordningen giver DKK 34.000 i afkast efter grænsen,
  mens en ny kontraktdel med nul i begyndelsesværdi giver DKK 18.000. Betalinger
  før grænsen, også tidligere samme dag, medregnes ikke; den historiske rest
  forbliver uden for § 53 A. Kapitalværdien er et typet kildefaktum for enten
  hele ordningen eller den nye del. En forkert rækkevidde eller en uoplyst værdi
  fejler lukket i både interpreter og compiler.
- `pensionsbeskatningsloven-aarsparametre.runa` adskiller nu regulerede
  årsbeløb fra den juridiske struktur i §§ 16 og 18. Fire typede kildeankre
  dækker den ufuldstændige 2002-2009-tidsserie og de fulde tidsserier for
  2010-2017, 2018-2024 og 2025-2026. Alle værdier fra 2010 til 2026 har
  fokuserede interpreter- og compiler-scenarier, og § 18-årsresultatet afviser
  2009 og 2027 med nul fradrag. `Pbl16FradragsloftResultat` bevarer det valgte
  parameterobjekt, så år, kildeserie og de tre relevante beløbsgrænser er
  synlige i beregningssporet.
- Pensionsbeskatningslovens § 15 B står nu med ordret lovtekst, gældende
  Retsinformation-kilde og officielle SKM-tidsserier for 2010-2026 i et
  særskilt modul. Kalderen leverer identificerede indkomstposter, ordninger,
  udbetalingsplaner, faktiske rater, historiske indbetalinger og årets
  indbetalinger. Reglerne afleder selv sportsindkomsten, alders- og
  påtegningskravet, den aktive eller afsluttede udbetalingslivscyklus, pausen
  under udbetaling, kravet om højst én ordning, den resterende regulerede
  livstidsgrænse og fordelingen mellem eget fradrag og arbejdsgiverens
  bortseelsesret. § 11 A, stk. 2 og 3, beregner både division efter primoværdi
  og annuitetsraten med den lovbestemte maksimale amortisationsrente. § 20
  udleder skattepligtig og personlig indkomst samt undtagelserne i stk. 3 og 4,
  mens overskridelser af § 15 B-loftet går til 60 pct. afgift efter § 29 og
  afrunding efter § 36. Arbejdsmarkedsbidrag holdes uden for både historikken
  og den aktuelle tildeling. § 18 forbruger indbetalingsresultatet uden at
  lægge sportspensionen under det almindelige § 16-rateloft. Den kanoniske
  Personskat-sag bruger aktuelle udbetalinger som personlig indkomst,
  foregående års § 20-resultat i Ligningslovens § 9 L og årets § 29-afgift i
  årsopgørelsen. De fokuserede interpreter- og compiler-scenarier dækker
  gyldige, overgrænse-, flerårige og ugyldige forløb samt isolering af
  fremtidige fakta. Det kanoniske pensionsscenarie passerer desuden 13
  end-to-end-invarianter i interpreteren.
- Pensionsbeskatningslovens §§ 15 og 15 A står nu med ordret lovtekst og typede
  kildeankre i et særskilt juridisk modul. § 15 A-modellen håndterer
  alderskravet, ti kvalifikationsår inden for de seneste femten år, succession,
  den passive kapitalprøve, 2013/2014-overgangen, ændringen for aktiv udlejning
  fra 2025, tilladte ordningsformer, hver afståelses fortjenesteloft og den
  fælles tiårige indbetalingshistorik. De tre seneste regnskabsperioder har
  eksakte start- og slutdatoer, skal være sammenhængende og slutte umiddelbart
  før den aktuelle periode. Derfor anvendes 75-procentsgrænsen præcist, når en
  af perioderne begyndte før 1. januar 2012, og overdragelsesgrænsen skifter
  præcist efter 31. december 2013. Direkte og indirekte ejerandele afledes fra
  typede ejerveje. Ved mindst 25 pct. udelades aktiernes værdi og afkast, og den
  forholdsmæssige andel af selskabets underliggende indtægter og aktiver
  medregnes uden dobbeltregning. Aktiv udlejning under ejergrænsen gennemlyses
  først fra 2025, mens koncernintern lejeindtægt udelades, og en ejendom, som
  lejeren bruger i driften, behandles som aktiv. Hver ordning henviser til den
  afståelse, der oprindeligt oprettede den. En ny ordning skal være oprettet
  senest året efter denne afståelse, mens en allerede gyldig ophørspension kan
  modtage fortjeneste fra en senere kvalificerende afståelse. Det regulerede
  samlede maksimum er 3.285.400 kr. for 2025 og 3.443.400 kr. for 2026 og
  afledes af det seneste indbetalingsår. Det særskilte fradragsloft i
  afståelsesåret følger derimod afståelsesårets beløbsgrænse, også når betalingen
  sker senest 1. juli året efter. Ikke-understøttede år fejler lukket. Flere
  ordninger fordeles deterministisk i inputrækkefølge uden genbrug af fortjeneste
  eller loft.
  Afståelsesfakta modtager ikke længere et beregnet fortjenestebeløb. Et
  særskilt, typet kildegrundlag eksekverer de relevante resultater fra
  Afskrivningsloven, Ejendomsavancebeskatningsloven,
  Aktieavancebeskatningsloven og Kursgevinstloven, bevarer hvert delresultat og
  modregner kun de tab, som kildelovens eget resultat har gjort
  fradragsberettigede. Hovedaktionærgrenen kræver faktiske ABL-afståelser, og en
  ægtefælles fortjeneste føres gennem en udtrykkelig KSL § 25 A, stk. 1-, 3-
  eller 8-henføring. Fire fokuserede scenarier dækker egen virksomhed,
  hovedaktionæraktier, ægtefællehenføring og blandede gevinster og tab.
  Interpreter og compiler gennemfører samme 27 fokuserede scenarier for
  almindelig ratepension, gyldig og ugyldig indeksordning, rettidig og for sen
  oprettelse, genbrug af en eksisterende ordning, flere § 15 A-ordninger,
  overgrænse, ugyldige fakta, tidligere udnyttet loft, datogrænser,
  periodehuller, næringsundtagelsen, direkte og indirekte
  selskabsgennemlysning, særskilte lige ejerveje, dublerede ejerveje samt
  udlejningsundtagelserne.
- Den kanoniske beregningskontrakt udstiller kun disse kildefakta. De nye
  pensionsstier har hver en eksplicit dansk etiket, et interviewspørgsmål,
  hjælp, relevant enhed og typede kilder. `@ calculate("Dansk personskat")`
  navngiver selve beregningen og arbejdsbogen; det er ikke en erstatning for
  feltmetadata. De stabile maskinstier ligger i den skjulte kontrakt. Det gør
  arbejdsbogen velegnet som et struktureret udvekslingsformat, hvor en AI kan
  interviewe borgeren og udfylde fakta, mens Futuruna alene validerer,
  beregner og forklarer resultatet deterministisk.
- `personskat.calculate.runa` forbinder nu den kanoniske borgergrænse med
  kildefakta for erhvervsmæssig befordring efter Ligningslovens § 9 B,
  befordring efter §§ 9 C/9 D, almindelige renter,
  §§ 6/6 A-fradrag, ejendomsdrift efter Personskattelovens § 4, stk. 1, nr. 6,
  § 4, stk. 2-omkostninger, egne og en samlevende ægtefælles ejendomsafståelser,
  ordinære ABL-hændelsesforløb og særlige ABL-aktiver.
  Ejendomsafståelsernes skatteår, § 9 C's skatteår og aftrapningsindkomst samt § 9 D's
  juridiske resultat afledes internt og er derfor ikke borgerfelter.
  EBL § 6's ægtefællepar-regel anvender begge ægtefællers egne tab først og
  begrænser derefter overførslen til modtagerens resterende nettofortjeneste.
  De juridiske PSL § 4-/§ 4 a-poster afledes internt og bevares i resultatet;
  kalderen leverer hverken beregnede skatteposter eller indkomstkategorier.
  Ugyldige kildekæder giver ingen skattemæssig virkning og er fortsat synlige
  som ugyldige. Personlig indkomst fra § 4, stk. 3 og ABL er særskilt fra
  lønnen, så AM-grundlag og lønmodtagerfradrag fortsat alene bruger bruttolønnen.
  Fri befordring efter § 9 C, stk. 7 føres tilsvarende til personlig indkomst
  uden at blive gjort til AM-bidragspligtig løn.
- Skattepligtig § 9 B-godtgørelse er derimod løn og udvider derfor det
  AM-bidragspligtige løngrundlag. Et direkte § 9 B-fradrag efter
  Personskattelovens § 3, stk. 2, nr. 8 trækkes særskilt fra personlig indkomst.
  Kørslens år, personrolle og lovbestemte fradragsmetode afledes internt. Den
  årlige grænse accepterer nul eller flere entydigt identificerede sager i en
  fortløbende kronologisk rækkefølge. Den udleder hver arbejdsgivers
  kilometerhistorik og det fælles direkte-fradragsforløb i stedet for at modtage
  beregnede startkilometer. Hvis blot én sag er ugyldig, rækkefølgen har huller,
  eller identifikationerne ikke er entydige, er alle samlede § 9 B-beløb nul,
  mens delresultaterne forbliver synlige.
- Det genererede Personskat-regneark afledes nu fra en kontrakt med 971 nåbare
  definitioner og 1.605 eksplicitte menneskelige feltmetadata-poster.
  XLSX/JSON-
  roundtrip fastholder den almindelige København-beregning, årsopgørelsen, en
  kildebaseret § 17-gevinst, rente-/fradragssagen med en relateret
  kapitalomkostning, et kildefaktabåret § 9 C-befordringsfradrag og en
  skattefri ABL § 15-afståelse med boligret. Ejendomsavancesagen fører samtidig
  en tidligere § 6 A-fortjeneste gennem § 8, stk. 5: boligejendommens egen
  fortjeneste på 190.000 kr. er skattefri, mens den gamle genanbragte
  erhvervsfortjeneste på 190.000 kr. beskattes. Efter eget tab, fremført tab og
  ægtefællens overførte tab afledes fortsat 65.000 kr. i ejendomsavance; renter
  og øvrige fradrag giver derefter 75.000 kr. i nettokapitalindkomst.
  En fjerde sag fører en 50 pct. ejerandel, en delafståelse, en historisk
  mælkekvote og en § 5, stk. 6-overførsel gennem både XLSX og JSON. Den afleder
  41.250 kr. i årligt § 5-tillæg, 20.000 kr. i mælkekvoteforhøjelse,
  30.000 kr. i mælkekvotenedsættelse, 37.500 kr. i § 5, stk. 6-overførsel og
  en reguleret anskaffelsessum på 268.750 kr. En femte sag udfylder
  sælgerpantebrevets tiårige afdragsplan, KGL-kildefakta og 2026-årsforholdet
  gennem XLSX og afstemmer mod kanonisk JSON; begge adaptere medregner 300.000
  kr. fra en ejendomsafståelse i 2025 og 75.000 kr. i kursgevinst, i alt 375.000
  kr. i kapitalindkomst. En sjette sag rekonstruerer en tidligere § 6 A-
  genanbringelse ved en § 11-afståelse. XLSX og kanonisk JSON lader begge det
  gamle anskaffelsessumsnedslag på 200.000 kr. bortfalde og medregner den gamle
  fortjeneste på 200.000 kr. præcis én gang i kapitalindkomsten. En ottende sag
  sender faktiske oplysninger om en landzoneejendom og et driftsoverskud på
  25.000 kr. gennem begge adaptere; hjemlen og kapitalindkomsten afledes af
  reglerne.
  En niende sag fører et ordnet A/B/A-forløb gennem den nøglebundne § 9 B-tabel.
  Arbejdsgiver A har først 19.500 kilometer, arbejdsgiver B derefter 1.000 og
  arbejdsgiver A til sidst yderligere 1.000. Reglerne udleder startkilometrene
  0/0/19.500, holder arbejdsgivernes grænser adskilt, summerer 83.880 kr.
  skattefrit og 400 kr. som AM-bidragspligtig løn og giver samme fulde
  beregningsspor fra XLSX og kanonisk JSON.
  Feltmetadataen og interviewspørgsmålene dækker nu også genanbringelsesvalg,
  centrale
  §§ 6 A/8/9-kildefakta, en ordinær ejendoms aktive
  anskaffelsessumsnedslag, kontrolophør, delafståelsernes særskilte
  anskaffelsessumsgrundlag, mælkekvoter og § 5, stk. 6 for begge ægtefæller.
  KGL-posterne navngiver pantebrevslisten, pantebrevsidentiteten,
  skatteyderfakta, § 14-grundlaget og senere dispositioners år, berørte
  tranche, art, modtagers nye tranche, hovedstol og provenu. Den navngiver nu
  også beregningens person, pantebrevets
  oprindelige skatteyder og alle fakta i de understøttede ægtefælle- og
  dødsboskifter.
  Otte af posterne beskriver ABL § 15's udsteder og værdipapir: selskabsform,
  foreningstype, dansk skattemæssigt hjemsted, SEL § 3-undtagelse,
  Fondsbeskatningsloven og ABL-status. Reglerne afleder herfra, om udstederen
  er et selvstændigt skattesubjekt, og holder denne klassifikation synlig i
  beregningssporet.
  Heraf beskriver 66 poster § 11, stk. 2-valget og en eventuel ny
  genanbringelse for personen eller ægtefællen.
  Sportspensionsgrenen tilføjer tre relationelle kildetabeller for
  sportsindkomst, ordningsfakta og tidligere indbetalinger. Hver relevant sti
  har en dansk etiket og et interviewspørgsmål, mens forbindelsen fra årets
  pensionsindbetaling til den konkrete ordning bevarer en stabil
  maskinidentifikation.
  Store enum-/variantvalg bruger et
  skjult `_choices`-ark med navngivne områder, så alle domænevalg kan blive
  dropdowns uden Excels 255-tegnsgrænse for indlejrede lister.
  Arbejdsbogens domænekolonner bruger præcise feltmetadata, hvor de er
  oprettet. Alle 64 nåbare § 15 A-stier for afståelsen har nu en eksplicit
  etiket og et interviewspørgsmål; ældre, dybe stier med deterministisk læsbar
  fallback er fortsat eksplicit opfølgningsarbejde. XLSX-kontrakt v6 viser og
  validerer samtidig beregningstitlen på hvert synligt ark.
- Ejendomsavancebeskatningslovens § 6 D er nu et vedvarende, typet
  sælgerpantebrevsforløb. Den kanoniske EBL-beregning afleder den
  kvalificerende erhvervsfortjeneste og 10-procentsgrænsen fra ejendommens
  § 8/§ 9-behandling; borgeren eller en interviewende AI leverer ikke disse
  juridiske delresultater. Valget prøver partrelation, erhvervsanvendelse,
  pantebrev, meddelelse, sikkerhed og placering, hvorefter lige årlige beløb og
  lovbestemte fremrykningshændelser føres på tværs af indkomstår. Hvert år
  indeholder en kronologisk liste af betalinger og hændelser med stabile
  pantebrevsdel-identiteter. Manglende år, uafklaret rækkefølge og
  ikke-understøttede hændelser afvises frem for at blive udfyldt ved gæt.
  Personskat kan derfor medregne § 6 D-beløbet fra en tidligere afståelse i
  det aktuelle skatteår. Den særskilte KGL-bro matcher pantebrevet ved identitet,
  frigiver kontantværdien som anskaffelsessum i takt med hovedstolen og danner
  årets Personskattelov § 4, stk. 1, nr. 2-poster efter § 14-grænsen. EBL's
  fremrykning bruger den berørte restgæld, mens KGL-resultatet bruger det
  faktiske provenu. Uoverensstemmende identitet, manglende årsforløb,
  dobbeltbeskatningsoverenskomst og ikke-understøttede modtagere får ingen
  skattemæssig virkning og forbliver synligt ugyldige. En dansk ægtefælle, et
  dødsbo og en længstlevende ægtefælle kan derimod overtage positionen med
  bevaret grundlag og erhvervelsesår, hvor loven foreskriver succession. Boets
  udlodning prøver de faktiske betingelser i dødsboskattelovens §§ 28, 36 og 38.
  Et tab eller et valg af salgsbeskatning udløser realisation hos boet til
  boopgørelsesværdien, og ægtefællen fortsætter fra det nye grundlag. EBL's og
  KGL's realisationsudfald sammenlignes disposition for disposition.
  Et delvist ejerskifte opdeler hovedstol og anskaffelsessum i entydigt
  identificerede trancher uden dobbeltregning. Fordelingen skal kunne ske
  præcist i hele kroner; ellers afvises dispositionen. KGL kan derefter føre
  hver ejers betalinger videre gennem den samme ordnede EBL/KGL-postrække.
  Personskat filtrerer på skatteyderidentitet og personrolle. Den årlige
  udskudte EBL-fortjeneste følger nu den samme delte ejerposition: et verificeret
  30-procents ægtefælleskifte fordeler 90.000 kr. til ægtefællen og 210.000 kr.
  til sælgeren, og de to Personskat-grene afstemmer til 300.000 kr. Udelelige
  kronefordelinger afvises fortsat.
- Personskattelovens § 4, stk. 1, nr. 5 b og stk. 6 forbruger nu den samme
  kildeafledte ABL-aktivklassifikation som § 4, nr. 5, § 4 a og KGL § 32.
  Det afledte resultat bevares i PSL-resultatet, så audits og det kommende
  samlede regneark kan spore § 19 C-/§ 22-status og § 17-modprøven tilbage til
  faktainputtet. Ugyldige direkte klassepåstande afvises frem for at glide ind
  i kapitalindkomsten.
- Aktieavancebeskatningsloven §§ 19, 19 A, 20 and 20 A now derive their legal
  classifications from typed source facts. The § 19 model covers UCITS,
  repurchase obligations, effective participant counts, securities ratios,
  exact ownership look-through and the employee-company exception. § 23
  consumes the derived result, including § 20 A's loss restriction, rather
  than accepting a caller-selected legal label.
- Aktieavancebeskatningsloven §§ 19 B-22 now derive investment status from
  annual-average asset facts, KGL underliers, exact capital-unit ownership,
  election/reporting dates and nested owner positions. § 23, PSL § 4/§ 4 a
  and KGL § 32 consume the effective result. Raw § 19 B, § 19 C and § 22
  constructors have been removed from the § 23 boundary, and full product
  equality rejects tampered or wrong-year chains.
- Aktieavancebeskatningsloven §§ 6-7 now derive a source-backed taxpayer result
  from the relevant underlying liability ground. § 17, § 23, § 9 and their KGL
  and Personskatteloven bridges consume and integrity-check the same result;
  no raw § 6/§ 7 caller label remains at those boundaries.
- Personskatteloven § 13 a now consumes a closed union of source-law annual
  results instead of `skyldner_tab_kroner`. The EBL § 6 dependency calculates
  its own current and carried losses, own-gain offset, spouse transfer and
  remaining carry-forward. The focused audit proves a 30.000 kr. ABL/KGL/EBL
  total and rejects a 7.000 kr. future-year result in both backends.
- `personskat.calculate.runa` exposes one aggregate
  `beregn_personskat(PersonskatInput) -> PersonskatBeregningResultat` boundary.
  The graph contains ordinary wage-earner facts, source-fact Ligningslov § 9 B
  business travel, interest and §§ 6/6 A inputs, source-fact ABL annual inputs, explicit variants
  for special tax conditions and § 13 deficit/spouse conditions, and an
  optional Kildeskattelov annual assessment. `runa schema`,
  JSON/TOML/XLSX templates and `runa call` carry the same source-linked
  contract. XLSX schema v6 creates related child worksheets only for genuine
  `List`, `Map` or `Set` fields; the ABL collections now prove that behavior in
  the aggregate itself. Remaining workbook gaps are therefore the remaining
  source-law branches that have not yet reached `PersonskatInput`, not
  hand-authored workbook topology.
- The 2026-08-01 calculation-boundary audit now inventories every public
  `@ calculate` entry with `runa audit --entry ... --json`. The four entries in
  `investeringsklassifikation.calculate.runa` accept typed source-fact graphs
  and derive their legal classifications: § 19 reaches 45 callable families,
  §§ 19 A-20 A reach 80, §§ 19 B-19 C reach 65, and §§ 21-22 reach 65. Their
  large `not_reached` sets are expected because the shared imported corpus is
  broader than each focused classifier.
  The canonical `beregn_personskat` entry loads 65 source modules and reaches
  3.060 of 7.934 registered callable families. These counts are inventory
  evidence, not a completeness percentage: a reached RuleScope conservatively
  marks all its members reached, and `not_reached` means review candidate rather
  than proof of impossible dynamic or external invocation.
  The review classifies ABL § 9's company portfolio ledger, advance
  registration, withholding regulations, final-settlement helpers and terminal
  provisions as intentionally separate calculation/workflow boundaries. It
  identified material Personskat composition gaps for ABL § 5 A
  (`td-6766a4`) and paired § 13 A loss offsets (`td-ed02d8`), which are now
  implemented and awaiting independent review. Subsequent slices have also
  closed the richer § 17 source model (`td-699438`), §§ 35 G-35 K
  (`td-0b0a4b`) and the canonical §§ 37-40 lifecycle (`td-3681f7`). It also
  confirms public caller-derived current-law values in the residual KGL annual
  net (`td-5beddd`), untyped capital expenses (`td-151941`) and the PSL § 13
  deficit-eligibility flag (`td-292327`). Fixtures remain deliberately outside
  the canonical graph.
- The first material reachability gap is now closed: ordinary
  `AblOrdinærAfståelse` loss events carry typed § 5 A source facts, derive their
  own gross loss and § 5 A result, and expose both the reduction and remaining
  loss. §§ 13, 13 A and 14 only see the remaining loss. Missing source facts
  reject a taxable loss before annual aggregation, while gains and § 15-exempt
  losses use an explicit no-facts variant. The same graph is reachable from
  `beregn_personskat`, and 17 typed field references give its nested workbook
  inputs Danish labels, interview questions and the § 5 A source. The generated
  XLSX now round-trips an actual reduced-loss case through its nested dividend
  sheet and matches the equivalent JSON result exactly.
- The second material reachability gap is now closed under `td-ed02d8`. ABL
  § 13 A's annual calculation is split into a preliminary result for each person
  and one paired result that derives both transfer directions from the spouses'
  source years. Own offset capacity is calculated from validated § 12 gains,
  typed dividend-source classifications, § 19 B net gains and § 44 holdings;
  neither spouse supplies the other's positive net amount as an input. The
  canonical Personskat pair graph computes the reciprocal result once before
  either person's § 4 a amount is assembled. A 50.000 kr. loss against the
  spouse's 12.000 kr. qualifying dividend therefore transfers 12.000 kr. and
  carries 38.000 kr.; without year-end cohabitation all 50.000 kr. are carried.
  Pair transfer is additionally disabled when either person's typed source year
  is invalid, so a carried loss cannot leak through an otherwise rejected input.
  Current ABL §§ 12-15 and §§ 17-22, current § 44, the historical § 4, stk. 2,
  and the official 136.600/273.100 kr. adjusted-threshold guidance are attached
  as typed sources. The generated Personskat workbook exposes the new § 13 A and
  § 44 source facts with Danish labels and related child sheets, and the same
  married case produces identical XLSX and JSON results. Focused § 44,
  ordinary-share, KGL § 32, pension and debt-restructuring scenarios all pass.
- `td-85ed17` lukker den resterende § 44-grænse. Direkte aktier og fondsaktier
  ligger i en typet beholdning; en fondsaktie skal pege på præcis én direkte
  grundaktie i samme selskab og år, og selvreference, manglende eller duplikeret
  identifikation, krydsselskabsreference og fondsaktie-på-fondsaktie afvises.
  Den lukkede linje følger stk. 2's ordlyd: grundaktien skal være omfattet af
  stk. 1; en tidligere fondsaktie er alene omfattet gennem stk. 2. Den oprindelige
  L 78-kilde er knyttet til regelblokkens typede proveniens.
  Stk. 3 er et dateret statusforløb, hvor kursværdien på ændringstidspunktet
  bliver en afledt anskaffelsessum i den ordinære ABL-beholdning. Efterfølgende
  hel- og delsalg bruger derfor den eksisterende gennemsnitsmetode og §§ 5 A,
  12-14 uden et særskilt beregningsspor. Årsledgeren afviser genafspilning af en
  statusændring fra et tidligere år; den værdi hører i stedet til primo-
  positionen. Et kanonisk Personskat-scenarie fører 150.000 kr. i statusgrundlag
  og et salg på 210.000 kr. til 60.000 kr. aktieindkomst og 16.200 kr. skat efter
  PSL § 8 a. De nye beholdnings-, linje-, dato- og kursværdifelter har typede,
  danske beregningsmetadata til JSON/TOML/XLSX-grænsen.
- Den typede ABL § 17-model er nu ført gennem den kanoniske Personskat-grænse
  under `td-699438`. Den direkte gren modtager år, et kildegrundlag efter ABL
  §§ 6-7, næringsstatus, instrument, erhvervelsesstatus og beløb. Reglerne
  afleder selv § 6/§ 7-resultatet og hele § 17-resultatet, herunder stk. 1,
  tabsafskæringen i stk. 2, minimumsbeviser i stk. 3, undtagelserne i stk. 4,
  § 23-broen og relationen til KGL § 32. Personskat accepterer kun den afledte
  § 7-persongren og afviser et ellers gyldigt § 6-selskabsresultat ved
  borgergrænsen.
  Den øvrige ABL-gren kræver en separat typet § 17-modprøvekilde for de
  effektive §§ 19 B, 19 C, 21 og 22. År, beløb, instrument og effektiv
  aktivklasse skal stemme, og en post, som faktisk omfattes af § 17, skal bruge
  den direkte gren. `AblNæringsaktiePar17` og de tidligere løse
  `par17_modprøve`-felter kan derfor ikke vælges i den kanoniske arbejdsbog.
  Resultatet bevarer både det fulde `AktieavancePar17Resultat` og den afledte
  `KursgevinstPar32Kontraktrelation`, så klassifikationen kan auditeres og
  anvendes af en særskilt kontraktkilde uden at gøre aktieposten til en
  KGL-kontrakt. `td-86bd9a` følger op med den manglende kanoniske
  KGL § 32-kontraktgrænse og dens års- og ægtefælleforløb.
  Alle 25 kanoniske scenarier og alle 15 fokuserede § 17-scenarier passerer
  kompileret. Den fulde XLSX-rundtur udfylder kildefakta for en § 17-gevinst på
  7.000 kr., gendanner samme kanoniske JSON og returnerer den afledte § 7-status,
  § 17-anvendelse og KGL § 32-relation. Skemaet for denne slice havde hash
  `e7a63e73e5c8943ea7d69588a3eea69423551347b8ee6a2715fe3c10aea6b09f`.
- `td-c743fb` tracks the complete generated Personskatteloven workbook. Its
  completion boundary is one typed `@ calculate` input graph that reaches every
  required and optional fact in the supported full-law calculation. The XLSX
  remains derived from that contract: scalar and optional values stay on the
  case sheet, while only genuine `List`, `Map`, or `Set` values become related
  child sheets.
  The workbook engine itself is no longer the limiting factor: this slice adds
  fact-only asset and owner-position lists that the eventual aggregate can
  expose. The remaining work is to make the canonical input graph reach every
  supported branch before generating and presenting one full citizen workbook.
  The focused investment contract also proves that recursive legal input graphs
  terminate in schema/XLSX generation and round-trip through `runa call`.
  The § 19 workbook currently derives four related fact sheets, and the
  §§ 19 A-20 A workbook derives seven, including nested participant-company
  claims. The eventual full workbook remains downstream of the complete
  calculation contract rather than replacing legal recursion with precomputed
  caller values.
- Aktieavancebeskatningsloven §§ 37-40 are now composed into the canonical
  Personskat calculation under `td-3681f7`. A typed source record establishes
  the departure holdings and their § 38 basis, derives the marginal § 8 a tax,
  applies § 39's reporting and security conditions, creates the § 39 A
  holdings/deferral ledger, replays ordered annual disposals, distributions,
  loans, payments, reporting, documentation, death and § 40 reductions, and
  applies § 39 B on re-entry. Callers provide legal facts and event history,
  not a tax amount or mutable opening ledger. Invalid identifiers, years or
  ordering fail closed without advancing the state; a negative acquisition
  basis remains a valid statutory fact.
  The lifecycle also carries a typed source-history cutoff and documented
  deadline extensions. It checks active deferral year by year and derives a
  missing annual report only after the effective § 39 A deadline has passed;
  the deadline day and an incomplete current year remain pending. The derived
  default is retained as an explicit event result and therefore remains
  traceable without asking the caller to submit a legal conclusion.
  The departure-year gain enters share income exactly once, while new
  deferral, amounts falling due, payments and balance lapse remain separate
  collection-state outputs. In the canonical 2026 example, a 100.000 kr.
  departure gain produces a 79.400 kr. low-rate basis and a 20.600 kr.
  high-rate basis: 21.438 kr. is the final low-rate share-tax field, 8.652 kr.
  enters the ordinary final assessment, and the total exit tax and new
  deferral are both 30.090 kr.
  The generated workbook exposes human-labelled parent, holding, event and
  re-entry tables. One populated XLSX case and the equivalent direct JSON case
  produce identical complete results, including the 30.090 kr. deferral, and
  the workbook metadata preserves the official sources and guidance for
  §§ 37-40. Fourteen focused lifecycle scenarios cover security denial, spouse
  threshold transfer, negative basis, ordered multi-year replay, explicit and
  missing late reporting, ordinary and extended deadlines, re-entry and death.
  The persistent multi-period state remains a separate typed module
  instead of being folded into § 39's one-time eligibility decision.
  Under `td-aea204` no longer repeats current-year own or spouse share income
  inside that lifecycle input. The canonical Personskat graph first derives
  signed ordinary share income, dividends and other valid share-income sources,
  then supplies that context to each current departure branch in source-list
  order. Negative own or spouse income is treated through the same § 8 a pair
  rules as positive income. A separate historical variant remains available
  only when a departure from an earlier income year is replayed; an explicit
  same-year aggregate is rejected. The generated JSON/XLSX contract therefore
  exposes the context origin but no duplicate current-year context amounts.
  A typed annual context keeps the signed own/spouse amounts, cohabitation and
  derivation validity together through the rule chain. When both cohabiting
  spouses have current-year departure branches, the canonical calculation now
  fails closed instead of attributing the shared § 8 a effect twice. The joint
  netting and attribution model is bounded follow-up `td-421749`.
  Twelve focused scenarios cover signed pair tax, own and spouse derivation,
  cohabitation, ordered multiple branches, historical replay, conflicting
  same-year context and simultaneous spouse departure in both the interpreter
  and generated Rust. The expanded direct-JSON/generated-XLSX boundary test
  also produces identical spouse-context results (including 50.000 kr. of
  spouse share income and 27.000 kr. of exit tax) and rejects the conflicting
  same-year historical context through both formats.
  Under `td-7469fc` er den tidligere, caller-valgte `tabsstatus` fjernet.
  Hvert § 38-aktiv bærer nu enten almindelige §§ 12-15-kildefakta om marked,
  tidligere optagelse, § 14-oplysninger og § 5 A eller et typet
  `AktieavanceAktivklassifikationInput` for §§ 17-22. Reglerne afleder selv
  både ABL-aktiv og Personskattelov-kategori. En blandet portefølje med 50.000
  kr. almindelig aktiegevinst, 30.000 kr. næringsgevinst og 20.000 kr. § 19
  C-gevinst føres derfor til henholdsvis aktieindkomst, personlig indkomst og
  kapitalindkomst. Et markedsnoteret tab på 10.000 kr. uden opfyldt § 14-
  oplysningsbetingelse afskæres fra nettogevinsten ud fra kildefakta; en endnu
  ikke understøttet § 20-kategori afviser hele inputtet i stedet for at blive
  almindelig aktieindkomst.
  De syv nye klassifikationsscenarier dækker også den lovlige nulbehandling af
  en historisk § 18-andelsgevinst og en § 22-gevinst under bagatelgrænsen; de
  skelnes udtrykkeligt fra en ikke understøttet ABL-kategori. Scenarierne
  passerer i både fortolkeren og genereret Rust. Den komplette JSON/XLSX-rundtur passerer fortsat og udfylder nu
  `aktivgrundlag` med de typede kildefakta i stedet for en konklusion om
  tabsfradrag. Genbrugelig typeankret metadata giver de nye felter danske
  etiketter og interviewspørgsmål i hele Personskat-grafen. Den lokale
  livscyklusberegning anvender fortsat kun § 8 a, når alle udgående resultater
  faktisk er aktieindkomst. En blandet portefølje returnerer derfor aldrig en
  opdigtet § 8 a-skat uden for den kanoniske Personskat-graf. Per-aktiv
  markedsproveniens til § 13 A/KGL § 32 er implementeret under `td-4e42fd`
  og beskrevet nedenfor.
- Under `td-8e8561` kan den kanoniske Personskat-graf nu beregne den blandede
  § 38-portefølje, som den lokale § 8 a-gren med vilje afviser. For hver aktuel
  fraflytningskilde afledes tre fulde slutskatteprojektioner: uden kildens
  fraflytterindkomst, med de lagerbeskattede eller straksbetalte poster og med
  samtlige poster. Fraflytterskatten er den positive forskel mellem den fulde
  og den første projektion. Henstanden er den positive forskel mellem den fulde
  og den umiddelbare projektion, begrænset til den samlede fraflytterskat. Hele
  beregningen er fortsat betinget af § 38's positive samlede nettogevinst; en
  samlet gevinst på 50.000 kr. og et fradragsberettiget tab på 60.000 kr. giver
  derfor hverken fraflytterskat eller henstand, selv om de to poster tilhører
  forskellige Personskattelov-kategorier.
  En portefølje med 50.000 kr. almindelig aktiegevinst, 30.000 kr.
  næringsgevinst og 20.000 kr. § 19 C-lagergevinst giver 31.200 kr. i
  fraflytterskat oven på 600.000 kr. i løn. Kun de to realisationsposter indgår
  i § 39-henstanden; lagerpostens marginalskat forfalder straks. Det fulde
  flerresultat fødes til årsberegningen, mens det ældre entalsresultat er tomt,
  så en blandet portefølje ikke kan fremstå som almindelig aktieindkomst.
  Flere fraflytningskilder behandles i den typede kildelistes rækkefølge, og
  deres deltas teleskoperer til forskellen mellem årets slutskat med alle og
  ingen af kilderne. En direkte livscyklustest, fem kanoniske scenarier og en
  præcis JSON/XLSX-rundtur passerer; sidstnævnte gendanner også de indlejrede
  § 19 B/§ 19 C-aktivmassefakta og giver bit-for-bit samme fulde resultat.
- Under `td-4e42fd` bærer hvert udsendt ABL-resultat nu sin egen
  kildeidentifikation og et typet kildegrundlag. Det almindelige grundlag
  bevarer markedsstatus, tidligere markedsoptagelse og § 14-oplysningsstatus;
  det særlige grundlag bevarer aktivets egen markedsstatus. § 38 udsender et
  sådant par for hvert aktieparti, og §§ 37-40-forløbet bevarer de fulde,
  umiddelbare og henstandsvalgte lister. De ældre lister uden kildegrundlag
  afledes mekanisk fra disse par og kontrolleres strukturelt mod dem.
  Personskats § 13 A-grundlag og KGL § 32 flader nu resultaterne ud pr.
  aktieparti i stedet for at genbruge porteføljens ydre markedsfelt. Et
  fokusscenarie med 40.000 kr. noteret og 30.000 kr. unoteret § 19 B-gevinst
  giver derfor præcis 40.000 kr. modregningskapacitet i begge regelsæt.
  Manglende proveniens giver ingen kapacitet og et synligt ugyldigt
  KGL-grundlag. En KGL-kontraktreference til et ABL-aktiv er kun gyldig, når
  alle udsendte resultater har samme dokumenterede markedsstatus; en blandet
  portefølje afvises som flertydig, mens enkeltkilder bevarer den hidtidige
  kontrakt.
  Den genererede arbejdsbog spørger nu om markedsstatus på hvert særligt
  fraflytteraktiv med et dansk feltlabel. Det eksisterende ydre felt er bevaret
  for enkeltstående kilder og forklaret som sådan i hjælpeteksten. Seks
  fokuserede invarianter passerer i både fortolkeren og genereret Rust. Den
  fulde kanoniske Personskat-kørsel passerer, og den komplette
  JSON/XLSX-rundtur gendanner en blandet noteret/unoteret portefølje med samme
  kilde-id'er, markedsgrene og bit-for-bit samme slutresultat.
- Under `td-5beddd` er
  `øvrigt_netto_fordringer_og_obligationsbaserede_investeringsbeviser_kroner`
  fjernet fra den offentlige Personskat-kontrakt. Erstatningen er et typet
  `KursgevinstlovÅrsnettoInput` med særskilte lister for fordringer og ABL
  § 22-beviser. Hvert forløb har en stabil identifikation, lovrelevante
  kildefakta, en eksplicit primo-position og ordnede anskaffelses-, afståelses-
  eller ultimohændelser. En videreført position accepteres kun fra det
  umiddelbart foregående indkomstår, og en aktiv lagerposition kræver en
  ultimoværdi.
  Futuruna afleder § 1-afgrænsning, §§ 13/17-klassifikation,
  tabsbegrænsningerne i §§ 14, 15 og 18, opgørelsen efter §§ 25-26 samt ABL
  § 22-status. Det rå, regelafledte bagatelgrundlag samordnes derefter med den
  eksisterende § 23-gæld og sælgerpantebrevene, før de endelige KGL- og
  ABL-resultater beregnes. Otte fokuserede scenarier dækker gevinst, tab,
  præcis 2.000 kr., et blandet grundlag på 2.300 kr., gyldig videreførsel og
  afvisning af en for gammel primo-position samt overlap mellem §§ 14/15/17
  og mellem §§ 17/18 i både fortolkeren og genereret Rust.
  Den uafhængige gennemgang fandt, at det første udkast samlede
  tabsgrundlaget og tabsbegrænsningen i én eksklusiv status. Det kunne ikke
  samtidig bevare, at et tab hørte under § 17, og at § 18 afskar fradraget.
  `KursgevinstlovFordringOpgørelse` har derfor nu et selvstændigt
  `tabsgrundlag` (§ 14 eller § 17) og en selvstændig `tabsbegrænsning`
  (§ 14, stk. 2, § 15 eller § 18). Et § 17-tab på 3.000 kr. forbliver uden for
  bagatelgrænsen og fradrages trods samtidige § 14/§ 15-fakta; med samme fakta
  og en § 18-DBO-begrænsning bevares § 17-grundlaget, mens hele tabet afskæres
  og ingen personlig omklassifikation udsendes.
  Gennemgangen præciserede samtidig Futurunas sproglige kontrakt for
  overlappende regler. Prioritetslagene er undtagelser, betingede standarder,
  almindelige klausuler og til sidst en ubetinget standard. Inden for samme
  lag vinder den første anvendelige regel i kildeorden. Reference og
  sprogskitse siger nu det samme, og permanente interpreter-/Rust-tests dækker
  både topniveau og RuleScope.
  Beregningsmetadataens nye KGL-stier bruger typede `pathof`-referencer gennem
  importerede lister og sumtyper. Den genererede arbejdsbog udstiller derfor
  menneskelige danske labels på fordrings-, positions- og hændelsesark uden at
  udstille `rå_netto_kroner` eller et færdigt KGL-resultat som input. En
  anskaffelse på 10.000 kr. og afståelse på 13.000 kr. giver samme fulde
  Personskat-resultat gennem direkte JSON og en genereret, udfyldt XLSX:
  3.000 kr. i afledt årsnetto og kapitalindkomst. Den fokuserede rundtur
  passerer på 1.035,80 sekunder; målingen er ført på performanceopgaven
  `td-6659f1`.
  Den nuværende position er bevidst smallere end en fuld KGL-ledger. Særskilt
  kredit- og valutakomponent efter §§ 14/25 er planlagt i `td-1e2380`, og
  delafdrag eller flere realisationer på samme position er planlagt i
  `td-aac8d3`. De afgrænsninger må ikke løses ved at genindføre et
  caller-beregnet årsnetto.
- Aktieavancebeskatningsloven §§ 35 G-35 K er nu ført gennem den kanoniske
  Personskat-beregning under `td-0b0a4b`. Et `AktieavancePar35Forløbsinput`
  etablerer ordningen fra det oprindelige valg og genafspiller identificerede,
  ordnede afståelser, udbytter, betalinger, omstruktureringer,
  tvangsafståelser, værdinedgangsdispositioner, årsoplysninger og
  hjemstedsflytninger. Personskat modtager kun det afledte ABL-resultat og
  bevarer hele hændelsessporet. Straksgevinsten efter § 35 H føres til
  aktieindkomst, mens virksomhedens forfaldne og betalte overdragerskat samt
  saldobortfald rapporteres særskilt og ikke lægges til personens ordinære
  skattetotal.
  Den kanoniske scenariofil har nu 43 invarianter. Et konkret 2026-forløb med
  -50.000 kr. i skattemæssig anskaffelsessum afleder 50.000 kr. i
  straksbeskattet aktieindkomst, 13.500 kr. i personens aktieindkomstskat og
  22.000 kr. i forfalden overdragerskat. Den genererede arbejdsbog udstiller
  valget på aktivrækken og bruger særskilte relationsark til overdragne
  aktiepartier og ordnede hændelser. Samme kildedata giver bit-for-bit samme
  fulde resultat gennem XLSX og JSON, inklusive hændelses-id, rækkefølge og
  ultimotilstand. Det aktuelle skema har hash
  `6a5ad4a81474e78e0197ccca10bb45358400ab385ba01aaf1dc6d2480672a0e0`.
- Close the Personskatteloven implementation gaps before deeper audits.
  § 3, stk. 2, nr. 2 is converted from a raw amount bridge to nine typed
  dependency outcomes, and nr. 3-11 now enter the canonical calculation
  through a closed union over their typed results. The focused aggregate
  scenario covers every numbered branch, both income-addition branches and
  rejection of all nine legacy raw amount categories in interpreter and
  compiled execution. The next bounded review should rank posture-only clauses
  and genuinely missing dependency rules by material calculation impact.
- Continue deepening dependency laws such as Kildeskatteloven, AM-law,
  municipal/church tax, Ligningsloven, and Opkrævningsloven only where they
  unblock Personskatteloven calculation completeness or validate a newly
  implemented legal slice.
- Brug den nu vedvarende § 12 B-hændelsesposition som mønster for andre
  flerperiodiske lovforløb, når den juridiske regel faktisk kræver en historik
  frem for et manuelt opgjort slutbeløb.
- Keep validation audits close to the implementation. Exploratory daisy-chain,
  confiscatory, household-benefit, minimum-retained-income up to 2 mio. kr., or
  loophole searches belong after the main law model is more complete.
- Keep reviewing domain boundaries as each slice grows. Encapsulate repeated
  legal facts when they are genuine statutory objects, but avoid broad refactors
  that would make source traceability weaker.
- Preserve original Danish legal text in multiline comment/source blocks above
  every Futuruna translation.
- Model ordinary legal statements primarily as `|` rules, using `under` for
  conditions and `exception` for overrides.
- Allow verification and audit files to break while the model is being
  reformulated; then repair them as milestone work rather than weakening the
  legal encoding.

## Next

- Færdiggør de afgrænsede SØBL-rester uden at svække den nye
  kildefaktamodel: `td-00b484` erstatter 92/184-dages tilnærmelser med eksakte
  kalendermåneder, `td-b874c0` samler § 6-driftstid pr. skib og indkomstår,
  `td-a97df7` fordeler LL § 7 U-bundfradraget, og `td-44eb29` giver dødsboer,
  begrænset skattepligtige og kulbrinteskattesager deres egne kanoniske
  beløbsresultater.
- Byg næste performance-lag efter de målte kontrakt- og artefaktcacher.
  `td-60a9d6` skal genbruge parsede og typede moduler på tværs af ændrede
  kildegrafversioner, så en enkelt lovfil ikke udløser cirka fire minutters
  frontendarbejde. `td-38f074` skal memoisere rene, uforanderlige globale
  værdier under én fortolket rodevaluering; et realistisk LL § 9-scenarie tog
  cirka 316 s fortolket mod 2,89 s som varm native artefakt. `td-783a9c` ejer
  derefter genbrug af en kompileret eller resident beregningsrunner på tværs af
  gentagne `call`-kald. Ingen af optimeringerne må kræve cacheannotationer i
  lovkoden eller ændre effekter og reaktiv semantik.
- Bevar den nu afledte KGL-årsnetto-grænse fra `td-5beddd`. Den næste
  KGL-udvidelse skal komme fra kildefakta: `td-1e2380` adskiller kredit- og
  valutakomponenter efter §§ 14/25, og `td-aac8d3` udvider positionen med
  delafdrag og gentagne realisationer. Ingen af delene må genindføre et råt,
  caller-beregnet årsnetto. Den eksisterende `td-6659f1` ejer fortsat den
  målte latenstid i den komplette JSON/XLSX-rundtur.
- Refine the completed ABL §§ 37-40 boundary without reopening its fact-first
  lifecycle design. Own/spouse share-income context is derived under
  `td-aea204`, and § 38's annual categories and ordinary loss eligibility are
  source-derived under `td-7469fc`. The mixed-category annual final-tax delta
  and realization-only henstand are derived under `td-8e8561`, and
  per-holding market provenance through § 13 A and KGL § 32 is derived under
  `td-4e42fd`. Under `td-3f2ee9`, a source-history cutoff and documented
  extensions now make a missing annual § 39 A report detectable without an
  explicit late-reporting event. These are bounded fidelity improvements, not
  a reason to replace the derived ledger with caller-supplied conclusions.
- Deepen the first-pass full-statute corpus from structural coverage into
  calculation coverage where official fixtures and dependent statutes make that
  safe. As those inputs become complete, extend the canonical calculation
  boundary tracked by `td-c743fb`; do not maintain a separate hand-authored tax
  workbook that can drift from the rules.
- Preserve the closed § 3, stk. 2, nr. 2-11 boundary while extending missing
  source-backed dependency outcomes. Do not reintroduce generic `{art, beløb}`
  adapters between typed dependency results and the canonical § 3 calculation.
- Continue Aktieavancebeskatningsloven from the now-executable §§ 35 G-40
  paths and the now-derived §§ 6-7 and §§ 19-22 boundaries: complete the
  remaining dependent classifications before calling the ABL dependency
  complete. Mixed
  nominal/no-par holdings, § 33 A status
  changes, employee-ownership transferor tax and the modeled exit-tax deferral
  lifecycle already have source-backed calculation paths. § 9 now has an
  annual two-ledger loss calculation, and § 5 A now reduces each validated
  disposal loss before that calculation. Rank the next dependent
  classification by its impact on Personskatteloven rather than deepening
  exploratory audits.
- Extend the canonical Personskatteloven `@ calculate` aggregate with the
  remaining executable § 3/§ 4/§ 4 a source-fact branches. The aggregate
  already reaches ordinary wage facts, ordinary and special ABL inputs, special
  tax/deficit conditions and optional annual settlement. Keep the generated
  workbook downstream of that graph so scalar cells, enum dropdowns, variant
  payloads and related collection sheets cannot drift from the executable law.
- Continue the current Ejendomsavancebeskatningsloven dependency from the
  source-backed §§ 5/5 A, 6, 8 and 9 paths. Milk quotas and § 5, stk. 6 are now
  executable and round-trip through the canonical Personskat contract, and
  § 6 D now carries seller-note installment taxation, owner-attributed annual
  EBL gain after partial succession and the separate KGL realization across
  years. § 10 now allocates reconstruction across identified properties,
  coordinates directly with Afskrivningslovens §§ 21 og 24, separates the
  stk. 9 owner-home gross amount from ordinary EBL gain and routes the selected
  property's deferred gain into later disposal. Remaining bounded work includes
  later owner-home classification changes, non-spouse beneficiaries and the
  remaining source-ranked dependency rules.
- Expand the first Personskat field-metadata slice as new source-fact branches
  reach the canonical calculation. Preserve canonical paths as machine keys and
  add human labels, interview questions, help, units and sources at the same
  time, so an AI can collect citizen facts without becoming the tax calculator.
- Preserve and deepen Personskatteloven § 3, stk. 2, nr. 10's now-contiguous
  Afskrivningsloven §§ 1-69 and Statsskatteloven § 6 dependencies. Add further
  historical fixtures only where official facts justify them; §§ 50-62 already
  feed typed income, depreciation, loss and other-deduction outcomes without
  raw bridge amounts. The § 40 transition to Ligningsloven § 12 B and the § 40
  D route through § 40 C remain end-to-end calculation dependencies.
- Replace remaining source-dependency placeholders with complementary official
  statutes and trusted calculation examples, especially for remaining
  municipal/church allocation and settlement edges beyond the current
  §§ 7/15/16/16 a formula slice, broader refresh automation for the current
  Nationalbank DNRUUPI-backed Opkrævningsloven § 7 annual-rate inputs, and
  remaining AM edge cases beyond the first source-explicit special-case slice.
  The AM-law
  slice now covers ordinary wage remuneration,
  taxable benefits, source-shaped § 2 nr. 1-6/stk. 3 wage-earner base posts,
  § 3 exclusions, self-employed bases with and without virksomhedsordning,
  library-fee compensation, the 2026 youth exemption, and collection-reference
  posture. The first municipal/church
  slice now covers ordinary municipal tax on Personskatteloven taxable income,
  church tax for Folkekirken members, Kommuneskatteloven §§ 2-3 skattekommune
  selection and move allocation, and Kommuneskatteloven §§ 7/15/16 provisional
  payment, § 7 stk. 4 statsguaranteed-basis calculation, own-budget
  after-regulation, and stk. 3 supplement formula plus stk. 4
  business-tax/conjuncture/income-equalisation/acconto-tax settlement. § 16 a
  now also covers the self-budgeting correction frame, the capped annual frame
  regulation from 2027, positive proportional municipal allocation and the
  resulting deduction from § 16, stk. 2. The first
  Kildeskatteloven
  slice now covers ordinary wage A-income, withholding duty, e-skattekort card
  types, main-card period allowances, bikort without allowance, optional higher
  withholding percentage, base rounding, and the statutory 55 pct. no-card
  fallback. The first BEK 839 slice now generates skattekort values from
  forskudsskat plus an unrounded withholding percentage. The first BEK 1094
  slice now derives the 2026 unrounded withholding percentage from municipal,
  church, bundskat, mellemskat, topskat and toptopskat components. The
  fictional household scenario now computes monthly A-skat and cash-flow payroll
  output both from supplied e-skattekort allowance/procent inputs and generated
  BEK 839 card values using the BEK 1094-derived percentage. The first
  Opkrævningsloven slice now covers ordinary and large-withholder A-skat/AM
  payment deadlines, late payment posture, provisional assessment posture, and
  the § 7 stk. 2 annual-rate formula from July/August/September Nationalbank
  kassekreditrente inputs plus source-backed 2025/2026 DNRUUPI cells that
  reproduce the Skattestyrelsen-published annual rates, and the 2026
  source-chain amendment to the § 7, stk. 1 late-payment
  supplement.
  The first Kildeskatteloven slutopgørelse slice now covers § 60 crediting,
  § 61 restskat plus percentage supplement, timing posture, B-skat/restskat
  rateplans with large/small installment splits, including system-date-driven
  late stk. 4 and stk. 6 three-rate plans, § 62
  overskydende skat plus compensation/refund posture, § 60 spouse offsetting,
  § 58 B-skat calendar projection, § 62 A amended annual statement interest
  posture with date-derived month counts and payout deadlines, § 62 C minimum
  thresholds, and § 67
  dividend-tax credit posture; the
  fictional household's generated-card annual settlement currently yields
  3.541 kr. overskydende skat and 3.541 kr. payout under the source-derived
  § 7 rate fixture.
  The first bomb-audit probes now formalize nine daisy-chain tensions: § 6
  spouse negative net-capital offset can lower the other spouse's bundskat
  basis; § 7 stk. 5 negative net capital can both offset the spouse's positive
  net-capital income and raise the spouse's effective positive-capital
  threshold, removing 6.375 kr. mellemskat in the probe; § 14 helårsomregning
  can increase the state-tax component for the 180-day
  wage-earner case by 3.293 kr.; a high municipal rate can lower the state-tax
  component through § 19 while still increasing total tax; the 2026 personlige
  skatteloft sits 10,83 percentage points below the full
  mellemskat/topskat/toptopskat progression stack in the Copenhagen probe; §
  8 a unused spouse share-income threshold can remove the high share-income
  tax bracket; § 8 a mandatory negative share-income spouse offset is not
  neutral in the family-net probe; § 8 b CFC tax sits outside both § 9
  personfradrag and § 19 skatteloft in the executable model; and § 13 can lock
  passive business losses to same-business carry-forward while active
  participation releases the same amount into current other-income deduction.
  A separate confiscatory effective-rate audit now searches 8.064 bounded
  year/municipality/income/payment configurations. It finds no encoded
  current-year `årsskat` above current positive wage/capital/share income in
  that grid, while it finds 360 configurations above 100 pct. when transferred
  restskat m.v. under Kildeskatteloven is included as a payment burden. The
  highest current-year `årsskat` rate in the grid is 52,63 pct. in a 2026
  Langeland high-wage church-tax case; the highest payment burden is 215,91
  pct. in a 2024 Copenhagen low-income share-income case with 150.000 kr.
  transferred restskat m.v.
  A first cross-law household benefit audit now covers Børne- og ungeydelse:
  the fictional three-child household has 48.216 kr. annual benefit before
  aftrapning and no reduction at the current wage levels, while a parent 100
  kr. over the 2026 mellemskat-linked threshold loses 2 kr. of that parent's
  own half and one fully phased-out parent leaves the other parent's 24.108 kr.
  half intact. The same audit also flags a source-wording tension: the current
  Retsinformation § 1 a and Borger.dk posture point to the mellemskat-linked
  reduction base for 2026, while the Skatteministeriet rate page still describes
  the reduction income as the topskat basis.
  The same audit now covers a first boligsikring § 22 cliff: at 80.000 kr.
  annual housing expense and 215.000 kr. household income, one child gives no
  child increment to the income threshold and leaves an 8.046 kr. income
  deduction, while the second child raises the threshold enough to remove that
  deduction in the encoded 2026 slice.
  § 13's first dependent-source slice now covers
  Pensionsbeskatningsloven § 16, Ligningsloven § 33 A,
  Sømandsbeskatningsloven §§ 5-8, the 2026 repeal in LOV nr. 482/2024, and
  LOV nr. 482/2024's reform insertion of § 8/toptopskat into the § 13
  modregningsrækkefølge.
  § 4 a's amount-level audit now covers included and excluded share-income
  posts, § 19 B-to-§ 17 personal-income reclassification, negative share-income
  preservation and pensionsfradrag in positive share income: the deduction is
  capped at positive share income, derives notice and requested amount through a
  typed election result, is cancelled by timely reversal by 30 June in the
  second calendar year after the income year, is blocked if already deducted in
  personal income, and is unavailable without positive share income.
- Add more trusted external differential fixtures after the first § 14/§ 19
  external slice. The ordinary 2026 Copenhagen wage-earner path now has a
  source-backed Skat.dk calculator fixture for final tax and generated tax-card
  values. The official § 14 guidance example now verifies annualisation
  rounding and one-off income handling, and the source-backed Langeland 2026
  high-municipal-rate fixture now compares § 19 personal relief against the
  published 1,24 pct. municipal `Nedslag pct.` while exercising both personal
  and positive-capital relief inside the wage-earner calculator.
- Separate legal structure from annual parameter packs: rates, thresholds,
  personal allowances, municipal tax, church tax, and other tax-year data.
- Build calculation fixtures for ordinary wage-earner cases before handling
  complex cases.
- Gather complementary official sources for:
  remaining municipal and church-tax settlement/allocation, personal allowance,
  broader automated refresh of Opkrævningsloven § 7 Nationalbank input lookup,
  date-exact B-tax
  remaining-rate selection, remaining
  AM edge cases, other
  itemized deductions beyond the ordinary §§ 9 J/9 K wage-earner deductions and
  the first § 9 L extra-pension slice,
  and annual rate/threshold adjustments.
- Keep existing audits running as implementation validation, but defer expanded
  source-drift, delegated-power, confiscatory, household-benefit, and
  daisy-chain searches until the main Personskatteloven calculation model has
  fewer first-slice gaps. The existing audit files remain useful guardrails; they
  should not lead the next milestone.
- Extend the website page as more of the corpus becomes calculation-ready.

## Later

- Encode remaining spouse rules, partial-year edges, pension special-plan and
  historical-year edges, share income, CFC income, business income,
  property-related income, and special regimes.
- Mature the existing normal-person income tax calculator as the remaining
  source-law branches and tax-year parameter packs reach its generated
  contract.
- Add differential checks against official examples or trusted calculators where
  legally safe and sourceable.
- Extend the Retsinformation update automation with optional reviewed patch
  generation after the fetch/detect/report workflow has been used a few times.
- Expand audits into legal "bomb" discovery: confiscatory effective rates,
  cliff effects, hidden delegations, obsolete provisions, incoherent categories,
  and temporal contradictions between consolidated law and annual parameters.
- Integrate the mature corpus into the website alongside the Danish
  Constitution research pages.

## Milestones

M0 - Source foundation

- Status: first slice implemented.
- Output: this status log plus a checked source-status `.runa` file.
- Done when: the project records current and historic source posture and has a
  passing Futuruna file that prevents historic law from being used for live tax
  calculation.

M1 - Income taxonomy

- Status: first slice implemented.
- Output: chapter/foundation `.runa` file for §§ 1-4 b.
- Done when: ordinary income, personal income, capital income, share income, and
  CFC income are represented as typed legal categories and amount-level result
  records with original text preserved.
- Current slice: § 1/§ 2 ordinary taxable-income composition is executable as a
  named result over personal income, capital income, share income, CFC income,
  and ligningsmæssige fradrag. The fixture proves § 4 a share income remains
  outside ordinary taxable income and § 4 b CFC income remains outside the
  §§ 6-8 a taxable-income base while reclassified § 4/§ 4 a amounts feed
  personal income. § 3, stk. 2, nr. 1 now keeps the broad self-employed
  business-expense deduction separate from the statutory carve-outs for § 4,
  stk. 1, nr. 1, 2, 7 and 8 and Ligningsloven §§ 9 G and 13, so those carved-out
  amounts are not deducted as personal-income business expenses. § 3, stk. 2,
  nr. 6 now delegates AM and foreign social-contribution eligibility to a typed
  Ligningsloven § 8 M result before adding the amount to personal-income
  deductions. § 3, stk. 2, nr. 7 likewise delegates new-reserve eligibility
  and amount calculation to typed Virksomhedsskatteloven § 22 b or § 22 d
  results. Nr. 3-11 are then combined by a closed result union instead of
  constructing generic personal-income deduction posts; nr. 4 and nr. 10
  taxable additions travel through the same aggregate.

M2 - State tax computation skeleton

- Status: first slice implemented.
- Output: `.runa` files for §§ 5-9.
- Done when: the legal structure of bundskat, mellemskat, topskat, toptopskat,
  abolished/zeroed taxes, aktieindkomstskat, CFC tax, and
  municipal-equivalent state tax is encoded.
- Current slice: 2026 LOV nr. 482/2024 state-tax structure is represented:
  § 5 now sums typed state-tax component posts into an amount-level
  `Par5StatsskatResultat`, filtering inactive components by tax year so
  udligningsskat/sundhedsbidrag are ignored from 2026 and mellemskat/
  toptopskat are ignored before the 2026 reform. Mellemskat under § 7,
  topskat under § 7 a, and toptopskat under § 8 now derive their 2026
  thresholds from typed `ReformStatsskatParameterResultat` values carrying the
  LOV nr. 482/2024 § 1 nr. 2-4/nr. 5/nr. 6 source branch, then regulate the
  amendment's 2010-level amounts through § 20 before feeding the calculator,
  with § 7 a and § 8 now exposed as named personal-income amount results; CFC
  tax under § 8 b can feed this state-tax
  component model, with § 8 b consuming the amount-level § 4 b CFC-income
  result before applying a structured Selskabsskatteloven § 17, stk. 1 rate
  result.
  The § 6 slice now computes the amount-level spouse negative net-capital
  offset before bundskat basis calculation, and its rate accessor now projects
  a source-backed `BundskatSatsResultat` covering the 2021 LBK rate plus the
  2022/2023/2024 statutory reductions to 12,09/12,06/12,01 pct. and the
  corresponding 4,09/4,06/4,01 pct. no-municipal-liability rates.
  The § 7 mellemskat slice now covers positive net capital income over the
  § 20-regulated 2026 threshold, including an executable spouse doubled-threshold
  case and the § 7 stk. 5 rule that negative net capital is offset against the
  spouse's positive net-capital income before the spouse's effective
  grundbeløb is increased. It now also exposes the § 7 stk. 10-11 allocation of
  the combined spouse capital tax and the stk. 12 tie-break for equal stk. 7
  beregningsgrundlag.
  The historical § 7 a udligningsskat slice now computes the amount-level
  tax from the regulated 2010-level grundbeløb, the stk. 3 corrected-personal-
  income cap, the stk. 6 spouse grundbeløb increase with the 121.000 kr.
  regulated cap, and the 2011-2018 phase-out rates.
  The historical § 8 sundhedsbidrag slice now computes amount-level tax from
  skattepligtig indkomst, the 2010-2019 rate phase-out, and the stk. 2
  municipal/§ 8 c liability condition. Both historical rate ladders now expose
  source-backed rate results with percentage, basispoints, and phased-out
  posture while preserving the scalar basispoint accessors used by the
  calculators.
  The ordinary wage-earner domain model now carries § 7 a udligningsskat and
  § 8 sundhedsbidrag as explicit state-tax component slots before and after
  personfradrag allocation, and feeds those slots into the § 5 aggregate.
  It also exposes ordinary zero-default § 8 a aktieindkomstskat, § 8 b
  CFC-indkomstskat, and § 8 c kommunal-lignende statsskat slots through the
  same § 5 aggregate boundary, so later nonzero special-income integration has
  a stable domain home.

M3 - Tax-year parameter packs

- Status: first slice implemented.
- Output: sourceable annual tables for rates, thresholds, allowances, and
  municipality-specific inputs.
- Done when: the same legal rules can run for at least two tax years by swapping
  parameter packs.
- Current slice: 2024, 2025, and 2026 national parameters now return
  `NationalParametreResultat` values with Skattestyrelsen source branches.
  Copenhagen and Gentofte municipal/church-tax inputs for 2024-2026, plus the
  2026-only Langeland municipal row, now return `KommunaleParametreResultat`
  values with Skatteministeriet source branches. Combined
  `SkatteårParameterpakkeResultat` values carry both national and municipal
  source provenance before projecting the existing plain parameter pack. The
  2026 pack covers mellemskat, topskat, toptopskat, personfradrag,
  aktieindkomst, skatteloft, municipal rates, church-tax rates, published
  skatteloftsnedslag, and grundskyldspromille. Parameter completeness is
  year+municipality specific, so Langeland is not treated as supported for
  2024/2025 until those rows are source-backed.

M4 - Ordinary taxpayer calculator

- Status: first slice implemented.
- Output: fixtures and executable examples for wage income, capital income,
  deductions, municipal tax, church tax, and AM contribution.
- Done when: normal cases produce reproducible tax breakdowns with source-backed
  assumptions.
- Current slice: 2025 Copenhagen and Gentofte wage-earner fixtures produce
  deterministic AM contribution, personal income after AM, ordinary taxable
  income through the § 1/§ 2 `Par1AlmindeligSkattepligtigIndkomstResultat`
  after derived Ligningsloven §§ 9 J/9 K wage-earner deductions,
  bundskat, topskat, municipal tax, church tax, § 10 personfradrag, § 12
  personfradrag tax values and § 9/§ 12 state-tax allocation,
  after-personfradrag totals, and a § 13 ordinary-positive-income boundary.
  Separate § 13 calculator breakdown fixtures now cover spouse-transfer deficit,
  LL § 33 A relief, 2026 post-PBL-repeal
  transfer, and same-business loss carry-forward cases. 2026 Copenhagen
  wage-earner fixtures now exercise mellemskat, topskat, and toptopskat under
  the LOV nr. 482/2024 reform thresholds, with topskat and toptopskat routed
  through named personal reform results, and a 2026 Copenhagen positive
  net-capital fixture exercises the mellemskat capital addition. The wage-earner
  model now routes state income tax before personfradrag through the
  `Par5StatsskatResultat` aggregate, so the ordinary calculator consumes the
  same § 5 active-component filtering as the source-law module instead of a
  parallel scalar sum. The public wage-earner model now delegates through
  `LønmodtagerBeregningSag` with standard tax conditions, and the audit model
  verifies that a Kildeskattelov §§ 48 E/48 F researcher-income posture carries
  § 10's personfradrag exclusion all the way through final tax. Its breakdown
  now includes `LønmodtagerSkatteloftResult`,
  so § 19 personal and
  positive-capital skatteloft input, excess basis points, and kroner relief are
  part of ordinary 2025/2026 calculator output. The 2026 Copenhagen
  positive-net-capital fixture now applies the 42 pct. positive-capital ceiling,
  reducing final tax by 106 kr.; current Copenhagen/Gentofte personal-income
  fixtures are explicitly under the personal ceiling. 2026 Langeland
  wage-earner fixtures now exercise source-backed high-municipal-rate § 19
  relief: 124 basis points and 2.316 kr. personal relief on a 900.000 kr. wage
  case, plus 381 basis points and 449 kr. positive-capital relief on a
  650.000 kr. wage plus 110.000 kr. capital-income case. The ordinary wage-earner
  AM contribution now imports Arbejdsmarkedsbidragsloven instead of
  using a local arithmetic shortcut, and the AM-law module now has
  a source-backed typed § 3 exclusion result for each of nr. 1-5, carried into
  the ordinary wage-earner result, plus special-case fixtures for self-employed
  bases, library-fee compensation, and the 2026 youth exemption. Ordinary
  municipal income tax and church tax now import Kommuneskatteloven and Folkekirkens
  økonomi instead of using
  local arithmetic shortcuts. Ordinary Ligningsloven employment/job deductions
  now import the Ligningsloven dependency slice instead of being manual zeroes,
  and the dependency slice now also models § 9 L extra pension deductions for
  direct use by Personskatteloven § 26 nr. 5.
  A first fictional household scenario now computes a 2026 Copenhagen married
  renter household with 50.000 kr./month primary wage income and 20.000
  kr./month spouse wage income. A first household benefit-cliff audit covers
  Børne- og ungeydelse for the three-child scenario and a first boligsikring
  § 22 child-threshold cliff while explicitly marking broader housing-support
  and other deduction discovery as outside the current executable slice.
  Kildeskatteloven now marks the primary wage as A-income,
  proves that A-skat must be withheld, computes the statutory 55 pct.
  withholding if no e-skattekort, bikort or frikort has been received, and
  computes monthly A-skat/cash-flow payroll output when e-skattekort
  allowance/procent inputs are supplied. BEK 839 now generates the household's
  main-card monthly allowances from forskudsskat and BEK 1094-derived
  withholding percentage inputs, producing a separate generated-card payroll
  view.
  Opkrævningsloven now provides source-backed payment-deadline/remittance rules
  plus the § 7 annual-rate formula, Nationalbank DNRUUPI raw-rate inputs for
  2025/2026 and date-exact daily late-payment interest context, with fixtures
  separated into `indeholdelse-afregning.scenario.runa` where they are
  scenario facts. The § 13
  complex calculator input now uses domain objects for income basis, tax-value
  rates, offset taxes, spouse transfer, stk. 5 limits, and same-business loss
  facts. `slutopgoerelse.scenario.runa` now also computes the fictional
  household's generated-card overskydende skat compensation and payout, plus a
  low-withholding restskat path with supplement and next-year transfer posture.
  Kildeskatteloven now also exposes the § 58 B-skat calendar as a rate-window
  domain object, § 62 A interest fixtures, and a restskat minimum-rate tension
  plus completion plan when remaining B-skat rates are too few, and separates
  late § 61 stk. 4 and stk. 6 system-start dates in executable restskat
  rateplans.
  `delaar-scenarier.scenario.runa`
  now runs a 2026 Copenhagen § 14 partial-year wage-earner case, annualizing
  180 days of wage income and applying the reduced §§ 6-9 state-income-tax
  result while keeping AM outside the § 14 helårsskat component.
  `kommuneskattelov-personskat.scenario.runa` now mirrors that posture for
  Kommuneskatteloven § 5, stk. 3, validating both helårsomregnet municipal
  income tax and the Personskatteloven § 14, stk. 2 election branch.
  `aktieindkomst-slutopgoerelse.runa` now composes Personskatteloven § 8 a
  with Kildeskatteloven § 67 as reusable calculation rules; the corresponding
  scenario file supplies fictional wage-earner fixtures:
  150.000 kr. share income with the spouse's unused share-income threshold stays
  in the 27 pct. final-tax layer, while the high-tax variant splits 21.438 kr.
  final low-layer tax from 29.652 kr. high-layer tax entering slutskat and
  leaves 7.900 kr. restskat after 19.062 kr. dividend-tax credit. The scenario
  now builds a `Par5StatsskatResultat` for slutskat-bound state-tax components,
  excluding the final low-layer § 8 a tax while including the high-layer
  `Aktieindkomstskat` amount before § 12 personfradrag allocation. The
  source-law module now keeps source-backed § 8 a stk. 1/stk. 2 rate results
  for the 28/27/42 pct. rates and also covers § 8 a, stk. 3 as a separate
  scoped rule case
  for over-withheld dividend tax and negative-share-income full credit, and
  § 8 a, stk. 6 for both-negative spouse share-income cases where the double
  threshold is split proportionally. It now also covers negative-share-income
  final settlement: a 120.000 kr. negative-share case is fully absorbed by own
  slutskat, while a 900.000 kr. negative-share case offsets 208.726 kr. in own
  slutskat, 50.000 kr. in spouse slutskat, and carries 95.454 kr. forward. A
  paired settlement case now derives the spouse's own annual settlement before
  applying the § 8 a, stk. 5 negative-tax credit: 150.000 kr. spouse positive
  share income is first offset against a 900.000 kr. negative-share case, the
  remaining negative tax offsets 208.726 kr. in own slutskat and 71.005 kr. in
  spouse slutskat, and 11.449 kr. is carried forward.
  The residual amount now continues through a typed annual ledger. Opening and
  closing tranches preserve owner and origin year, current-year negative tax is
  used before old carry, old tranches are consumed oldest first, and every
  opening amount is conserved as use plus closing carry. A prior Futuruna
  result must be balanced and exactly one year old; older externally assessed
  tranches require typed authority provenance. Single, married and role-swapped
  cases pass interpreted and compiled execution, while the calculation CLI
  round-trips an external 2024 tranche through JSON and XLSX and derives the
  corresponding Kildeskatteloven § 60 credit rather than accepting it as input.
  Personskatteloven § 8 c now computes the municipal-equivalent tax for covered
  limited-taxpayer postures, using the Skatteministeriet-published 25 pct.
  2026 rate and the same personfradrag reduction posture as § 10 stk. 5.
  Personskatteloven § 8 b now keeps the Selskabsskatteloven § 17, stk. 1
  historic/current source line, 22 pct. ordinary selskabsskat rate, 3
  percentage-point kulbrinte supplement, and applied CFC rate in one result
  object.
  `LønmodtagerBeregningSag` now exercises a nonzero 2026 CFC/§ 8 c
  tax-position path: 500.000 kr. CFC income feeds 110.000 kr. § 8 b tax into
  the § 5 aggregate, § 8 c replaces ordinary municipal income tax for the
  limited-taxpayer posture, § 10 stk. 5 personfradrag reduces the § 8 c amount,
  and § 19 uses the 25 pct. § 8 c rate in place of a municipality rate.
  Personskatteloven § 11 now computes negative net-capital-income reduction
  with spouse threshold pooling, spouse positive-net-capital offset before
  threshold increase, source-backed stk. 2 rate provenance, statutory tax-order
  reduction, and unused spouse transfer.

M5 - Audit suite

- Status: first slice implemented.
- Output: audit files that intentionally search for tension, missing inputs,
  discontinuities, and source drift.
- Done when: audits can fail loudly without blocking legal reformulation work.
- Current slice: source-status rejection, covered § 1/§ 2 taxable-income
  composition from separate income categories, covered normal-fixture
  personfradrag, covered § 10 stk. 5-6 choice/deadline/exclusion posture,
  covered § 10 stk. 3 spouse transfer of unused personfradrag state-tax value,
  covered § 11 negative net-capital reduction with source-backed stk. 2 rate
  provenance, spouse threshold pooling, spouse positive-capital offset,
  statutory reduction order, and unused spouse transfer, covered § 9/§ 12 split
  state personfradrag tax-value reduction
  order, § 9 non-state personfradrag reduction, and wage-earner component
  projection,
  covered source-backed 2024-2026 national/municipal parameter-pack provenance,
  covered 2026 state-tax reform parameter source branches, covered § 19
  personal and positive-capital skatteloft source branches across the LBK text
  and LOV nr. 482/2024 rewrite, covered § 20 regulation-number source branches
  and corrected 2020-2024 regulation figures against SKM's historical table,
  covered § 6 source-backed bundskat-rate
  provenance across the 2022/2023/2024 amendment chain,
  covered § 13 deficit mechanics,
  § 13 stk. 4 negative-personal-income spouse and carry-forward offset order,
  mellemskat positive-net-capital and spouse-threshold activation,
  § 7 stk. 5 spouse negative-capital offset/effective-grundbeløb activation,
  § 7 stk. 10-11 spouse capital-tax allocation, and § 7 stk. 12 tie-break,
  historical § 7 a udligningsskat amount calculation with post-level stk. 1
  included/excluded pension-like payments plus stk. 3 and stk. 6 spouse-threshold
  cases and source-backed phase-out rate provenance,
  historical § 8 sundhedsbidrag amount calculation with liability, zero-rate
  boundary cases, and source-backed phase-out rate provenance,
  wage-earner domain-model projection of explicit § 7 a/§ 8, positive § 8 a
  high-layer share-income tax, and § 8 b/§ 8 c state-tax slots through § 5
  aggregation and § 9 personfradrag allocation,
  ordinary wage and special-case AM-law coverage, ordinary Ligningsloven
  §§ 9 J/9 K wage-earner-deduction coverage plus § 9 L/§ 26 nr. 5 validation
  coverage and § 15 Q subletting/letting surplus, including stk. 4
  § 15 P coordination, feeding Personskatteloven § 4, stk. 1, nr. 17,
  Virksomhedsskatteloven §§ 7/22 a/22 c/23 a capital-return validation feeding
  Personskatteloven § 4, stk. 1, nr. 3 and nr. 3 a, including § 23 a
  personal-income election reduction,
  ordinary municipal/church-tax legal coverage,
  covered Kildeskatteloven ordinary A-income/withholding/e-skattekort posture,
  covered BEK 839 forskudskort generation, covered BEK 1094 2026
  indeholdelsesprocent derivation, covered Kildeskatteloven slutopgørelse
  balance/restskat timing/system-date-driven § 61 stk. 4/stk. 6 rateplans/
  overskydende skat compensation/dividend-tax credit posture, covered § 8 a
  source-rate provenance and share-income final-settlement scenarios with § 67
  dividend-tax credit
  splitting plus § 8 a, stk. 3 over-withheld/negative-share-income dividend-tax
  credits and § 8 a, stk. 6 both-negative spouse threshold allocation, covered
  § 8 b CFC tax source-rate provenance from the historic/current
  Selskabsskatteloven § 17, stk. 1 source line,
  § 8 c municipal-equivalent limited-taxpayer tax with
  personfradrag reduction, published 2023-2026 rate source/method provenance,
  and non-covered boundary case, covered fictional
  household scenario, covered Børne- og ungeydelse
  benefit-cliff/source-tension audit plus a first boligsikring § 22 threshold
  cliff, covered external Skat.dk 2026
  ordinary wage-earner fixture, topskat threshold activation,
  covered § 4 a share-income aggregation, exclusions, personal-income
  reclassification and pension deduction from positive share income,
  covered § 14 annualization and first wage-earner calculator integration plus
  the Den juridiske vejledning external annualisation example,
  covered first bomb-audit probes for § 6/§ 7/§ 8 a/§ 8 b/§ 13/§ 14/§ 19 daisy-chain tensions,
  covered § 19 skatteloft including the 2026
  44,57 pct. personal ceiling, 42 pct. positive-capital ceiling, and
  calculator-level wage-earner integration for both paths, including
  source-backed Langeland 2026 high-municipal-rate personal and positive-capital
  relief fixtures and the published 1,24 pct. SKM `Nedslag pct.` differential,
  covered § 20 regulation/rounding, covered § 26 transition
  compensation including a composed annual settlement path, pair-level stk. 4
  spouse difference offset, and stk. 7 spouse top-tax allocation for nr. 3,
  covered § 28 territorial exclusion, covered AM-law special cases,
  covered shared Pengebeløb rounding and øre-fraction posture,
  covered Opkrævningsloven payment-deadline/remittance posture and § 7
  rate-derivation fixture plus 2026 late-payment supplement source-chain
  amendment and date-exact daily interest context, covered B-skat installment
  calendar/rate-window projection, covered § 7 cross-calendar-year daily
  interest split, covered § 62 A interest fixtures, exposed and scheduled
  restskat remaining B-skat-rate minimum tension and system-start residual
  restskat collection,
  § 13 foreign/pension/business amount limitations are executable audit signals,
  including 2025 PBL § 16 behavior, 2026 repeal behavior, LL § 33 A relief,
  seamen-relief exceptions, and calculator-level § 13 integration signals over
  the domain-object calculator input.

M6 - Website integration

- Status: first slice implemented.
- Output: research page under the website showing the Personskatteloven corpus,
  source status, milestones, and selected audits.
- Done when: the website renders the checked corpus and clearly marks whether it
  is a calculation-ready slice or a research/audit slice.
- Current slice: `/research/personskatteloven` links the valid and historic
  sources, renders the milestone log, embeds the checked §§ 1-28 `.runa`
  corpus plus `.scenario.runa` executable scenarios and the `.audit.runa`
  audit suite, marks the shared Pengebeløb rounding posture, the limited
  wage-earner fixture slice, a source-backed external Skat.dk 2026 ordinary
  wage-earner fixture, the § 14/§ 19 external differential scenario, plus ordinary and
  special-case AM-law coverage, ordinary Ligningsloven deductions and § 15 Q
  subletting/letting surplus derivation, Kildeskatteloven
  A-income/withholding/e-skattekort/slutopgørelse/restskat timing and system-start rateplan posture,
  BEK 839 generated-card path, BEK 1094 2026 indeholdelsesprocent derivation,
  first § 1/§ 2 taxable-income composition from the separate income categories,
  first § 4 nr. 3/nr. 3 a/nr. 6/nr. 9/nr. 11/nr. 17 focused capital-income
  classification audits,
  first § 4 a pension/share-income and ABL bridge audits,
  first § 8 a/§ 67 share-income annual-settlement scenario including negative
  share-income carry-forward,
  first § 8 c limited-taxpayer municipal-equivalent tax calculation and
  published-rate source/method provenance,
  first § 11 negative net-capital reduction order and spouse-transfer audit,
  first § 9/§ 12 split state personfradrag tax-value reduction-order audit and
  wage-earner component projection,
  § 26 transition-compensation audit including stk. 7 spouse top-tax allocation,
  first § 6/§ 7/§ 8 a/§ 8 b/§ 13/§ 14/§ 19 bomb-audit probes, the Børne- og ungeydelse
  household benefit-cliff/source-tension probe plus a boligsikring § 22
  threshold-cliff probe,
  Opkrævningsloven payment-deadline, § 7 rate-derivation, date-exact daily
  interest-context, and cross-calendar-year interest-split slices, the B-skat
  calendar projection, system-date-driven § 61 stk. 4/stk. 6 rateplans,
  minimum-rate completion plan, § 62 A interest
  fixtures, § 14 partial-year wage-earner
  scenario, § 14 official guidance example, and personal plus positive-capital § 19 skatteloft inside the
  wage-earner breakdown, including the 2026 Langeland high-rate municipality
  fixture and published SKM `Nedslag pct.` differential, as calculation-ready, and marks the full statute model as
  research/audit-only.

M7 - Personfradrag and deficit layer

- Status: first slice implemented.
- Output: `.runa` file for §§ 10-13 plus calculator/audit integration for
  ordinary positive-income wage-earner cases.
- Done when: personfradrag amount selection, § 12 tax-value calculation,
  after-personfradrag fixture totals, and § 13 deficit boundary signals are
  executable.
- Current slice: adult 2025 personfradrag is pulled from the official
  tax-year parameter pack, state/municipal/church tax values are calculated,
  Copenhagen and Gentofte fixtures settle after personfradrag, § 13 deficit tax
  value and offset order are executable, § 10 stk. 5-6 eligibility for
  Kildeskatteloven § 2 taxpayers is modeled with choice/reversal deadlines and
  explicit sailor/residence-permit/researcher exclusions, § 10 stk. 3
  spouse transfer of unused state-personfradrag tax value is amount-modeled for
  the § 9 state-tax basket and year-end cohabitation condition, § 11 negative
  net-capital reduction covers source-backed stk. 2 rate provenance, spouse
  threshold pooling, spouse positive-capital offset, statutory tax-order
  reduction, and unused spouse transfer, § 9/§ 12
  split state personfradrag tax-value reduction across the state-tax basket and
  § 9 non-state personfradrag reduction for § 8 c/kommunal/kirkelig tax are
  now wired into the wage-earner calculator, spouse
  deficit transfer, § 13 stk. 4 negative personal income offsets through spouse
  personal income and both spouses' positive capital income, and carried-forward
  negative personal income ordering are fixture-tested,
  § 13 a debt-settlement reduction now lowers the debtor's carried deficits,
  then a closed union of ABL/KGL/EBL carried-loss results, then negative
  share-income tax at 40 pct., before any remaining reduction lowers a
  cohabiting spouse's business deficit after stk. 3 non-business netting;
  later-year loss results are rejected and EBL § 6 calculates its own
  exclusions, own/spouse offsets and remaining carry-forward,
  foreign/pension spouse transfer limitations are executable, and same-business
  loss carry-forward amounts are fixture-tested. § 13's first dependent-source validation now
  covers PBL § 16 through 2025, the 2026 repeal, LL § 33 A relief, and seamen
  relief. Complex § 13 calculator breakdown fixtures now cover spouse transfer,
  LL § 33 A relief, 2026 PBL repeal, and same-business carry-forward. The
  ordinary wage-earner calculator now also exposes a `LønmodtagerPar13UnderskudResult`
  for negative taxable-income cases, keeps municipal/church tax from becoming
  negative, applies the § 13 tax-value offset to state tax before § 9/§ 12
  personfradrag, and carries the unused tax value in the breakdown. Remaining
  work is broader calculator integration with complete 2026 parameters and
  external differential fixtures rather than the first § 13 amount formulas
  themselves.

M8 - Omregning, skatteloft, and regulation

- Status: first slice implemented.
- Output: `.runa` file for §§ 14-20 plus audit coverage for annualization,
  tax-ceiling relief, and statutory regulation rounding.
- Done when: partial-year annualization, repeal markers, personal/capital
  tax-ceiling rates, calculated ceiling relief, and § 20 rounded regulated
  amounts are executable.
- Current slice: § 14 converts partial-year income to whole-year equivalents
  rounded to whole kroner and reduces whole-year tax proportionally. A first
  ordinary wage-earner § 14 integration now annualizes a delårs wage-earner
  input and uses the state income-tax component after §§ 6-9, rather than the
  AM-inclusive total. The official guidance example now proves recurring-vs.
  one-off amount handling, and § 14 stk. 2 now has an executable election result
  for oplysningsskema election, timely reversal by 30 June in the second
  calendar year after the income year, late reversal, and the § 10 stk. 6
  limited-taxability path where the stk. 2 election is not available.
  §§ 15-18 are explicit repealed markers, § 19 computes
  personal and positive-capital tax ceiling excess and relief as typed
  `Par19SkatteloftNedslagResultat` objects, both personal and positive-capital
  § 19 relief now flow into the ordinary wage-earner breakdown for supported tax
  years and municipalities, the 2026 Langeland fixture proves a source-backed
  high municipal-tax ceiling case and matches the published 1,24 pct. `Nedslag
  pct.`, and § 20 now returns source-backed regulation-number
  results for statutory 2009-2013 values, SKM historical 2014-2024 values, and
  SKM 2025-2026 published values. The § 20 table now uses SKM's 2020-2024
  figures of 114,3, 116,9, 118,3, 121,8 and 126,1 before computing 2010-level
  amount regulation with round-up to the nearest 100 kroner.

M9 - Final provisions and transition compensation

- Status: first slice implemented.
- Output: `.runa` file for §§ 21-28 plus audit coverage for effective date,
  transition compensation, ministerial delegation, and territorial exclusion.
- Done when: repealed tail provisions, effect from income year 1987, § 26
  compensation and offset order, § 27 delegation, and § 28 exclusion of the
  Faroe Islands and Greenland are executable.
- Current slice: §§ 21-24 a, 25 a, 25 b, and 27 a are explicit repeal markers,
  § 25 applies from 1987, § 26 computes negative transition difference as
  compensation and applies it in statutory order, § 26 stk. 9 now regulates the
  2010-level thresholds through the § 20 rounding rule before deriving stk. 2
  line items from statutory bases, § 26 stk. 4-6 and stk. 8 now compute
  samlevende-ægtefælle offsets for positive/negative transition differences
  including pair-level post-stk. 4 compensation amounts, pair-level post-stk. 5
  and post-stk. 8 negative/positive net-capital interaction, and pair-level
  nr. 2 bundfradrag transfer with the § 48 F exception. An annual pair-level
  `Par26ForskelsbeløbParÅrsSag` now feeds the stk. 6 effective thresholds into
  the actual nr. 2 line items before the stk. 4 spouse difference offset.
  `Par26ForskelsbeløbParÅrsKomponentSag` now accepts raw annual personal-income,
  net-capital-income, and ligningsmæssige-fradrag components for both spouses,
  applies stk. 5 before the nr. 2 base, stk. 8 before the nr. 8 base using the
  § 11 threshold, and then delegates to the annual pair calculation.
  § 26 stk. 9 can now derive
  source-backed 2012-2019
  threshold packs from the official § 20 `reguleringstal`, § 26 nr. 5 can derive
  2012, 2017 and 2019 Ligningsloven §§ 9 J/9 K/9 L fradrag and the 4,25 pct.
  baseline from source-backed inputs, § 26 stk. 7 now applies § 7 stk. 5 and
  stk. 10-11 spouse capital rules when deriving nr. 3 for transition
  compensation, and `Par26KompensationAfregningResult` now composes an annual
  compensation calculation with the statutory tax-offset order. § 27 is encoded
  as `Par27BemyndigelseResultat` for delegated implementation/administration
  authority, and § 28 is encoded as `PersonskattelovTerritorialResultat` for
  Denmark/Faroe Islands/Greenland territorial scope and exclusion hjemmel.
  Remaining § 26 depth is mostly integration work: broader
  historic compensation fixtures, dependent-year settlement parameter wiring,
  and eventual wiring into a full historic tax-settlement calculator.
