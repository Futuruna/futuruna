# Arbejdsplan: Danmarks Riges Grundlov i Futuruna

## Formål

Grundlovskorpusset skal være en kildefast, idiomatisk og efterprøvbar
Futuruna-udgave af Danmarks Riges Grundlov. Det skal ikke blot gemme
facitsvarene fra bestemmelserne, men skildre deres aktører, roller, relationer,
kompetencer, betingelser, undtagelser og virkninger som en sammenhængende
regelmodel. Ordlyd, fortolkning og prøvning skal kunne læses sammen uden at
blive sammenblandet, og ethvert modeludsagn skal kunne føres tilbage til sit
grundlag.

Arbejdet omfatter det aktive korpus i `examples/danish-constitution`, den
tilhørende prøvningskode og websiderne om grundloven. Det historiske korpus i
`examples/danish-constitution-legacy` røres ikke.

## Ufravigelige principper

1. Den officielle lovtekst ændres kun ved en dokumenteret kilderettelse. Den
   eksisterende kontrolsum for de 91 kildetekstblokke er en fast port; en ny
   kontrolsum kræver officiel kilde, før- og eftertekst samt begrundelse.
2. Hver bestemmelse følger samme rækkefølge: ordret kildetekst, kun om
   nødvendigt en kort note, derefter de navngivne regler.
3. Dansk er korpussets og websidernes sprog. Engelske ord beholdes kun, når de
   er reserverede Futuruna-begreber eller stabile filendelser, eksempelvis
   `Meta`, `MetaRole`, `value`, `.audit.runa` og `.scenario.runa`.
4. `|` er udgangspunktet for retlige udsagn. `#` bruges til domænetyper, `=`
   til værdier og konstanter, og `>` kun til almindelig beregning uden
   selvstændigt normindhold.
5. Kildemodellen må ikke tillægge teksten mere, end teksten siger.
   Fortolkningsmodeller må gerne gå videre, når fortolkningen er navngivet,
   dens forudsætninger og retskilder er synlige, og den prøves selvstændigt.
6. Én retlig sammenhæng skal have én kanonisk repræsentation. Hjælperegler
   afledes fra den; de må ikke skabe parallelle sandhedskilder.
7. Prøvninger giver fortolkninger modelmæssig styrke ved at afdække
   konsekvenser, konflikter og modeksempler. Retskilder giver fortolkninger
   juridisk vægt; prøvninger kan ikke erstatte dem.
8. Domænemodellen optimeres for retvisende begreber og relevante juridiske
   spørgsmål, ikke for abstrakt genbrug. Den skal være dyb nok til at kunne
   forespørges og prøves, men ikke større end grundlovsdomænet kræver.

## Udgangspunkt

Den aktive udgave har allerede et stærkt kildegrundlag:

- 89 paragraffer samt indledning og stadfæstelse er fordelt på 11 kapitelfiler.
- Alle 91 kildetekstblokke har et entydigt metadataanker og et tilsvarende
  kodespænd.
- Retsinformation er primær tekstkilde, og Folketingets tekstvisning er
  støttekilde.
- Den ordrette tekst er beskyttet af en fast kildekontrolsum.
- Den fortolkede prøvning, genereret Rust og metadataindekset har tidligere
  været gennemført samlet.

Det næste arbejde er derfor ikke en ny transskription. Det er en systematisk
forbedring af domænemodellen, prøvningsstyrken og den offentlige fremstilling.

## Fagligt grundlag

Arbejdsplanen bruger fire etablerede retninger som kontrol mod lokale
smagsvalg:

- Eric Evans beskriver en domænemodel som et system af abstraktioner for
  udvalgte aspekter af et domæne inden for en afgrænset kontekst. Hans
  retningslinjer om fælles fagsprog, eksplicitte begreber, invarianter og
  begrebsmæssige konturer bruges til at vurdere modellens dybde:
  <https://www.domainlanguage.com/wp-content/uploads/2016/05/DDD_Reference_2015-03.pdf>
- OASIS LegalRuleML kræver sporbarhed mellem tekstdele og regler og skelner
  blandt andet mellem aktør, funktion og rolle, normtyper, undtagelser,
  regelkonflikter, tidslighed og konkurrerende fortolkninger:
  <https://docs.oasis-open.org/legalruleml/legalruleml-core-spec/v1.0/os/legalruleml-core-spec-v1.0-os.html>
- Catala viser, at lovens grundregler og undtagelser bør følge kildens
  struktur, og at prioritering mellem overlappende undtagelser selv er en
  juridisk fortolkningshandling:
  <https://book.catala-lang.org/en/2-2-conditionals-exceptions.html>
- Alloy skelner mellem fundne modeksempler og fravær af modeksempler inden for
  et erklæret, afgrænset søgerum. Samme ærlighed skal gælde Futurunas
  prøvningsresultater:
  <https://alloytools.org/spec.html>

Ingen af kilderne leverer én universelt perfekt model. Den relevante standard
er i stedet en model, der er kildefast, forklaringsstærk, falsificerbar, egnet
til præcise forespørgsler og tilpasset netop grundlovens regelrum.

## Aktuel diagnose

### Domænemodellen

De fælles typer `Statsmagt` og `Institution` findes allerede, men flere
relationer er stadig udtrykt som uafhængige nulargumentregler eller rå
booleske parametre. Det gør lovens helheder sværere at se og gør det muligt at
indføre modstridende fakta uden en samlet typekontrol.

Følgende mønstre skal findes i samtlige kapitler:

- flere løse fakta, der tilsammen beskriver ét begreb eller én proces;
- rå `Boolsk`-parametre, hvor værdierne har selvstændig retlig betydning;
- regler med mange parametre, som burde modtage et navngivet domæneobjekt;
- samme forhold lagret i flere regler i stedet for afledt fra én regel;
- generelle ord som `Institution`, hvor et mere præcist fagord findes;
- regler, der uden kilde tilføjer ord som "alene", "altid" eller "stærkere".

En nulargumentregel er ikke i sig selv et problem. Den beholdes, når
bestemmelsen faktisk udtrykker et enkelt atomisk udsagn. Der indføres ikke
domæneobjekter alene for ensartethedens skyld.

### Prøvningslaget

`grundlov.audit.runa` rummer både scenariedata, lokale
overensstemmelseskontroller, tværgående modelkontroller og juridisk farvede
konklusioner. Ordene "bevis", "paradoks" og "revision" bruges flere steder
mere vidtgående, end den udførte kontrol kan bære.

Den kendte topologikontrol har desuden 49 endnu ikke klassificerede
regelfamilier og en dækning på 27 procent af de booleske regler. Tallet er ikke
et mål i sig selv. Hvert hul skal enten få en meningsfuld kontrol eller
registreres som bevidst uden en kunstig tautologi.

### Websiderne

Grundlovssiden åbner i dag med tekniske forklaringer, optællinger og tre
fremhævede "Ny audit"-henvisninger. Prøvningssiden bruger kort, engelske
metadata og bastante formuleringer som "Revisionen beviser". Resultatet ligner
en produktpræsentation mere end en nøgtern kilde- og kodeudgave.

Det synlige ord "grundlovsrevision" er desuden upræcist her, fordi det normalt
kan forstås som en ændring af grundloven. Den offentlige betegnelse skal være
"prøvning af grundlovsmodellen" eller kort "grundlovsprøvning".

## Tre adskilte modellag

Grundlovsprojektet organiseres omkring tre lag, som kan importere det
foregående lag, men aldrig skrive det om.

### Kildemodellen

Kildemodellen er den ordlydsnære regelmodel i kapitelfilerne. Den indeholder de
begreber og relationer, der er nødvendige for at gøre bestemmelserne
eksekverbare og tilgængelige for præcise opslag, men ingen skjulte doktrinære
antagelser. Hver regel er knyttet til den eller de tekstdele, den formaliserer.

### Fortolkningsmodellerne

En fortolkning er tilladt som en førsteklasses del af projektet, når den ligger
i et navngivet Futuruna-regelscope eller et særskilt fortolkningsmodul. Den skal
angive:

- hvilke kilderegler den fortolker;
- fortolkningsspørgsmålet og den valgte forståelse;
- nødvendige forudsætninger;
- supplerende retskilder eller en tydelig markering af, at forståelsen alene er
  en arbejdshypotese;
- hvilke andre navngivne fortolkninger den konkurrerer med;
- hvilke scenarier og kontroller der prøver den.

Flere uforenelige fortolkninger må eksistere samtidigt. Valget af én
fortolkning i et scenarie skal være eksplicit; den må ikke vinde alene på grund
af importorden eller et skjult standardvalg.

### Prøvningsmodellen

Prøvningsmodellen indeholder scenarier, invarianter, grænseprøver og søgning
efter modeksempler. Den prøver både kildemodellen og hver navngiven
fortolkningsmodel. Resultatet skal altid oplyse, hvilket modellag og hvilket
prøvningsomfang det gælder.

En fortolkning kan optages som en understøttet model, når den er
sammenhængende, dens regler er dækkende og entydige i det erklærede domæne,
dens undtagelser er nåelige, og den har overlevet relevante normale,
grænseprøvende og modstridssøgende scenarier. Det gør fortolkningen
modelmæssigt velunderbygget, ikke automatisk til gældende ret.

## Målbillede for domænemodellen

### Eksempel: § 3

Forslaget om at samle magtfordelingen er rigtigt. En liste som
`magt([Kongen, Folketinget], Lovgivende)` er dog ikke tilstrækkelig præcis:
listen udtrykker medlemskab, men ikke ordene "i forening", og den tillader
rækkefølge, dubletter og vilkårligt antal deltagere.

Den foretrukne retning er en udtømmende, typet relation. Personen `Monark`
holdes adskilt fra `KongenSomStatsorgan`, fordi regler om eksempelvis alder og
tro vedrører en person, mens § 3 placerer statsmagt hos et statsorgan:

```runa
# Statsmagt = Lovgivende | Udøvende | Dømmende
# Statsorgan = KongenSomStatsorgan | Folketinget | Domstolene
# Udøvelsesform = Hos(organ: Statsorgan) | IForening(organ: Statsorgan, medorgan: Statsorgan)
# Magtplacering(magt: Statsmagt, udøvelse: Udøvelsesform)

| statsmagtens_placering(magt: Statsmagt) -> match magt {
    | Lovgivende -> Magtplacering(
        magt = Lovgivende,
        udøvelse = IForening(
            organ = KongenSomStatsorgan,
            medorgan = Folketinget
        )
    )
    | Udøvende -> Magtplacering(
        magt = Udøvende,
        udøvelse = Hos(organ = KongenSomStatsorgan)
    )
    | Dømmende -> Magtplacering(
        magt = Dømmende,
        udøvelse = Hos(organ = Domstolene)
    )
}
```

`Statsorgan` foretrækkes frem for `MagtHaver`: typen står i ental, og navnet
beskriver grundlovens institutionelle placering uden at foregribe, hvordan den
formelle magt udøves i nutidig statsretlig praksis. `IForening` bevarer det
afgørende forhold, som en flad liste ville tabe.

Den generelle `Udøvelsesform` foretrækkes frem for tre specialvarianter som
`HosKongenOgFolketingetIForening`: specialvarianterne ville gøre ugyldige
tilstande sværere at konstruere, men de ville blot kode de tre aktuelle svar og
svække modellens evne til at udtrykke og undersøge relationerne. Gyldigheden af
en `IForening`-værdi håndhæves derfor med kanoniske konstruktionsregler og
invarianter om forskellige deltagere og korrekt antal.

Hvis opslag som `udøver(organ, magt)` fortsat er nyttige, skal de afledes fra
`statsmagtens_placering`. De fire nuværende fakta og de to særskilte
`lovgivning_kræver`-fakta må ikke fortsætte som uafhængige sandhedskilder.

Kontrollerne for § 3 skal mindst fastslå:

- at hver `Statsmagt` har præcis én placering;
- at den lovgivende magt er placeret hos kongen og Folketinget i forening;
- at den udøvende og dømmende magt har de tekstnære placeringer;
- at `Monark` og `KongenSomStatsorgan` ikke kan bruges i hinandens regler uden
  en udtrykkelig rolleforbindelse;
- at `IForening` ikke kan indeholde samme statsorgan to gange;
- at eventuelle afledte opslagsregler er konsistente med den kanoniske regel;
- at modellen ikke indfører ordet "alene", som ikke står i § 3.

### Fælles modelleringsstandard

Hver bestemmelse gennemgås med følgende spørgsmål:

1. Hører udsagnet til kildemodellen eller en navngiven fortolkningsmodel?
2. Hvilke retligt betydende aktører, roller, institutioner, genstande,
   hændelser og udfald findes i ordlyden?
3. Er samme ord brugt om en person, et embede, et statsorgan eller en rolle i
   forskellige sammenhænge?
4. Er reglen konstituerende, kompetencetildelende, pligtpålæggende,
   forbydende, tilladende eller procedurefastsættende?
5. Hvilke begreber er lukkede alternativer og bør være sumtyper?
6. Beskriver flere parametre i virkeligheden én sag, beslutning, relation eller
   proces?
7. Hvilken regel er den kanoniske sandhedskilde, og hvilke juridisk relevante
   spørgsmål skal modellen kunne besvare ud fra den?
8. Hvilke betingelser hører til reglen med `under`, og hvilke er egentlige
   navngivne `exception`-regler?
9. Kan to regler eller undtagelser gælde samtidigt, og er deres forrang i så
   fald udtrykkeligt begrundet?
10. Er modaliteten bevaret: "skal", "kan", "må ikke" og "bør" må ikke gøres
    semantisk ens?
11. Kan en ugyldig tilstand gøres urepræsenterbar med en type, en kanonisk
    konstruktionsregel eller en invariant?
12. Er enhver bekvemmelighedsregel afledt og prøvet mod den kanoniske regel?

Navngivne konstruktørargumenter bruges ved alle ikke-trivielle domæneværdier.
Store universelle typer undgås; typer placeres fælles, når de faktisk deles på
tværs af kapitler, og ellers ved den bestemmelse, der ejer begrebet.

Konstruktørnavne skal være entydige i det samlede korpus. Når samme ord indgår
i to forskellige juridiske roller, indgår rollen i navnet, eksempelvis
`AfstamningSomLigebehandlingsgrund` og
`AfstamningSomFrihedsberøvelsesgrund`. En samlet audit må ikke afhænge af
importorden for at afgøre, hvilken domæneværdi et navn betegner.

Regler, der alene vurderer felterne i ét domæneobjekt, placeres som udgangspunkt
i objektets regelscope og kaldes gennem objektet. Eksterne regler beholdes til
relationer mellem selvstændige objekter eller til atomiske udsagn uden et
naturligt ejerobjekt. Samme forhold må ikke både eksistere som intern og
ekstern sandhedskilde.

En betingelse, der udløser en procedure, må ikke sammenblandes med et fuldført
forløb eller dets endelige retsvirkning. Modellen navngiver derfor særskilt,
hvornår et krav udløses, hvornår de efterfølgende procesled er opfyldt, og
hvornår resultatet får den virkning, ordlyden foreskriver.

Tærskelregler validerer først, at antal og delmængder er konsistente. Derefter
bevares forskellen mellem strenge og inklusive grænser, eksempelvis mellem
"et flertal" og "mindst 40 pct.". En bestået tærskel kan ikke reparere et
umuligt stemmetal.

Kalenderfrister må ikke omsættes til grove heltalsenheder, hvis det skaber
falsk præcision ved grænsen. Indtil modellen har en defineret kalendersemantik,
bruges en lukket, tekstnær tidsrelation som "inden et halvt år"; egentlige
datoer bruges først til beregning, når sammenligningen selv er specificeret og
prøvet.

Overgangsbestemmelser modelleres som navngivne faser med en udtrykkelig
ophørshændelse. Når ny og tidligere ret efter ordlyden gælder samtidigt i et
afgrænset område, bevares denne samtidighed i modellen frem for at blive
presset ned i ét samlet ikrafttrædelsesflag.

En god type er ikke nødvendigvis den type, der tillader færrest værdier. Den
skal først og fremmest udtrykke domænets naturlige begreb og understøtte de
spørgsmål, prøvningen skal stille. Ugyldige kombinationer kan udelukkes af
typen, men også af én central konstruktionsregel og synlige invarianter, når en
mere lukket type ellers ville overtilpasse modellen til de nuværende
eksempler.

Modellen gennemgås løbende for begrebsmæssige konturer: begreber, der ændres,
prøves og forklares sammen, bør normalt ligge sammen; begreber med forskellige
roller eller forandringsakser bør ikke klemmes ind i samme type. Det fælles
danske fagsprog i lovtekst, typer, regler, scenarier og webtekst er en del af
selve kvalitetskontrollen.

## Kilder og metadata

Det korte anker bevares:

```runa
--@label:grundlov_par3::meta:grundlov_kildemetadata--
----
§ 3. ...
----

--@begin:grundlov_par3--
...
--@end:grundlov_par3--
```

Metadataoplysninger skal ligge i almindelige typer og værdier, ikke vokse ind
i kommentarsyntaksen. Den fælles metadata skal forbedres sådan:

- brugerdefinerede typer, varianter og felter får danske navne;
- reserverede protokolnavne beholdes kun, hvor Futuruna kræver dem;
- label og kodespænd bruges som bestemmelsesidentitet, når de allerede giver
  metadataindekset den nødvendige sporbarhed;
- særskilte metadataværdier pr. bestemmelse indføres kun, når de bærer en
  reel oplysning, som ikke sikkert kan afledes af label, kodespænd og fælles
  kildemetadata;
- primærkilde og støttekilde får tydeligt forskellige roller;
- hentningsdato forbliver en typet dato;
- fortolkningsmodeller får metadata om navn, grundlag, forudsætninger,
  konkurrerende fortolkninger og supplerende retskilder;
- prøvningsfund får metadata om modellag, udsagnsstatus og prøvningsomfang og
  genbruger ikke ukritisk bestemmelsens kildemetadata;
- `refof(...)` bruges til kontrollerede henvisninger til de regler, et fund
  bygger på, så omdøbninger og manglende mål opdages ved kontrol.

Primær tekstkilde er:

- Retsinformation, `LOV nr. 169 af 05/06/1953`:
  <https://www.retsinformation.dk/eli/lta/1953/169>

Støttekilden er:

- Folketingets tekstvisning af Danmarks Riges Grundlov:
  <https://www.ft.dk/da/dokumenter/bestil-publikationer/publikationer/grundloven/danmarks-riges-grundlov>

Fortolkende offentlige udsagn må efter behov tilføje særskilte officielle
forarbejder, domme eller anden autoritativ statsretlig dokumentation. Sådanne
kilder må aldrig præsenteres som en del af selve lovteksten.

### Kontrollerede kilderettelser

Kildekontrolsummen beskytter mod lydløse ændringer, men må ikke fastholde en
opdaget transskriptionsfejl. En kilderettelse skal være sin egen afgrænsede
ændring og dokumentere:

1. den hidtidige tekst;
2. den korrigerede tekst;
3. den præcise officielle kilde og dens hentningsdato;
4. hvorfor forskellen er en kilderettelse og ikke en modernisering eller
   fortolkning;
5. den nye forventede kontrolsum;
6. hvilke regler og prøvninger der eventuelt påvirkes.

Ingen almindelig domæneomlægning må samtidig ændre tekstblokkene.

## Prøvningsmodel

Alle ikke-trivielle fund klassificeres efter modellag, udsagnsstatus og
prøvningsomfang. Den endelige metadatatype skal følge Futurunas generiske
metaprotokol, men domænebegreberne er:

```runa
# Modellag = Kildemodel | Fortolkningsmodel(navn: Tekst)

# Udsagnsstatus = TekstnærKontrol | Modelresultat | Fortolkningsspørgsmål | RetskildestøttetKonklusion

# Prøvningsomfang = EnkeltScenarie | Scenariesamling | Grænseanalyse | UdtømmendeLukketDomæne | AfgrænsetSøgning(beskrivelse: Tekst)
```

- `TekstnærKontrol` kontrollerer, at en udtrykkelig bestemmelse er korrekt
  repræsenteret.
- `Modelresultat` følger af de kodede regler, men gælder først og fremmest
  modellen.
- `Fortolkningsspørgsmål` viser en spænding eller en uafklaret grænse, som
  ordlyden ikke alene afgør.
- `RetskildestøttetKonklusion` kræver særskilte, angivne retskilder ud over den
  blotte modelkørsel.

`Prøvningsomfang` beskriver evidensen, ikke hvor interessant fundet er. Et
fundet modeksempel gælder konkret. Fravær af modeksempler gælder kun for det
erklærede scenariesæt eller søgerum, medmindre domænet er lukket og faktisk
gennemløbet udtømmende.

Hvert offentliggjort fund skal vise:

1. den relevante ordlyd og dens kildelabel;
2. de regler, prøvningen faktisk kalder;
3. kildemodel eller navngiven fortolkningsmodel;
4. den konkrete kontrol, det mindste vidne eller det mindste modeksempel;
5. udsagnsstatus og prøvningsomfang;
6. hvad resultatet viser;
7. hvad resultatet ikke afgør;
8. eventuelle supplerende retskilder.

`?` omtales som en kontrol eller invariant, ikke automatisk som et juridisk
bevis. "Paradoks" bruges kun, hvis modellen viser en formel modstrid, hvor de
relevante udsagn ikke samtidig kan opfyldes. Ellers bruges "spænding",
"asymmetri", "åbent fortolkningsspørgsmål" eller "modelgrænse".

### Generelle modelprøvninger

Ud over bestemmelsesspecifikke scenarier skal hvert sammenhængende regelområde
prøves for:

- **opfyldelighed:** findes der mindst én gyldig tilstand, eller har modellen
  gjort sit eget domæne umuligt?
- **dækning:** får alle tilsigtede input et resultat, eller findes der huller?
- **entydighed:** giver flere samtidige regler forskellige resultater uden en
  begrundet forrang?
- **undtagelsesrækkevidde:** kan hver undtagelse aktiveres, og gentager den alle
  betingelser, som dens juridiske rækkevidde kræver?
- **skygning:** gør en generel regel en mere specifik regel utilgængelig?
- **invarians:** bevarer alle konstruktionsveje domæneobjekternes erklærede
  krav?
- **relationel konsistens:** er afledte opslag og omvendte relationer enige med
  den kanoniske regel?
- **fortolkningsadskillelse:** kan to fortolkninger køres hver for sig uden
  skjult påvirkning fra hinanden?
- **negativ afgrænsning:** viser en manglende henvisning kun, at bestemmelsen
  ikke er nævnt, eller er der fejlagtigt udledt en positiv rettighed eller et
  generelt forbud af tavsheden?

Tautologier som `regel() -> regel()` dokumenterer kun, at et navn kan kaldes.
De tæller ikke som prøvning af lovmodellens betydning.

### To fund, der skal genbehandles

**§§ 6 og 70, monarkens troskrav:** Den nuværende tekst kalder forholdet en
direkte selvmodsigelse og bruger en muslimsk tronfølger som bevis. Modellen kan
vise, at § 6 opstiller et troskrav til kongen, at den kodede § 70 er generelt
formuleret, og at arvefølgen ikke har fået indlagt samme prøve. Om § 70 retligt
begrænser tronfølgen eller kongens embede, kræver fortolkning. Fundet skal
derfor offentliggøres som `Fortolkningsspørgsmål`, indtil stærkere retskilder
underbygger en konklusion. Prøvningen skal kunne sammenligne mindst en model,
hvor § 6 behandles som en særregel for embedet, og en model, hvor § 70 også
anvendes på adgangen til embedet. Ingen af dem må være et skjult standardvalg.

**§§ 43 og 73, skat og ekspropriation:** Modellen kan vise, at ordlyden ikke
selv opstiller en beregnelig grænse mellem skattepålæg og
ejendomsafståelse. Den kan ikke alene fastslå, at en skat på 100 procent er
forfatningsmæssig eller omgår § 73. Den påstand afhænger af
forfatningsretlig kvalifikation og praksis. Fundet skal beskrives som en
tekstlig modelgrænse og et fortolkningsspørgsmål. Fortolkningsmodeller kan
afprøve forskellige kvalifikationskriterier og deres konsekvenser, men hvert
kriterium skal have sit eget grundlag og må ikke skrives ind i §§ 43 eller 73
som ordlydsnær regel.

## Filstruktur

Kapitelfilerne beholdes som den primære juridiske opdeling. Prøvningslaget
splittes efter ansvar, så scenarier, lokale kontroller og tværgående analyser
ikke længere ligger i én fil:

```text
examples/danish-constitution/
  grundlov-faelles.runa
  kapitel-01.runa ... kapitel-11.runa
  fortolkninger/
    grundlov-fortolkning-faelles.runa
    *.fortolkning.runa
  grundlov-proevningsscenarier.scenario.runa
  grundlov-bestemmelser.audit.runa
  grundlov-procedurer.audit.runa
  grundlov-rettigheder.audit.runa
  grundlov-tvaergaaende.audit.runa
  grundlov.audit.runa
  arbejdsplan.md
```

`grundlov.audit.runa` bliver en tynd samlet indgang, der importerer de øvrige
prøvningsfiler og kører den samlede kontrol. Filendelserne følger Futurunas
værktøjskonvention; filnavnenes faglige dele er danske.

Fortolkningsfiler oprettes kun for faktiske fortolkningsspørgsmål. De må ikke
blive en parallel kopi af de 11 kapitler. Den fælles fortolkningsfil ejer alene
de typer og metadata, som flere navngivne fortolkninger deler.

## Gennemførelsesforløb

### Trin 0: Frys grundlaget

- Registrer den nuværende kildekontrolsum, de 91 labels og alle eksisterende
  kontroller som sammenligningsgrundlag.
- Kør metadataindekset og gem en maskinlæsbar oversigt over label, kodespænd og
  symboler.
- Registrer de 49 topologiske dækningshuller efter bestemmelse.
- Registrer de juridiske spørgsmål, modellen skal kunne besvare, så
  domæneomlægningen styres af faktiske forespørgsler og ikke kun af
  typeæstetik.
- Fastlæg proceduren for kontrollerede kilderettelser og bekræft, at ingen er
  blandet ind i den første domæneomlægning.
- Fastslå, at det historiske korpus er uden for ændringsområdet.

### Trin 1: Fælles ordforråd og kapitel I-III

- Omdøb `Institution` til `Statsorgan`, adskil `Monark` fra
  `KongenSomStatsorgan`, og indfør den kanoniske relationsmodel for § 3.
- Gennemgå monark, tronfølge, regentskab, regering, ministre, statsråd,
  samtykke og beslutning som sammenhængende domæner.
- Indfør de fælles typer for modellag, fortolkningsgrundlag og
  prøvningsomfang, men opret ikke fortolkningsfiler uden et konkret spørgsmål.
- Flyt kun typer til `grundlov-faelles.runa`, når mindst to kapitler ejer samme
  begreb.
- Tilføj målrettede kontroller efter hver sammenhængende ændringsgruppe.

### Trin 2: Kapitel IV-V

- Modellér valgret, valgbarhed, valgperiode, møder, beslutningsdygtighed,
  lovgivningsproces og folkeafstemning som navngivne sager og udfald frem for
  kæder af rå booleske værdier.
- Gør tærskler til fælles, typede begreber, hvor de faktisk har samme
  matematiske struktur; antag ikke samme retlige betydning alene på grund af
  samme brøk.
- Prøv alle grænseværdier umiddelbart under, på og over tærsklen.

### Trin 3: Kapitel VI-VIII

- Gennemgå Rigsretten, domstolenes prøvelse, dommeruafhængighed,
  religionsforhold og grundrettigheder.
- Modellér rettighed, indgreb, hjemmel, prøvelse og undtagelse som adskilte
  begreber, hvor ordlyden kræver det.
- Fjern rangordninger som "stærkere" eller "svagere" fra lovmodellen, medmindre
  sammenligningskriteriet er eksplicit og hører til prøvningslaget.
- Genbehandl alle nuværende "paradokser" efter udsagnsstatus.

### Trin 4: Kapitel IX-XI og samlet konsistens

- Gennemgå den kommunale valgretsalder, islandske statsborgeres
  overgangsrettigheder, grundlovsændring, ikrafttræden og stadfæstelse i
  §§ 86-89 og den afsluttende stadfæstelsestekst.
- Kontroller krydshenvisninger, frister, tællinger, modalitet og
  delegationsregler på tværs af alle kapitler.
- Klassificer hvert resterende topologisk dækningshul. Dæk kun huller med en
  meningsfuld egenskab; dokumenter resten som bevidste.

### Trin 5: Fortolknings- og prøvningslag

- Flyt alle scenarieværdier til `.scenario.runa`.
- Del lokale overensstemmelseskontroller, procedurekontroller,
  rettighedskontroller og tværgående undersøgelser i hver sin `.audit.runa`.
- Flyt fortolkende regler ud af kildemodellen og ind i navngivne regelscopes
  eller fortolkningsfiler med tydeligt grundlag.
- Modellér konkurrerende fortolkninger side om side, hvor ordlyden faktisk
  tillader dem, og kræv et eksplicit valg i scenarier.
- Giv hvert væsentligt fund typet metadata med modellag, udsagnsstatus,
  prøvningsomfang og kontrollerede programhenvisninger.
- Tilføj de generelle prøvninger af opfyldelighed, dækning, entydighed,
  undtagelsesrækkevidde, skygning og relationel konsistens.
- Erstat tautologier som `regel() -> regel()` med egentlige egenskaber eller
  klassificer dem som udækkede.
- Bevar én samlet kørbar indgang til hele prøvningen.

### Trin 6: Gør websiderne nøgterne

Hovedsiden `/research/danish-constitution` skal være en kilde- og kodeudgave,
ikke et kontrolpanel:

- første skærmbillede viser titlen `Danmarks Riges Grundlov`, en kort dansk
  beskrivelse, primærkilden og en diskret teksthenvisning til prøvningssiden;
- statistiklinjen og de tre "Ny audit"-kort fjernes;
- forklaringen af metadata-syntaks flyttes til en kort sektion "Om udgaven"
  efter indledningen eller udelades, hvis kodevisningen er selvforklarende;
- "multiline source blocks" erstattes med almindeligt dansk;
- hvert kapitel vises fortsat med ordlyden før reglerne;
- introduktionsteksten flyttes ud af en stor Rust-streng og ind i en dansk
  indholdsfil, som websiden indlæser;
- ingen synlig tekst forklarer, at siden er på dansk. Dansk er udgangspunktet.

Den eksisterende adresse `/research/danish-constitution-audit` beholdes for
stabile henvisninger, men den synlige side omdøbes til "Prøvning af
grundlovsmodellen":

- engelsk sidetitel, metabeskrivelse og sprognote omskrives til dansk;
- de tre fremhævelseskort fjernes;
- indledningen forklarer kort forskellen mellem ordlyd, model og fortolkning;
- hvert fund følger den faste struktur fra prøvningsmodellen;
- fund, kildelinks, modellag og prøvningsomfang afledes så vidt muligt fra den
  typede metadata i stedet for at blive skrevet igen i Rust;
- siden oplyser den valgte fortolkningsmodel, når et resultat ikke alene kommer
  fra kildemodellen;
- bastante eller sensationsprægede overskrifter erstattes med præcise
  beskrivelser;
- optællinger offentliggøres kun, når de beregnes fra den viste version og har
  selvstændig betydning;
- oversigtskortet på `/research` får samme nøgterne danske ordvalg.

### Trin 7: Samlet juridisk og teknisk gennemgang

- Læs hver kildetekstblok og dens kodespænd som ét par.
- Gennemfør en særskilt gennemgang af navne, modalitet, betingelser,
  undtagelser og utilsigtede fortolkninger.
- Kør alle tekniske porte på både fortolker og genereret Rust.
- Gennemgå websiderne visuelt på smal og bred skærm.
- Dokumenter kendte fortolkningsspørgsmål uden at fremstille dem som fejl i
  grundloven.

## Kontrolrytme

Kontrollerne køres i lag, så arbejdet kan udføres i meningsfulde grupper uden
en fuld samlet kørsel efter hver lille rettelse:

1. **Efter en lokal ændringsgruppe:** formatering, syntaks, typekontrol og den
   nærmeste berørte scenario- eller prøvningsfil.
2. **Efter et kapitel eller et sammenhængende domæne:** alle scenarier og
   prøvninger for området samt metadata- og kildekobling.
3. **Ved afslutningen af hvert gennemførelsestrin:** samlet fortolket
   grundlovsprøvning og topologirapport.
4. **Ved udgivelsesmilepæle:** genereret Rust, samlet metadataindeks,
   kildekontrolsum, webbygning og visuel kontrol på smal og bred skærm.

En fuld port må gerne afsløre en gruppe fejl, som derefter rettes samlet. Den
bruges ikke som erstatning for de hurtige, målrettede kontroller under
modelleringen.

Frontendkontrol er den hurtige lokale port, men er ikke tilstrækkelig som
milepælsport. Regelscope-metoder, importer og genereret kode skal også gennem
Rust-backenden, fordi en frontend kan acceptere en metodeform, som backenden
ikke kan generere. En sådan uoverensstemmelse registreres som compilerfejl;
korpusset skjuler den ikke med kompatibilitetsaliaser.

## Kvalitetsporte

Arbejdet er først færdigt, når følgende holder samtidigt:

- alle 91 officielle kildetekstblokke har den godkendte kontrolsum, og enhver
  ændring siden udgangspunktet har fulgt kilderettelsesproceduren;
- alle 91 labels er entydige og har præcis ét sammenhængende kodespænd;
- metadataindekset kan føre hvert retligt symbol tilbage til bestemmelsen og
  dens kilder;
- hver fortolkende regel ligger i en navngiven fortolkningsmodel med synligt
  grundlag og kan prøves uden de konkurrerende fortolkninger;
- samtlige aktive `.runa`-filer består syntaks-, type- og navnekontrol;
- den samlede grundlovsprøvning består i fortolkeren;
- samme prøvning består som genereret Rust;
- § 3 har én udtømmende magtfordeling og ingen parallelle løse fakta;
- hvert centralt regelområde er prøvet for opfyldelighed, dækning, entydighed,
  undtagelsesrækkevidde og relationel konsistens;
- alle ikke-trivielle offentlige fund har modellag, udsagnsstatus,
  prøvningsomfang, programhenvisninger og nødvendige retskilder;
- alle dækningshuller i den afsluttende topologirapport er enten meningsfuldt
  dækket eller begrundet som bevidste;
- det aktive korpus og de synlige grundlovssider indeholder ikke undgåeligt
  engelsk eller blandingssprog;
- websidens fund og optællinger kan føres tilbage til den viste korpusversion
  og er ikke uafhængige kopier af oplysninger i `.runa`-filerne;
- websiderne viser ingen forældede optællinger eller påstande om juridiske
  beviser, som maskinkørslen ikke kan bære;
- det historiske korpus er urørt.

## Afgrænsning

Arbejdet skal ikke:

- modernisere eller sprogligt rette grundlovens ordlyd;
- simulere hele dansk forfatningsret alene ud fra grundlovsteksten;
- indbygge politiske eller doktrinære vurderinger i kildemodellen; sådanne
  vurderinger må kun optræde som navngivne, begrundede fortolkninger;
- behandle en fortolkning som juridisk autoritativ alene, fordi dens scenarier
  og invarianter består;
- maksimere kontroldækning med tautologier;
- samle hele grundloven i én stor domænetype;
- ændre offentlige adresser uden en kompatibel viderestilling;
- ændre Futuruna-sproget, medmindre en konkret, generel sprogbegrænsning
  forhindrer den idiomatiske model. En sådan begrænsning skal isoleres og
  vurderes som et selvstændigt sprogarbejde.

## Færdigdefinition

Grundlovsarbejdet er færdigt, når en læser kan gå fra den officielle ordlyd til
den kanoniske kilderegel, videre til en eventuel navngiven fortolkning og
derfra til dens scenarier, invarianter, modeksempler og retskilder. Læseren skal
kunne se, hvilket modellag et udsagn tilhører, hvorfor domænebegreberne har den
valgte form, hvilket prøvningsomfang et resultat dækker, og hvilke spørgsmål
modellen endnu ikke afgør.

Webudgaven skal afspejle samme disciplin: først grundloven og kildemodellen,
dernæst de eksplicit valgte fortolkninger og til sidst den afgrænsede prøvning.
Det endelige resultat er ikke en statisk oversættelse, men en sammenhængende,
falsificerbar lovmodel, der kan besvare præcise forespørgsler og udvikles uden
at miste sin forbindelse til ordlyden.
