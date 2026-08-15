# Arbejdsplan: Danmarks Riges Grundlov i Futuruna

## Formål

Grundlovskorpusset skal være en kildefast, idiomatisk og efterprøvbar
Futuruna-udgave af Danmarks Riges Grundlov. Ordlyden skal kunne læses uden
forstyrrende fortolkning, reglerne skal udtrykke den samme struktur med så få
ugyldige tilstande som muligt, og enhver prøvning skal kunne føres tilbage til
de bestemmelser og øvrige retskilder, den bygger på.

Arbejdet omfatter det aktive korpus i `examples/danish-constitution`, den
tilhørende prøvningskode og websiderne om grundloven. Det historiske korpus i
`examples/danish-constitution-legacy` røres ikke.

## Ufravigelige principper

1. Den officielle lovtekst ændres ikke. Den eksisterende kontrolsum for de 91
   kildetekstblokke er en fast port gennem hele arbejdet.
2. Hver bestemmelse følger samme rækkefølge: ordret kildetekst, kun om
   nødvendigt en kort note, derefter de navngivne regler.
3. Dansk er korpussets og websidernes sprog. Engelske ord beholdes kun, når de
   er reserverede Futuruna-begreber eller stabile filendelser, eksempelvis
   `Meta`, `MetaRole`, `value`, `.audit.runa` og `.scenario.runa`.
4. `|` er udgangspunktet for retlige udsagn. `#` bruges til domænetyper, `=`
   til værdier og konstanter, og `>` kun til almindelig beregning uden
   selvstændigt normindhold.
5. Kode må ikke tillægge teksten mere, end teksten siger. Retspraksis,
   forfatningsretlig teori og statsretlig sædvane hører kun hjemme i modellen,
   når de er særskilt kildebelagt og tydeligt adskilt fra grundlovens ordlyd.
6. Én retlig sammenhæng skal have én kanonisk repræsentation. Hjælperegler
   afledes fra den; de må ikke skabe parallelle sandhedskilder.
7. Prøvninger beviser egenskaber ved den formaliserede model. De beviser ikke
   i sig selv, at en bestemt juridisk fortolkning er gældende ret.

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

## Målbillede for domænemodellen

### Eksempel: § 3

Forslaget om at samle magtfordelingen er rigtigt. En liste som
`magt([Kongen, Folketinget], Lovgivende)` er dog ikke tilstrækkelig præcis:
listen udtrykker medlemskab, men ikke ordene "i forening", og den tillader
rækkefølge, dubletter og vilkårligt antal deltagere.

Den foretrukne retning er en udtømmende, typet placering:

```runa
# Statsmagt = Lovgivende | Udøvende | Dømmende
# Statsorgan = Kongen | Folketinget | Domstolene
# Udøvelsesform = Hos(organ: Statsorgan) | IForening(
    organ: Statsorgan,
    medorgan: Statsorgan
)

| statsmagtens_placering(magt: Statsmagt) -> match magt {
    | Lovgivende -> IForening(
        organ = Kongen,
        medorgan = Folketinget
    )
    | Udøvende -> Hos(organ = Kongen)
    | Dømmende -> Hos(organ = Domstolene)
}
```

`Statsorgan` foretrækkes frem for `MagtHaver`: typen står i ental, og navnet
beskriver grundlovens institutionelle placering uden at foregribe, hvordan den
formelle magt udøves i nutidig statsretlig praksis. `IForening` bevarer det
afgørende forhold, som en flad liste ville tabe.

Hvis opslag som `udøver(organ, magt)` fortsat er nyttige, skal de afledes fra
`statsmagtens_placering`. De fire nuværende fakta og de to særskilte
`lovgivning_kræver`-fakta må ikke fortsætte som uafhængige sandhedskilder.

Kontrollerne for § 3 skal mindst fastslå:

- at hver `Statsmagt` har præcis én placering;
- at den lovgivende magt er placeret hos kongen og Folketinget i forening;
- at den udøvende og dømmende magt har de tekstnære placeringer;
- at eventuelle afledte opslagsregler er konsistente med den kanoniske regel;
- at modellen ikke indfører ordet "alene", som ikke står i § 3.

### Fælles modelleringsstandard

Hver bestemmelse gennemgås med følgende spørgsmål:

1. Hvilke retligt betydende aktører, genstande, hændelser og udfald findes i
   ordlyden?
2. Hvilke af dem er lukkede alternativer og bør være sumtyper?
3. Beskriver flere parametre i virkeligheden én sag, beslutning eller proces?
4. Hvilken regel er den kanoniske sandhedskilde?
5. Hvilke betingelser hører til reglen med `under`, og hvilke er egentlige
   navngivne `exception`-regler?
6. Er modaliteten bevaret: "skal", "kan", "må ikke" og "bør" må ikke gøres
   semantisk ens?
7. Kan en ugyldig tilstand gøres urepræsenterbar med en type i stedet for
   efterfølgende boolesk kontrol?
8. Er enhver bekvemmelighedsregel afledt og prøvet mod den kanoniske regel?

Navngivne konstruktørargumenter bruges ved alle ikke-trivielle domæneværdier.
Store universelle typer undgås; typer placeres fælles, når de faktisk deles på
tværs af kapitler, og ellers ved den bestemmelse, der ejer begrebet.

## Kilder og metadata

Det korte anker bevares:

```runa
--@label:grundlov_par3::meta:grundlov_par3_metadata--
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
- hver bestemmelse får en maskinlæsbar bestemmelsesbetegnelse, ikke kun en
  henvisning til hele loven;
- primærkilde og støttekilde får tydeligt forskellige roller;
- hentningsdato forbliver en typet dato;
- prøvningsfund får deres egen metadata og genbruger ikke ukritisk
  bestemmelsens kildemetadata;
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

## Prøvningsmodel

Alle ikke-trivielle fund klassificeres med en typet udsagnsstatus:

```runa
# Udsagnsstatus =
    TekstnærKontrol
  | Modelresultat
  | Fortolkningsspørgsmål
  | RetskildestøttetKonklusion
```

- `TekstnærKontrol` kontrollerer, at en udtrykkelig bestemmelse er korrekt
  repræsenteret.
- `Modelresultat` følger af de kodede regler, men gælder først og fremmest
  modellen.
- `Fortolkningsspørgsmål` viser en spænding eller en uafklaret grænse, som
  ordlyden ikke alene afgør.
- `RetskildestøttetKonklusion` kræver særskilte, angivne retskilder ud over den
  blotte modelkørsel.

Hvert offentliggjort fund skal vise:

1. den relevante ordlyd og dens kildelabel;
2. de regler, prøvningen faktisk kalder;
3. den konkrete kontrol eller det konkrete modeleksempel;
4. udsagnsstatus;
5. hvad resultatet viser;
6. hvad resultatet ikke afgør;
7. eventuelle supplerende retskilder.

`?` omtales som en kontrol eller invariant, ikke automatisk som et juridisk
bevis. "Paradoks" bruges kun, hvis modellen viser en formel modstrid, hvor de
relevante udsagn ikke samtidig kan opfyldes. Ellers bruges "spænding",
"asymmetri", "åbent fortolkningsspørgsmål" eller "modelgrænse".

### To fund, der skal genbehandles

**§§ 6 og 70, monarkens troskrav:** Den nuværende tekst kalder forholdet en
direkte selvmodsigelse og bruger en muslimsk tronfølger som bevis. Modellen kan
vise, at § 6 opstiller et troskrav til kongen, at den kodede § 70 er generelt
formuleret, og at arvefølgen ikke har fået indlagt samme prøve. Om § 70 retligt
begrænser tronfølgen eller kongens embede, kræver fortolkning. Fundet skal
derfor offentliggøres som `Fortolkningsspørgsmål`, indtil stærkere retskilder
underbygger en konklusion.

**§§ 43 og 73, skat og ekspropriation:** Modellen kan vise, at ordlyden ikke
selv opstiller en beregnelig grænse mellem skattepålæg og
ejendomsafståelse. Den kan ikke alene fastslå, at en skat på 100 procent er
forfatningsmæssig eller omgår § 73. Den påstand afhænger af
forfatningsretlig kvalifikation og praksis. Fundet skal beskrives som en
tekstlig modelgrænse og et fortolkningsspørgsmål.

## Filstruktur

Kapitelfilerne beholdes som den primære juridiske opdeling. Prøvningslaget
splittes efter ansvar, så scenarier, lokale kontroller og tværgående analyser
ikke længere ligger i én fil:

```text
examples/danish-constitution/
  grundlov-faelles.runa
  kapitel-01.runa ... kapitel-11.runa
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

## Gennemførelsesforløb

### Trin 0: Frys grundlaget

- Registrer den nuværende kildekontrolsum, de 91 labels og alle eksisterende
  kontroller som sammenligningsgrundlag.
- Kør metadataindekset og gem en maskinlæsbar oversigt over label, kodespænd og
  symboler.
- Registrer de 49 topologiske dækningshuller efter bestemmelse.
- Fastslå, at det historiske korpus er uden for ændringsområdet.

### Trin 1: Fælles ordforråd og kapitel I-III

- Omdøb `Institution` til `Statsorgan` og indfør den kanoniske model for § 3.
- Gennemgå monark, tronfølge, regentskab, regering, ministre, statsråd,
  samtykke og beslutning som sammenhængende domæner.
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

- Gennemgå værnepligt, kommunalt selvstyre, historiske privilegier,
  grundlovsændring og overgangsbestemmelser.
- Kontroller krydshenvisninger, frister, tællinger, modalitet og
  delegationsregler på tværs af alle kapitler.
- Klassificer hvert af de 49 kendte dækningshuller. Dæk kun huller med en
  meningsfuld egenskab; dokumenter resten som bevidste.

### Trin 5: Del prøvningslaget

- Flyt alle scenarieværdier til `.scenario.runa`.
- Del lokale overensstemmelseskontroller, procedurekontroller,
  rettighedskontroller og tværgående undersøgelser i hver sin `.audit.runa`.
- Giv hvert væsentligt fund typet metadata med udsagnsstatus og kontrollerede
  programhenvisninger.
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
- ingen synlig tekst forklarer, at siden er på dansk. Dansk er udgangspunktet.

Den eksisterende adresse `/research/danish-constitution-audit` beholdes for
stabile henvisninger, men den synlige side omdøbes til "Prøvning af
grundlovsmodellen":

- engelsk sidetitel, metabeskrivelse og sprognote omskrives til dansk;
- de tre fremhævelseskort fjernes;
- indledningen forklarer kort forskellen mellem ordlyd, model og fortolkning;
- hvert fund følger den faste struktur fra prøvningsmodellen;
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

## Kvalitetsporte

Arbejdet er først færdigt, når følgende holder samtidigt:

- alle 91 officielle kildetekstblokke har uændret kontrolsum;
- alle 91 labels er entydige og har præcis ét sammenhængende kodespænd;
- metadataindekset kan føre hvert retligt symbol tilbage til bestemmelsen og
  dens kilder;
- samtlige aktive `.runa`-filer består syntaks-, type- og navnekontrol;
- den samlede grundlovsprøvning består i fortolkeren;
- samme prøvning består som genereret Rust;
- § 3 har én udtømmende magtfordeling og ingen parallelle løse fakta;
- alle ikke-trivielle offentlige fund har udsagnsstatus, programhenvisninger og
  nødvendige retskilder;
- de 49 kendte dækningshuller er enten meningsfuldt dækket eller begrundet som
  bevidste;
- det aktive korpus og de synlige grundlovssider indeholder ikke undgåeligt engelsk
  eller blandingssprog;
- websiderne viser ingen forældede optællinger eller påstande om juridiske
  beviser, som maskinkørslen ikke kan bære;
- det historiske korpus er urørt.

## Afgrænsning

Arbejdet skal ikke:

- modernisere eller sprogligt rette grundlovens ordlyd;
- simulere hele dansk forfatningsret alene ud fra grundlovsteksten;
- indbygge politiske vurderinger i de kanoniske lovregler;
- maksimere kontroldækning med tautologier;
- samle hele grundloven i én stor domænetype;
- ændre offentlige adresser uden en kompatibel viderestilling;
- ændre Futuruna-sproget, medmindre en konkret, generel sprogbegrænsning
  forhindrer den idiomatiske model. En sådan begrænsning skal isoleres og
  vurderes som et selvstændigt sprogarbejde.

## Færdigdefinition

Grundlovsarbejdet er færdigt, når en læser kan gå fra den officielle ordlyd til
den kanoniske regel, fra reglen til dens betingelser og undtagelser og fra en
prøvning tilbage til alle dens kilder uden at skulle gætte, om et udsagn er
lovtekst, modelresultat eller juridisk fortolkning. Webudgaven skal afspejle
samme disciplin: først grundloven, derefter koden og til sidst den tydeligt
afgrænsede prøvning.
