# Personskatteloven i Futuruna

Dette er den ene webviste projektfil for arbejdet med dansk personskat, ikke
en lang forskningsartikel. Selve lovoversættelsen ligger i
`examples/danish-income-tax/` som Futuruna-regler, scenario-filer og
audit-filer.

## Intro: Et samlet sprog til lov og ret

Futuruna gør lovtekst til eksekverbare, indkapslede regler. Det giver en
regelstruktur, hvor lovens originale ordlyd kan stå lige over de regler, der
oversætter den, og hvor regelkaskader kan føre de afledte beløb videre gennem
hele beregningen.

Det samme sprog kan bruges til audit: ikke kun "hvad betaler denne person?",
men også "hvilke konfigurationer gør systemet hårdt, overraskende eller
uklart?"

## Personskatteloven (indkomstskat)

Målet er at formulere den samlede danske indkomstskattelovgivning i Futuruna,
så almindelige skatteforløb kan udregnes i samme sprog som de juridiske regler
er skrevet i. Personskatteloven er kernen, men en reel beregning kræver også
afhængigheder som arbejdsmarkedsbidrag, kommunal skat, kirkeskat,
Kildeskatteloven, Ligningsloven, aktieindkomst, kapitalindkomst,
ægtefælleregler, underskud og slutopgørelse.

## Struktur

Futuruna-filerne følger samme gentagne form:

- original dansk lovtekst i flerlinjeblok
- kun en kort note, hvis kilden kræver det
- faktiske regler med `|`, `under` og `exception`

## Aktuel status

Projektet er beregningsegnet for en væsentlig lønmodtagervej: AM-bidrag,
kommunal skat, kirkeskat, aktieindkomst, kapitalindkomst, personfradrag,
underskud, delår, skatteloft, indeholdelse, slutopgørelse, årlige
pensionsindbetalinger og dele af Ligningslovens fradragsregler.

Den almindelige lønmodtagervej er også udstillet som en samlet, typet
beregningsgrænse. Futuruna kan generere JSON-, TOML- eller XLSX-input direkte fra
`PersonskatInput`, validere det mod samme kontrakt og returnere både det fulde
skatteresultat og en valgfri årsopgørelse. Arbejdsbogen afledes fra den samme
nåbare domænegraf som beregningen. `@ calculate("Dansk personskat")` giver
beregningen dens menneskelige titel, som vises øverst på hovedarket og på de
relationelle ark. Hvert eksponeret felt kan samtidig have en
dansk etiket, et interviewspørgsmål, hjælp, enhed og typede kildehenvisninger,
mens den kanoniske feltsti forbliver en stabil maskinnøgle.

Regnearket er derfor også et maskinlæsbart udvekslingsformat. En AI kan læse den
samme kontrakt, interviewe borgeren med de menneskelige spørgsmål og udfylde de
kanoniske svar. AI'en skal ikke selv gætte skatteregler eller beregne skatten:
Futuruna validerer fakta og udfører den kildebundne beregning deterministisk.
Resultatet bevarer de juridiske mellemresultater, så AI'en bagefter kan forklare
hvilken regel, betingelse, undtagelse og kilde der førte til beløbet.

Pensionsgrenen viser samspillet i praksis. Borgeren eller AI-interviewet
oplyser de faktiske indbetalinger, ordningstyper, betalingsår og relevante
valg. Futuruna afleder årets fradrag efter Pensionsbeskatningslovens § 18,
deler loftet mellem flere ordninger og giver arbejdsgiverindbetalinger prioritet
efter arbejdsmarkedsbidrag. Resultatet føres til Personskattelovens § 3 og
genbruges i det ekstra pensionsfradrag efter Ligningslovens § 9 L. Et valg efter
Personskattelovens § 4 a kan placere et § 15 A-fradrag i positiv aktieindkomst;
reglerne trækker da præcis samme beløb ud af fradraget i personlig indkomst.
Ugyldige pensionsfakta bliver synlige i sporet, men giver intet fradrag.
Kilderne er
[Pensionsbeskatningsloven, LBK nr. 1243/2024](https://www.retsinformation.dk/eli/lta/2024/1243),
[Personskatteloven, LBK nr. 1284/2021](https://www.retsinformation.dk/eli/lta/2021/1284)
og
[Ligningsloven, LBK nr. 1500/2025](https://www.retsinformation.dk/eli/lta/2025/1500).

Afkast fra pensions- og forsikringsordninger efter
Pensionsbeskatningslovens § 53 A går nu gennem den samme kanoniske beregning.
Borgeren eller AI-interviewet vælger ikke selv en juridisk § 53 A-kategori eller
en undtagelse. Det oplyser det faktiske produkt, ejer eller berettigede,
kapitel 1-status, oprettelsesdato, senere daterede erhvervelser og om de skete
ved arv, eventuelt afkald på afsnit I,
institutionsfinansiering, skattemæssigt hjemsted og indbetalingernes faktiske
udenlandske skattebehandling. Futuruna afleder alle ni kategorier i § 53 A,
stk. 1, udelukkelsen efter § 53 B, forsikringsundtagelserne i stk. 4 og
statsstøttebegrænsningen i stk. 6. Det afleder også, om en ordning fra før den
18. februar 1992 senere er blevet omfattet, eller om direkte arv af en ældre
livsforsikring bevarer den historiske retsstilling. Resultatet viser hver
delkonklusion, mens ufuldstændige eller modstridende oplysninger fejler lukket.

Den daterede rettighedskæde afleder også afkastperioden ved en overdragelse.
Den tidligere ejer og erhververen kan derfor beregnes hver for sig frem til og
fra det præcise overdragelsestidspunkt, uden at borgeren selv angiver periodens
skattemæssige grænser. Ved én rettighedshaver afledes identiteten af kæden. Kun
flere samtidige berettigede kræver særskilte, validerede indeståender ved den
relevante afkastperiodes udgang.

For en historisk ordning oplyses et eventuelt overgangsvalg som daterede
kildedata: beslutning, modtagelse, modtager og valgt § 53 A- eller § 53 B-regime.
Futuruna afleder selv den ordinære frist før 2006, særvirkningen fra 1. januar
2004, lovens ikrafttræden den 22. december 2004, fristen ved senere fuld
skattepligt og den første valgmulighed ved senere
arv. Det første gyldige valg er bindende. Et senere modstridende forsøg bliver
synligt i forklaringssporet uden at omskrive det allerede valgte regime.
Kilderne er
[LOV nr. 1388 af 20. december 2004, § 5, nr. 2](https://www.retsinformation.dk/eli/lta/2004/1388),
[Den juridiske vejledning C.A.10.4.2.5](https://info.skat.dk/data.aspx?oid=2048459)
og
[SKM2023.171.SR](https://info.skat.dk/data.aspx?oid=2383693).

Hver ordning har desuden en stabil identifikation og en sammenhængende række af
årlige kildefakta. Borgeren eller AI-interviewet oplyser pensionsudbyderens
PAL-afkast eller de faktiske depotværdier, daterede ind- og udbetalinger,
begyndelsesstatus for skattepligt og eventuel direktørsikkerhed, ændringer i
årets løb og de berettigedes indeståender ved årets udgang. Futuruna afleder selv
den relevante skatteperiode, metodehistorikken, periodens betalingssummer, hver
berettigets andel og negativt afkast til fremførsel på samme ordning. Ved ind-
eller udtræden bruges depotværdien på ændringsdatoen; det samme gælder ved
etablering eller ophør af en direktørsikkerhed. Et helt år uden skattepligt
giver intet skattepligtigt afkast og etablerer ikke et nyt metodevalg.

Det bindende metodevalg håndhæves på tværs af årene. Udeladte mellemår,
metodeskift, modstridende hændelser og flere adskilte skatteperioder i samme år
fejler lukket uden en kapitalpost. En dokumenteret udbetaling til dækning af
afkastskat er kun skattefri, hvis den sker senest året efter optjeningsåret.
Et positivt resultat føres til Personskattelovens § 4, stk. 1, nr. 13, mens et
negativt resultat bevares på den identificerede ordning. Både lovresultater og
en eventuel ugyldighed bliver i forklaringssporet.

Variantvalg gør særlige
skatteforhold,
underskudsforhold, årsopgørelse og valgfri fradragsgrene eksplicitte; kun den
valgte grens felter skal udfyldes. Lister bliver til særskilte, nøglebundne
kildetabeller frem for et håndskrevet antal gentagne kolonner. Den kanoniske
graf modtager nu renteindtægter,
renteudgifter, identificerede pensionsindbetalinger efter
Pensionsbeskatningslovens § 18, identificerede ordninger og afkast efter
Pensionsbeskatningslovens § 53 A, Ligningslovens §§ 6/6 A-fradrag,
identificerede § 9 B-sager om
erhvervsmæssig kørsel og godtgørelse, § 9 C-befordringsfakta,
valgfri § 9 D-forhold, udlejning eller fremleje efter Ligningslovens § 15 Q,
driftsresultater fra bolig-, fritids- og lignende ejendomme efter
Personskattelovens § 4, stk. 1, nr. 6,
identificerede omkostninger efter Personskattelovens
§ 4, stk. 2, egne og en samlevende ægtefælles ejendomsafståelser,
KGL-kildefakta for EBL § 6 D-sælgerpantebreve samt ordinære eller særlige
ABL-forløb. Ejendomsafståelsernes skatteår, § 9 C's skatteår og
aftrapningsindkomst samt § 9 D-resultatet afledes af reglerne og er ikke
borgerfelter. For ejendomsdriften leverer borgeren ejendomstype, beliggenhed,
anvendelse og årets beløb. Futuruna afleder hjemlen efter
Ejendomsskattelovens § 3, herunder 2027-omnummereringen, og bevarer både
medregnede og gyldigt ekskluderede beløb i resultatet.
Kilderne er
[ændringen af Personskattelovens § 4, stk. 1, nr. 6, LOV nr. 679/2023](https://www.retsinformation.dk/eli/lta/2023/679),
[Ejendomsskatteloven, LOV nr. 678/2023](https://www.retsinformation.dk/eli/lta/2023/678)
og
[ændringslov nr. 615/2026 med virkning fra 2027](https://www.retsinformation.dk/eli/lta/2026/615).

Ejendomsavancebeskatningslovens §§ 5 og 5 A udleder nu
anskaffelsessummens årlige tillæg, forbedringsudgifter, nedsættelser og
eventuel indeksering fra de faktiske datoer og hændelser. Ejerandel og
delafståelse er adskilte fakta. Historiske mælkekvoter udleder
anskaffelsestillæg, afståelsesnedsættelse, udløb til 0 kr.,
toldningsundtagelsen og de korrekte § 5 A-år. Ved delafståelse bruger de
almindelige § 5-beløb hele ejendommens anskaffelsessum, mens
mælkekvotevederlag bruger anskaffelsessummen for den del, der ikke udgør
boligdelen. § 5, stk. 6 er obligatorisk, når betingelserne er opfyldt, og
omfatter både de to nummererede 1993-indgangsværdier og en vurdering efter den
dagældende vurderingslovs § 4 B. Reglen udleder den højeste af
tillægsparcelværdien og den tekniske værdi, fratrækker de relevante
bygningsbeløb og fordeler resten efter ejerandel og den afståede jords
anskaffelsessum; samme rå beløb nedsætter restejendommen. § 8 udleder
parcelhusfritagelsen, og § 9 fordeler en blandet ejendoms bolig- og
erhvervsdel, så skattefri boligfortjeneste og skattepligtig
erhvervsfortjeneste ikke blandes sammen. Genanbringelse efter §§ 6 A, 6 C og
10 er nu kildeafledt fra den oprindelige erhvervsfortjeneste, investeringen,
anvendelsen, fristerne, ejerskabet, placeringen og eventuelle
genopførelsesfakta. Når den nye ejendom senere bliver solgt, beskatter § 8,
stk. 5 den gamle erhvervsfortjeneste særskilt fra boligejendommens egen
skattefri fortjeneste. § 9, stk. 4 kan tilsvarende beskatte den gamle
fortjeneste særskilt og fjerne anskaffelsessumsnedslaget, hvis en forøget
boligandel betyder, at erhvervsdelen ikke kan bære hele fortjenesten. § 11,
stk. 2 behandler nu også en tidligere genanbragt erhvervsfortjeneste ved
ekspropriation af den nye ejendom. Den nye ejendoms egen fortjeneste forbliver
skattefri efter stk. 1, mens den regulerede gamle fortjeneste beskattes
særskilt, og det tilsvarende anskaffelsessumsnedslag bortfalder. Borgeren kan
vælge en ny genanbringelse efter §§ 6 A eller 6 C gennem typede kildefakta;
reglerne afleder selv den straks beskattede del og det nye aktive nedslag. § 6
modregner derefter egne og
fremførte tab og kan overføre en samlevende ægtefælles overskydende tab, før
Personskattelovens § 4, stk. 1, nr. 14-post dannes. Overførslen kan højst bruge
modtagerens fortjeneste efter dennes egne tab; resten bevares til fremførsel.
Mælkekvoter og § 5, stk. 6 er kombinerbare, kildeafledte grene med synlige
delresultater. De resterende ikke færdigmodellerede særforhold er fortsat
synlige og fail-closed i stedet for at blive beskattet af en smallere
standardregel. Værdipapirer med brugsret til en
bolig efter § 8, stk. 4, er nu koblet til Aktieavancebeskatningslovens § 15.
Futuruna udleder fritagelsen af udstederens direkte ejerskab af
flerlejlighedsejendommen, boligbrug i den kvalificerende ejerperiode, et
eventuelt bestemt grundareal og likvidationsåret. Hvis betingelserne ikke er
opfyldt, fortsætter gevinst eller tab gennem de almindelige ABL-regler; grenen
giver derfor ikke længere et fail-closed nulresultat. Kilderne er
[Ejendomsavancebeskatningsloven, LBK nr. 132/2019](https://www.retsinformation.dk/eli/lta/2019/132),
[lov nr. 308/2006](https://www.retsinformation.dk/eli/lta/2006/308),
[Den juridiske vejledning C.H.2.1.9.10](https://info.skat.dk/data.aspx?oid=1948630),
[C.H.2.1.9.11](https://info.skat.dk/data.aspx?oid=1948631),
[Den juridiske vejledning C.H.2.1.11.2](https://info.skat.dk/data.aspx?oid=1948642),
[C.H.2.1.11.4](https://info.skat.dk/data.aspx?oid=1948713),
[C.H.2.1.11.5](https://info.skat.dk/data.aspx?oid=1948714),
[C.H.2.1.11.6](https://info.skat.dk/data.aspx?oid=1948715),
[C.H.2.1.17.5](https://info.skat.dk/data.aspx?oid=1948739)
og
[Aktieavancebeskatningsloven, LBK nr. 1098/2025](https://www.retsinformation.dk/eli/lta/2025/1098),
med de relevante senere ændringer bevaret ved de enkelte lovblokke.

Arbejdsgiverbetalt befordring er et typet kildefaktum. Et frikort til offentlig
transport bliver personlig B-indkomst uden AM-bidrag, mens modposten ved fri
bil bliver AM-bidragspligtig personlig indkomst. Afgrænsningen følger
[Den juridiske vejledning C.A.5.14.4.4](https://info.skat.dk/data.aspx?oid=1976849)
og [SKM2025.537.BR](https://info.skat.dk/data.aspx?oid=2459313).
§ 4, stk. 3 omklassificerer både poster og omkostninger ved næring uden at gøre
dem til AM-bidragspligtig løn. De resterende § 3-, § 4- og § 4 a-kildegrene
kobles fortsat på det samme input, så den samlede borgerarbejdsbog udbygges fra
reglerne frem for at blive håndskrevet ved siden af dem.

Den stærkeste brugerflade er ikke nødvendigvis manuel udfyldning af et stort
regneark. En AI kan interviewe borgeren, bygge det samme typede input og bruge
Futurunas regler til den deterministiske beregning og den efterfølgende
forklaring. Arbejdsbogen er dermed også et stabilt udvekslingsformat mellem
interviewet og beregningsmotoren, ikke kun en formular til manuel indtastning.
Kolonnestierne er stabile maskinnøgler, mens generisk, typet feltmetadata nu kan
give hver sti en menneskelig etiket, et interviewspørgsmål, hjælp, enhed og
kildespor. Personskat-kontrakten bruger dette for skatteår, kommune, bruttoløn,
befordring, pensionsindbetalinger, pensionsvalg, aldersstatus, kirkeskat,
renter, årsopgørelse og centrale
ejendomsdriftsfakta samt
ejendomsavancefakta som anskaffelsesår, afståelsesår, kontante summer,
anskaffelsesgrundlag, indekseringsvalg og ejendomstype. Genanbringelsens
lovgrundlag, oprindelige afståelsesår og erhvervsfortjeneste,
geninvesteringsår, anskaffelsesgrundlag, anvendelse, placering, begæring,
ejerskab og overgangsforhold har også egne menneskelige etiketter og
interviewspørgsmål for både personen og ægtefællen. Det samme gælder
ordinære aktiebeholdninger og boligretsfakta efter ABL § 15: værdipapirets
tilknytning til lejligheden, udstedervariant, registreret selskabsform eller
foreningstype, dansk skattemæssigt hjemsted, SEL § 3- og
Fondsbeskatningslovsstatus, værdipapirets ABL-status, boligbrug, grundforhold,
afståelsesform og likvidationsår har hver sin menneskelige etiket og sit
interviewspørgsmål. En AI kan læse metadataene og indsamle fakta, men
formuleringerne ændrer ikke felternes gyldighed eller skattereglernes
deterministiske resultat. Futuruna afleder selv, om udstederen er et
selvstændigt skattesubjekt, og om værdipapiret er omfattet af ABL. En
transparent udsteder eller et værdipapir uden for ABL gør hændelsen ugyldig;
et ellers gyldigt ABL-forløb, der alene ikke opfylder § 15's
fritagelsesbetingelser, fortsætter gennem de almindelige ABL-regler.
Beregningsgrænsen har samtidig titlen `@ calculate("Dansk personskat")`; den
tekst navngiver hele beregningen, mens feltmetadata navngiver de enkelte
interviewoplysninger. Kontrakten har nu 942 eksplicitte feltmetadata-poster,
heraf 258 for § 53 A-ordninger. De historiske overgangsvalg ligger i de
fjorten relationelle § 53 A-ark frem for som gentagne kolonner på ordningen.
Alle 98 nåbare § 15 A-stier for en virksomhedsafståelse har en dansk etiket og
et interviewspørgsmål, herunder de tre regnskabsperioders datoer, direkte og
indirekte ejerandele samt selskabernes underliggende indtægter og aktiver.
Otteogtyve beskriver den nøglebundne liste over erhvervsmæssig kørsel efter
Ligningslovens § 9 B med sagsidentifikation, godtgørende arbejdsgiver, køretøj,
kilometer, kronologisk rækkefølge, 60-dages-forhold, udgifter og
godtgørelsesforhold. Reglerne udleder arbejdsgivernes hidtidige kilometer fra
de ordnede sager, så borgeren eller AI-interviewet ikke skal levere et beregnet
mellemresultat. Seks
beskriver ejendomsdriftens variant, ejendomstype, beliggenhed,
erhvervsmæssig udlejning, særlige betingelser og årets underskud eller
overskud. Herunder er også otte for ABL § 15's udsteder- og
værdipapirklassifikation og alle nye
ejerandels-, delafståelses-, ikke-boligdelens
anskaffelsessums-, mælkekvote- og § 5, stk. 6-felter for personen og
ægtefællen. EBL § 6 D's valg om at fordele en ejendomsfortjeneste via et
sælgerpantebrev har samme menneskelige lag: pantebrevets vilkår, parternes
faktiske brug, meddelelsen til Skatteforvaltningen og hvert efterfølgende års
hændelser har egne etiketter og spørgsmål. KGL-sporet navngiver desuden
pantebrevslisten, identiteten, skatteyderens kildefakta, § 14-grundlaget, den
berørte pantebrevstranche og senere afståelser eller indfrielser med år, art,
hovedstol og faktisk provenu. Ved ejerskifte får modtagerens nye tranche også
sin egen menneskelige etiket og sit eget interviewspørgsmål, så en AI kan
forbinde senere betalinger med det korrekte skattemæssige grundlag.
Det navngiver også personen, som beregningen vedrører, pantebrevets oprindelige
skatteyder og de fakta, der fordeler pantebrevet ved ægtefællesuccession eller
dødsfald.
66 af posterne dækker § 11, stk. 2
for personen og ægtefællen: valget om ny genanbringelse, anvendelse,
selskabsforhold, investering, udenlandsforhold, begæringsdatoer, ejerskab og
hjemmel efter §§ 6 A eller 6 C. De 18 felter for udlejning efter
Ligningslovens § 15 Q har ligeledes menneskelige etiketter og spørgsmål for
boligrolle, udlejningsform, indberetning, metode, beløb og samordning med
§ 15 P. Indkomstårets længde afledes af skatteåret og er ikke et borgerfelt.
AI'en kan dermed indsamle fakta;
Futuruna afleder selv, om betingelserne er opfyldt, og hvilke beløb der skal
medregnes i hvert indkomstår. En verificeret XLSX/JSON-sag fører en afståelse
i 2025, sælgerpantebrevets tiårige afdragsplan, KGL-kildefakta og
årsforholdene frem til 300.000 kr. i medregnet ejendomsavance og 75.000 kr. i
kursgevinst i 2026, i alt 375.000 kr. i kapitalindkomst. En anden verificeret sag
rekonstruerer en tidligere § 6 A-genanbringelse ved en § 11-afståelse. Begge
inputformater lader det gamle anskaffelsessumsnedslag på 200.000 kr. bortfalde
og medregner den gamle fortjeneste på 200.000 kr. præcis én gang.
Felter,
der endnu mangler præcis metadata, vises med en læsbar afledning af den stabile
sti og beholder den kanoniske sti i kolonnens note; det er et fallback, ikke en
erstatning for det juridisk præcise interviewspørgsmål.

Sælgerpantebrevets kursgevinst behandles særskilt fra EBL § 6 D-fordelingen.
Kontantværdien er pantebrevets anskaffelsessum, mens hovedstolen frigives
forholdsmæssigt ved hvert afdrag. I Skattestyrelsens eksempel har pantebrevet en
hovedstol på 3.750.000 kr. og en kontantværdi på 3.000.000 kr. Et årligt afdrag
på 375.000 kr. frigiver derfor 300.000 kr. af anskaffelsessummen og giver en
kursgevinst på 75.000 kr. Ved hel eller delvis afståelse eller ekstraordinær
indfrielse bruger EBL den berørte restgæld til at fremrykke ejendomsavancen,
mens KGL sammenholder den frigivne anskaffelsessum med det faktiske provenu.
Futuruna holder de to regelkaskader og deres delresultater adskilt og samler
først de gyldige poster i Personskattelovens kapitalindkomst. En dansk
ægtefælle, et dødsbo eller en efterlevende ægtefælle kan overtage pantebrevets
skattemæssige position, hvor Kildeskatteloven eller Dødsboskatteloven foreskriver
succession. Hvis en udlodning i stedet udløser realisation, beskattes resultatet
i boet, og ægtefællen fortsætter fra boopgørelsesværdien. Personskat medregner
kun personens egne realisationer. Delvise ejerskifter opdeler hovedstol og
anskaffelsessum i stabile, entydigt identificerede trancher; en fordeling, der
ikke kan opgøres præcist i hele kroner, fejler lukket. Den direkte KGL-model kan
derefter føre betalinger for hver ejer særskilt. EBL-broen fejler kun lukket,
når en delvis succession efterlader senere betalinger uden en entydig
ejerfordeling. Ikke-understøttede modtagere afvises tilsvarende. Kilderne er
[Kursgevinstloven, LBK nr. 1176/2025](https://www.retsinformation.dk/eli/lta/2025/1176),
[Kildeskatteloven, LBK nr. 460/2024](https://www.retsinformation.dk/eli/lta/2024/460),
[Dødsboskatteloven, LBK nr. 426/2019](https://www.retsinformation.dk/eli/lta/2019/426)
og
[Den juridiske vejledning C.H.2.1.11.9](https://info.skat.dk/data.aspx?oid=2292757).

Kursgevinstloven § 32 er nu formuleret som en selvstændig årsopgørelse. Den
fordeler kontrakttab mellem egne gevinster i året, tidligere års skattepligtige
nettogevinster, en samlevende ægtefælles kontraktgevinster og kvalificerede
aktiegevinster. Det typede grundlag omfatter ABL § 12-aktier og § 25-rettigheder,
§ 20, stk. 2- og § 21-beviser samt §§ 19 B, 19 C og 22. Omsættelige
investeringsbeviser anses for optaget på et reguleret marked efter ABL § 3;
andre unoterede investeringsselskabsaktier og ikke-kvalificerede klasser
udelades. § 19 D's oplysningsbetingelse anvendes før nettogevinsten når § 32.
Aktieavancebeskatningslovens § 13 A-tab bruges først hos personen og derefter hos
en samlevende ægtefælle. Først derefter kan kontrakttab bruge den resterende
aktiegevinst. Et helt eller delvist aktiemodregningsvalg er eksplicit, og kun en
identitets- og årsbundet rest kan fremføres. Tab på fast-ejendomsaftaler kan kun møde gevinster
på sådanne aftaler; resten bliver til nedsat afståelsessum for sælgeren eller
forhøjet anskaffelsessum for køberen. Tab fra før 2010 bevarer samtidig deres
ældre, snævrere anvendelse og kan ikke senere modregnes i aktiegevinster.
MTF-kontrakter og gevinster på MTF-aktier indgår først i § 32-modregningen fra
1. januar 2024. Seksogtyve scenarier kører ens i interpreter og kompileret kode.
Personskattelovens § 4-bro bruger nu det samlede § 32-årsresultat og bevarer hver
kontraktposts klassifikation som kapital- eller personlig indkomst. Blandede
sager bliver opdelt, når fordelingen er entydig; ellers bliver den tværgående
årsfordeling stående som et udtrykkeligt uallokeret beløb, der ikke kan medregnes
som kapitalindkomst. Ni fokuserede § 4-scenarier kører ens i begge backends.
Den rå enkeltkontraktbro er fortsat tilgængelig og mærket som før
årsopgørelsen. Kilderne er
[Kursgevinstloven, LBK nr. 1176/2025](https://www.retsinformation.dk/eli/lta/2025/1176),
[Aktieavancebeskatningsloven, LBK nr. 1098/2025](https://www.retsinformation.dk/eli/lta/2025/1098),
[LOV nr. 1563/2023, § 4 og § 8](https://www.retsinformation.dk/eli/lta/2023/1563)
og [Skattestyrelsens juridiske vejledning C.B.1.8.4.2](https://info.skat.dk/data.aspx?oid=1946050).

Personskatteloven § 13 a er nu også en lukket, typet regelkaskade. Ved
gældssanering eller akkord kan modellen kun modtage fremførte tab gennem de
faktiske årsresultater fra Aktieavancebeskatningsloven, Kursgevinstloven eller
Ejendomsavancebeskatningsloven; et løst `skyldner_tab_kroner` findes ikke
længere. Ejendomsavancebeskatningsloven § 6 beregner selv, om et ejendomstab
afskæres efter §§ 8 eller 9, begrænses af § 4, stk. 3, modregnes i egen eller en
samlevende ægtefælles fortjeneste eller fremføres. Et fokuseret eksempel samler
10.000 kr. fra hver af de tre afhængige love og afviser samtidig et tab fra et
senere indkomstår. Metadataindekset gør også en henvisningsforskydning synlig:
den ældre Personskattelovtekst nævner Kursgevinstlovens § 32, stk. 3, mens den
gældende almindelige fremførselsregel står i stk. 4. Kilderne er
[Personskatteloven, LBK nr. 1284/2021](https://www.retsinformation.dk/eli/lta/2021/1284),
[Kursgevinstloven, LBK nr. 1176/2025](https://www.retsinformation.dk/eli/lta/2025/1176)
og [Ejendomsavancebeskatningsloven, LBK nr. 132/2019](https://www.retsinformation.dk/eli/lta/2019/132).

Personskatteloven § 3, stk. 2, nr. 2 er nu også en egentlig regelkaskade.
Ligningslovens regler om salgs- og repræsentationsudgifter, forskning,
råstofefterforskning, skov og andre plantninger, friplejeboliger,
ansættelsesudgifter, ejendomsskatter, medarbejderfonde og behandling eller
rygeafvænning beregner hver deres typede resultat. Kildeskatteloven § 25 A
beregner tilsvarende overførsel til en medarbejdende ægtefælle med
overskudskorrektion, 50 pct.-grænse, reguleret loft og arbejdsindsats som
særskilte led. Først derefter afgør Personskatteloven, om beløbet vedrører
selvstændig erhvervsvirksomhed. Et løst beløb mærket som nr. 2 kan derfor ikke
længere skabe et fradrag. Forskningsreglen skelner desuden eksplicit mellem et
kendt påbegyndelsesår og en virksomhed, der endnu ikke er påbegyndt, så en
ukendt start ikke kan blive til et straksfradrag. De 31 fokusinvarianter kører
ens i interpreter og kompileret kode. Kilderne er
[Ligningsloven, LBK nr. 1500/2025](https://www.retsinformation.dk/eli/lta/2025/1500)
og [Kildeskatteloven, LBK nr. 460/2024](https://www.retsinformation.dk/eli/lta/2024/460).

Nr. 3-11 fortsætter nu gennem én lukket resultattype i stedet for at blive
omdannet til løse poster med en kategori og et beløb. Hver gren kan kun bære
det typede resultat fra den lovregel, der har beregnet den. Den samme kaskade
fører både fradragene og de positive indtægtsføringer fra nr. 4 og nr. 10 ind i
§ 3-resultatet, så de ikke skal tilføjes manuelt som parallelle poster. Et
fokusscenarie dækker alle ni grene i både interpreter og kompileret kode og
viser samtidig, at ni rå beløb mærket som nr. 3-11 samlet giver 0 kr. i fradrag.

Aktieavancebeskatningslovens § 17 er nu en selvstændig, typet regelkaskade for
aktienæring. Den skelner mellem selskaber og personer, aktienæringsstatus,
anskaffelse som led i næringsvejen og instrumentets art. Stk. 2 afskærer kun
tab på koncerninterne konvertible obligationer og tegningsretter, stk. 3
medtager alle minimumsbeskattede investeringsbeviser hos den næringsdrivende,
og stk. 4's undtagelser har udtrykkelig forrang. Resultatet forbinder den samme
sag med § 23's opgørelsesmetode, Kursgevinstloven § 32's kontraktrelation og
Personskatteloven § 4's personlige indkomstklassifikation for personer.
Femten scenarier kører ens i interpreter og kompileret kode.

Aktieavancebeskatningslovens §§ 19 B-19 C og §§ 21-22 afledes nu fra de
faktiske investeringsforhold. Den gennemsnitlige aktivmasse bygges af direkte
aktiver og de underliggende aktiver efter Kursgevinstlovens §§ 29-33. Ved en
ejerandel på præcis 25 pct. eller mere gennemlyses den ejede enhed efter ejede
og samlede kapitalenheder i stedet for en oplyst procent. Meddelelse,
nyoprettelsesfrist, årsoplysninger og anbringelsesgrænse afgør derefter den
effektive status. § 23, Personskattelovens §§ 4 og 4 a og Kursgevinstlovens §
32 genbruger samme integritetskontrollerede resultat. En bruger eller et
genereret regneark indtaster derfor aktiver, ejerposter og datoer, ikke om
enheden »er § 19 B« eller »er § 22«. Fyrre fokusscenarier kører ens i
interpreter og kompileret kode.

Aktieavancebeskatningslovens §§ 6-7 er nu også en kildebundet regelgrænse.
Skattepligt efter Selskabsskatteloven, Fondsbeskatningsloven,
Kildeskatteloven eller Dødsboskatteloven afleder ét integritetskontrolleret §
6/§ 7-resultat, som § 17, § 23 og § 9 genbruger. Resultatfelterne kan ikke
forfalskes uafhængigt af grundlaget, og livsforsikringsselskabets nødvendige
særstatus bevares til § 23, stk. 6-7. Elleve scenarier kører ens i interpreter
og kompileret kode.

Aktieavancebeskatningslovens § 5 A beregner nu selv den tabsreduktion, der skal
ske før de almindelige tabsregler. Skattefrie udbytter, forøget
dobbeltbeskatningslempelse, endnu uudnyttede præferenceudbytter og kvalificerede
koncernbeløb bevares som særskilte, typede led. Reduktionen kan ikke overstige
afståelsestabet. § 22, stk. 6-undtagelsen, virkningsgrænsen den 24. november
2010 og særreglen for tidligere statusskifter er udtrykkelige udfald. § 9
genberegner resultatet fra dets input og afviser forfalskede resultatfelter.
Ved lagerbeskatning indtastes årets afståelser som identificerede rækker med
skattemæssig værdi og afståelsessum. En § 5 A-tabsbehandling skal matche hver
tabsrække præcis én gang, mens gevinst- og LL § 16 B-rækker ikke kræver den.
Det giver et naturligt relateret ark i den genererede arbejdsbog og afleder
årets afståelsessummer fra de samme data. Tyve scenarier kører ens i
interpreter og kompileret kode.

Aktieavancebeskatningslovens § 9 er tilsvarende en typet årsopgørelse for
selskabers skattepligtige porteføljeaktier. Den bruger § 23's beregnede
realisations- eller lagerprincip, anvender §§ 8 og 10 som udtrykkelige
udelukkelser og afskærer koncerninterne konvertible afståelsestab efter stk. 7.
Direkte lagertab og de to forskellige realisationstabsbeholdninger holdes adskilt på
tværs af år. Et principskift kan kun udvide et fremført tabs anvendelse, når
år, aktiv og den aktuelle post faktisk hænger sammen. § 23, stk. 6-valget skal
desuden være ens for alle kvalificerede poster. Atten scenarier kører ens i
begge backends. Hver post anvender det validerede § 5 A-resultat før stk. 2-7
og bevarer både bruttotab og tabsreduktion i auditsporet. Årsopgørelsen
genberegner desuden hver post fra dens input, før posterne summeres.
Metadataindekset viser
fortsat, at rækkefølgen mellem to samtidige tabsbeholdninger er et
fortolkningsvalg.

Personaktievejen er nu også eksekverbar fra anskaffelse og gennemsnitlig
anskaffelsessum til afståelse, noterede og unoterede tab,
ægtefælleoverførsel og den endelige aktieindkomstpost efter Personskatteloven
§ 4 a. Den håndterer homogene beholdninger både med og uden pålydende værdi.
En særskilt § 25-vej beregner aktie- og tegningsretter efter FIFO og aktie for
aktie-metoden, herunder 0 kr. i anskaffelsessum ved aktionærtildeling,
§ 30's afståelsesbegreb ved bortfald, MTF-overgangen fra 2024 og § 14's
oplysningsbetingelse. Et særskilt §§ 23-27-modul gør valget mellem
realisations- og lagerprincip, den eksakte syvårsperiode, årlige
lageropgørelser, principskift, den hidtidige MTF-regel, adskilte § 7
N-beholdninger og § 27's anskaffelsessumtillæg eksekverbare. § 33 A beregner
desuden skattemæssige statusskifter som afståelse og genanskaffelse til
handelsværdi, sender udfaldet til den udgående status' almindelige regel og
holder skattefri omstruktureringer samt § 33-undtagelsen synlige. § 24, stk. 3
bruger dette typede resultat frem for et løst ja/nej-flag. §§ 37-39 gør nu også
tilflytningsværdi, fraflytterskattens 100.000 kr.- og syvårsgrænser, døds- og
§ 44-undtagelser, gevinst/tab-netting, tegningsretsvalg samt henstandens
indberetnings- og sikkerhedskrav eksekverbare. Videreflytning mellem bistands- og
ikke-bistandslande og en for sen beholdningsoversigt er særskilte udfald.
§§ 39 A-40 fortsætter henstanden som en typet flerperiodetilstand med
beholdningspartier, FIFO-afståelser, en samlet henstandssaldo, reserverede
fordringer og betalinger. Gevinst, tab, udbytte, andre dispositioner, lån og
undtagelser, død, årsoplysninger, dokumentation, endeligt bortfald,
tilbageflytning og betalt skat er eksekverbare. Fordringerne bærer indkomstår,
så samme års ubetalte fordringer genfordeles efter den officielle rækkefølge,
mens tidligere års og allerede betalte beløb bevares. De 41 nye scenarier
omfatter både disse flerhændelsesforløb og et bevis for, at flere fordringer
hverken kan reservere eller opkræve mere end henstandssaldoen. Satsbaserede
hændelser er aktuelt afgrænset til de
kildebundne parameterpakker for 2024-2026; andre år afvises før skatteopslaget.
§§ 35 G-35 K modellerer særskilt medarbejderejeordningen fra 1. januar 2026.
Valg, negativ anskaffelsessum, overdragerskattesaldo, FIFO-afståelser,
8 pct.-udbyttegrænsen, skattefradrag, betalinger, tvangsafståelser,
årsoplysninger, sikkerhed og hjemstedsflytninger er typede led i samme
vedvarende forløb. De 17 scenarier omfatter både virkningsgrænsen og et kædet
saldo-, udbytte-, salgs- og betalingsforløb i begge backends.
Resultaterne går videre gennem den samme
ABL/Personskattelov-bro. De fokuserede scenarier dækker nu mere end 200 udfald i
Aktieavancebeskatningsloven § 5 A, §§ 6-7, § 9, §§ 12-15, § 17, §§ 23-27, § 33 A,
§§ 35 G-35 K og §§ 37-40. Kilderne er
[Aktieavancebeskatningsloven, LBK nr. 1098/2025](https://www.retsinformation.dk/eli/lta/2025/1098),
[Selskabsskatteloven, LBK nr. 279/2025](https://www.retsinformation.dk/eli/lta/2025/279),
[Den juridiske vejledning C.H.2.1.15.6 om selvstændige skattesubjekter](https://info.skat.dk/data.aspx?oid=1948728),
[den oprindelige § 5 A-ændring og overgang, LOV nr. 254/2011](https://www.retsinformation.dk/eli/lta/2011/254),
[medarbejderejeændringen, LOV nr. 1755/2025](https://www.retsinformation.dk/eli/lta/2025/1755),
[vejledningen om § 5 A-tabsreduktionen](https://info.skat.dk/data.aspx?oid=1950044),
[vejledningen om § 9-porteføljeaktier](https://info.skat.dk/data.aspx?oid=1946340),
[vejledningen om tilflytning](https://info.skat.dk/data.aspx?oid=1946389),
[vejledningen om fraflytterskattens personkreds](https://info.skat.dk/data.aspx?oid=1946393),
[vejledningen om gevinst og tab ved fraflytning](https://info.skat.dk/data.aspx?oid=1946398),
[vejledningen om henstand](https://info.skat.dk/data.aspx?oid=1946400),
[betalingen på henstandssaldoen](https://info.skat.dk/data.aspx?oid=1946405),
[rækkefølgen for nedskrivning](https://info.skat.dk/data.aspx?oid=1946406)
og [tilbageflytning](https://info.skat.dk/data.aspx?oid=1946418).
Selskabslovens tilladte kombination af aktier med og uden pålydende værdi
bevares som særskilte domæneværdier på et dokumenteret fælles
kapitalandelsgrundlag, så anskaffelsessummen kan fordeles på tværs af begge
former.

Det er endnu ikke en fuld implementering af hele Personskatteloven. Det næste
vigtige arbejde er at gøre de resterende kildepostur- og kategoriregler til
kildebundne beløbsregler og udfylde de afhængigheder, der endnu mangler et
beregnet resultat.

Nyere afhængighedsdækning omfatter Ligningsloven § 9 C og § 9 D, herunder
befordringsfradrag, yderkommuner, lavindkomsttillæg, broer, SU-yderområde og
§ 9 D's særregler for varigt nedsat funktionsevne eller kronisk sygdom, samt
Kildeskatteloven § 62 A's frist for udbetaling efter ændret årsopgørelse og
Kommuneskatteloven § 5, stk. 3's delårsregel gennem Personskatteloven § 14's
tilsvarende beregningsform. Arbejdsmarkedsbidragsloven § 2, stk. 2 har nu
også typede naturalia-kategorier, så f.eks. fri bil kun føres ind i
AM-grundlaget, når den både er en nævnt naturalia-art og et stk. 1-vederlag.
§ 3 udstiller samtidig hver af lovens fem udelukkelser som et navngivet beløb,
så et beregnet AM-grundlag kan auditeres tilbage til den konkrete nummerpost.
Kommuneskatteloven § 16 a er nu også kildebundet og eksekverbar: modellen
fordeler selvbudgetteringsordningens nationale korrektionsbeløb mellem
kommuner med positive efterreguleringsbeløb og fratrækker kommunens andel i
§ 16-opgørelsen.
Ligningsloven § 8 M og Personskatteloven § 3, stk. 2, nr. 6 er tilsvarende
forbundet, så AM-bidrag og obligatoriske udenlandske sociale bidrag kun føres
til personligt indkomstfradrag, når den relevante skattepligt, social sikring
og eventuelle arbejdsgiveraftale opfylder lovens betingelser. Den kanoniske
`beregn_personskat`-regel modtager nu de underliggende borgerfakta, afleder selv
skatteåret og AM-bidraget af lønnen og bevarer begge lovresultater i svaret.
Kun de yderligere udenlandske bidrag føres ind i lønmodtagermodellen, så et
AM-bidrag, som allerede er indregnet dér, ikke fradrages to gange.
Personskatteloven § 3, stk. 2, nr. 7 er nu også koblet til de beregnede
henlæggelser efter Virksomhedsskatteloven §§ 22 b og 22 d, herunder procent- og
beløbslofter, udligningsskat, bundet konto og rettidigt indskud.
Personskatteloven § 3, stk. 2, nr. 8 og 9 modtager nu tilsvarende typede
resultater fra Ligningsloven §§ 9 B og 8 O. § 9 B-reglerne afgør bl.a.
60-dages-perioder, Skatterådets kilometersatser, skattefri eller personlig
godtgørelse, § 9 C-henvisning og den kundeopsøgende undtagelse for flere
arbejdsgivere. Den kanoniske lønmodtagerberegning modtager de faktiske kilometer,
udgifter og godtgørelsesforhold som nul eller flere identificerede sager. Hver
sag bevarer den godtgørende arbejdsgiver og dennes hidtidige kilometer, så
20.000-kilometergrænsen ikke blandes sammen mellem arbejdsgivere. Skatteår,
personrolle og den lovbestemte fradragsmetode afledes fortsat af reglerne. En
skattepligtig godtgørelse behandles som løn med AM-bidrag, mens et direkte
fradrag føres særskilt gennem Personskattelovens § 3, stk. 2, nr. 8. Begge
mellemresultater bevares pr. sag og summeres kun, når hele årslisten er gyldig.
Et verificeret XLSX/JSON-eksempel med to arbejdsgivere giver delresultaterne
3.940/560 kr. og 3.110/390 kr. og dermed samlet 7.050 kr. skattefri
godtgørelse og 950 kr. AM-bidragspligtig løn.
§ 8 O-reglerne skelner mellem ydelseskredsen før 2026 og den
udvidede kreds fra 2026, begrænser fradraget til tidligere beskattede beløb og
afskærer dobbeltfradrag. Kilderne er [Ligningsloven, LBK nr. 1500/2025](https://www.retsinformation.dk/eli/lta/2025/1500),
[LOV nr. 198/2025](https://www.retsinformation.dk/eli/lta/2025/198) og
[Skatterådets BEK nr. 1333/2025](https://www.retsinformation.dk/eli/lta/2025/1333).
Godtgørelsens løn- og AM-behandling følger desuden
[Den juridiske vejledning C.A.4.3.3.3.2](https://info.skat.dk/data.aspx?oid=2061750),
og flerarbejdsgiverfradraget følger
[C.A.4.3.3.3.3.1](https://info.skat.dk/data.aspx?oid=2061752).

Personskatteloven § 3, stk. 2, nr. 3 modtager nu også beregnede resultater fra
Pensionsbeskatningsloven §§ 18 og 52. Reglerne dækker bl.a. det regulerede
ratepensionsloft, arbejdsgiverindbetalinger, tiårsfordeling,
opfyldningsfradrag, selvstændiges 30 pct.-valg, betalingsår, indekskontrakter
og hjælpe- og understøttelsesfonde. Et typet § 4 a-resultat sørger for, at et
§ 15 A-pensionsbidrag, der allerede er fratrukket i aktieindkomsten, ikke også
fratrækkes i personlig indkomst. Kilderne er
[Pensionsbeskatningsloven, LBK nr. 1243/2024](https://www.retsinformation.dk/eli/lta/2024/1243)
og [Skatteministeriets beløbsgrænser](https://skm.dk/tal-og-metode/satser/satser-og-beloebsgraenser-i-lovgivningen/pensionsbeskatningsloven).

Personskatteloven § 3, stk. 2, nr. 4 er nu forbundet med typede resultater fra
Husdyrbeskatningsloven §§ 2 og 8. Den aktuelle § 2-vej håndterer
normalhandelsværdi, handelsværdi uden fradragsberettiget moms og loftet på 15
pct. § 8-vejens historiske overgangsordning holder kvæg, svin, får og heste
adskilt og modellerer A-, B- og C-fradrag, basisantal, restsaldo,
tilbageregulering og de tillæg til personlig indkomst, som en regulering kan
udløse. Kilderne er
[Husdyrbeskatningsloven, LBK nr. 1099/2025](https://www.retsinformation.dk/eli/lta/2025/1099),
[BEK nr. 543/1981](https://www.retsinformation.dk/eli/lta/1981/543) og
[Skattestyrelsens blanket 04.013](https://skat.dk/media/3nbjrsem/04013_dk_-final_16325.pdf).

Personskatteloven § 3, stk. 2, nr. 5 modtager tilsvarende et typet resultat fra
Varelagerloven § 1. Reglerne vælger opgørelsesmåde pr. varegruppe, fjerner
fradragsberettiget indgående moms og anvender den historiske satsrække. Det gør
også en usædvanlig lovrest synlig: henvisningen står stadig i
Personskatteloven, men den højeste nedskrivningssats har været 0 pct. siden
1998. Kilden er
[Varelagerloven, LBK nr. 1088/2025](https://www.retsinformation.dk/eli/lta/2025/1088).

Personskatteloven § 3, stk. 2, nr. 10 og 11 har nu også en egentlig
beløbskaskade. Nr. 10 modtager typede resultater for almindelig
saldoafskrivning, valgt tabsfradrag, straksafskrivning og den snævre ordinære
afskrivning efter Statsskatteloven. Afskrivningslovens aktuelle kilde- og
regelkorpus omfatter §§ 1-69: anskaffelse og benyttelsesændring,
den almindelige saldo og selskabers udlejningsforløb, særskilt skibssaldo,
15/7 pct.-infrastruktursaldi, de tidsafgrænsede 116/108 pct.-saldi,
straksfradrag og salg af straksafskrevne aktiver, selskabers udskudte fradrag
for udlejningsaktiver, skade og erstatning, negativ saldo, virksomhedsophør og
senere salg, dok- og beddingsanlæg, delvist erhvervsmæssigt benyttede aktiver,
bygninger og installationer. Bygningsdelen omfatter afgrænsning og tilknytning,
anskaffelsestidspunkt, 3/4 pct.- og levetidsafskrivning, 5 pct.-straksfradrag,
herunder ejerens valg af et lavere straksfradrag, delvise bygninger og stopår.
Den vedvarende § 19-historik bærer hvert særskilt anskaffelsessuminterval og
dets egen afskrivningsprocent videre til §§ 21-24. Dermed kan samme model
beregne genvundne afskrivninger og tab ved salg, nedrivnings- og skadefradrag,
samlet forskudsafskrivning fra bestilling til anskaffelse eller efterbeskatning,
mineralforekomsters dokumenterede værdiforringelse, forbedringer af lejede
lokaler og immaterielle aktiver samt godtgørelser og vederlag. §§ 38-40 holder
bl.a. lejeperiode, nærtstående og selskabskontrol, § 14-undtagelsen,
goodwillgrundlag, rettighedsperioder, 5-pct.-straksfradrag, yder, modtager og
salg som særskilte juridiske fakta. LOV 749/2025-overgangen er eksplicit:
aftaler før 2026 kan bevare det tidligere § 40, stk. 7-regime, mens aftaler fra
2026 henvises til Ligningsloven § 12 B. Den nye ordning er også beregnet i
Ligningsloven-modulet: henstand med skat og arbejdsmarkedsbidrag, forholdsmæssige
afdrag ved betaling eller afståelse af retten, rente, misligholdelse, ophør og
virksomhedsordningens konto for opsparet overskud holdes som særskilte, typede
resultater. Skattestyrelsens eksempel med
1.000.000 kr. goodwill, 515.000 kr. skat og 500.000 kr. kapitaliseret løbende
ydelse giver 257.500 kr. skattehenstand i begge Futuruna-backends. Vejledningens
virksomhedsordningseksempel reducerer tilsvarende en konto på 702.000 kr. til
292.500 kr., når 115.500 kr. virksomhedsskattehenstand frafaldes.

§§ 40 A-40 D føjer kvoter til samme eksekverbare kæde. Engangskvoter fordeler
den resterende anskaffelsessum forholdsmæssigt ved anvendelse, salg eller
udløb. Løbende kvoter bærer aftaleår, udnyttelsesperiode og tidligere
afskrivninger gennem et forløb på højst syv år. Ved salg af en andel fordeles
både anskaffelsessum og tidligere afskrivninger forholdsmæssigt, mens resten
fortsætter som en eksekverbar position. FIFO afledes af daterede kvotelots i
lageret, og vederlagsfri tildeling samt lovens udelukkelser er egne typede
forhold. § 40 C afleder selv sine saldobevægelser fra daterede
betalingsrettigheder, gamle og nye mælkekvoter og sukkerroerettigheder. Modellen
holder de historiske datogrænser, forholdsmæssige delafståelser,
forpagterreglen, udløb til nul og den særlige FIFO for et blandet mælkelager
synlige. Ved ejendomstab fører stk. 8-10 de tidligere indtægtsførte negative
saldi videre til modregning, 22 pct. acontoskat, egen og ægtefælles slutskat,
kontant udbetaling eller fremførsel. § 40 D omsætter handelsværdien ved
indtræden af dansk skattepligt eller dansk DBO-hjemsted til en
anskaffelsesbevægelse på § 40 C-saldoen. Derfor går § 40 A- og § 40 B-beløb
gennem Personskatteloven § 3, mens § 40 D kun når kapitalindkomsten gennem
§ 40 C og Personskatteloven § 4.

§§ 41-49 fortsætter med udtrykkeligt ophævet § 41, landboturisme,
tilslutningsafgifter, tilskudsbetalte aktiver, kunstnerisk udsmykning,
leveringskontrakter, kontantomregning og fordeling af overdragelsessummer samt
fælles regler om andre afståelsesformer, erstatning, gave og arv. § 42 holder
20 pct.-loft, momspligt, udlejningsindtægtsloft og salg adskilt. § 43 fører
restbeløbet ved salg som et særskilt fradrag uden at konstruere genvundne
afskrivninger. §§ 44 A-44 B bruger hver sin vedvarende kunstposition, mens §
44 C lader forskudsafskrivninger være uden betydning for selve
kontraktfortjenesten. § 45 modellerer den lovbestemte samlede fordeling på
driftsmidler og skibe med en typet nøgle; § 49 har tilsvarende en særskilt
tilstand for skattesuccession, hvor værdiansættelsesreglen ikke anvendes.

§§ 50-69 afslutter den konsoliderede paragrafsekvens. § 50 holder den nominelle
næringsfortjeneste adskilt fra den kontantomregnede saldodel, mens §§ 51-52
modellerer forsøgs- og forskningsudgifter før erhvervsstart samt ansøgningsfrist
og dispensation. § 53 og §§ 63-67 står eksplicit som ophævede. §§ 54-62 og
68-69 gør ikrafttrædelse, historiske saldi og afskrivningsgrundlag,
miljøinvesteringer, udlejningsregimet og den territoriale afgrænsning
eksekverbare. LOV 615/2026 er samtidig lagt oven på konsolideringen: skov- og
naturejendomme skifter anvendelsesområde i §§ 40 C og 42 fra 2027 gennem én
fælles, typet ejendomskategori.

Kun fradrag for en
selvstændig erhvervsdrivende person går videre som nr. 10-fradrag. En
skattepligtig fortjeneste for en fysisk person går i stedet videre i
nr. 10-resultatets særskilte indtægtsføringsfelt og medregnes automatisk af den
samlede § 3-kaskade. Modellen bruger de offentliggjorte 2026-
grænser på 36.000 kr. og deler forsøgs- og
forskningsudgifter mellem 114 pct. under loftet på 1.088,8 mio. kr. og 110 pct.
over loftet. Et § 5 A-tab bliver begrænset, ikke bare afvist, hvis den fulde
saldoformindskelse ellers ville føre under nul.

Den samme typede årsopgørelse bærer både faktisk erhvervsmæssig og samlet
benyttelse samt beregnet og fradraget afskrivning. Derfor kan § 12 gengive
Skattestyrelsens salgseksempel: 47.000 erhvervskilometer ud af 110.000 fordeler
68.000 kr. i fortjeneste til 29.055 kr. skattepligtig fortjeneste og 37.000 kr.
i tab til 15.809 kr. fradragsberettiget tab. Ved køb og salg i samme indkomstår
bruger reglerne i stedet årets erhvervsandel for både fortjeneste og tab.

Nr. 11 modtager et samlet resultat fra Etableringskontoloven §§ 1-4. Det holder
etableringskontoens ligningsmæssige fradrag adskilt fra iværksætterkontoens
fradrag i personlig indkomst og håndterer 5.000 kr.-minimum, 60 pct.-grænsen,
muligheden for altid at indskyde op til 250.000 kr., fælles kontoloft,
forskudsafskrivning og de kontoformer, loven kræver. Kilderne er
[Afskrivningsloven, LBK nr. 1222/2025](https://www.retsinformation.dk/eli/lta/2025/1222),
[ændringslov nr. 749/2025](https://www.retsinformation.dk/eli/lta/2025/749),
[ændringslov nr. 615/2026](https://www.retsinformation.dk/eli/lta/2026/615),
[Statsskatteloven, LOV nr. 149/1922](https://www.retsinformation.dk/eli/lta/1922/149),
[Etableringskontoloven, LBK nr. 1307/2025](https://www.retsinformation.dk/eli/lta/2025/1307),
[Skatteministeriets 2026-satser](https://skm.dk/tal-og-metode/satser/satser-og-beloebsgraenser-i-lovgivningen/afskrivningsloven),
[Skattestyrelsens saldovejledning](https://info.skat.dk/data.aspx?oid=2060781),
[Skattestyrelsens vejledning til § 6](https://info.skat.dk/data.aspx?oid=2060787),
[Skattestyrelsens vejledning og salgseksempler til §§ 11-13](https://info.skat.dk/data.aspx?oid=2060792),
[Skattestyrelsens bygningsafgrænsning til § 14](https://info.skat.dk/data.aspx?oid=2083984),
[Skattestyrelsens installationsvejledning til § 15](https://info.skat.dk/data.aspx?oid=2083985),
[Skattestyrelsens afskrivningsmetoder til §§ 16-20](https://info.skat.dk/data.aspx?oid=2083987),
[Skattestyrelsens vejledning til § 21](https://info.skat.dk/data.aspx?oid=2083989),
[Skattestyrelsens vejledning til § 22](https://info.skat.dk/data.aspx?oid=2083988),
[Skattestyrelsens vejledning til §§ 23-24](https://info.skat.dk/data.aspx?oid=2083990),
[Skattestyrelsens vejledning til § 38](https://info.skat.dk/data.aspx?oid=2083993),
[Skattestyrelsens vejledning til § 39](https://info.skat.dk/data.aspx?oid=2083992),
[Skattestyrelsens vejledning til § 40](https://info.skat.dk/data.aspx?oid=2083994),
[Skattestyrelsens kvotevejledning til §§ 40 A-40 D](https://info.skat.dk/data.aspx?oid=2083995),
[Skattestyrelsens vejledning til § 42](https://info.skat.dk/data.aspx?oid=2083996),
[Skattestyrelsens vejledning til § 43](https://info.skat.dk/data.aspx?oid=2083997),
[Skattestyrelsens vejledning til § 44](https://info.skat.dk/data.aspx?oid=2061440),
[Skattestyrelsens vejledning til §§ 44 A-44 B](https://info.skat.dk/data.aspx?oid=2083999),
[Skattestyrelsens vejledning til § 44 C](https://info.skat.dk/data.aspx?oid=2060796),
[Skattestyrelsens kontantomregning efter § 45](https://info.skat.dk/data.aspx?oid=1976528),
[Skattestyrelsens aktivfordeling efter § 45](https://info.skat.dk/data.aspx?oid=1976529),
[Skattestyrelsens vejledning til §§ 47-49](https://info.skat.dk/data.aspx?oid=1976531),
[Skattestyrelsens vejledning til § 50](https://info.skat.dk/data.aspx?oid=1976532),
[Skattestyrelsens vejledning til § 51](https://info.skat.dk/data.aspx?oid=1976534),
[SKM2022.474.SR om CO2-kvoter og afholdt anskaffelsesudgift](https://info.skat.dk/data.aspx?oid=2365032)
og [Skattestyrelsens konto-eksempler](https://skat.dk/erhverv/egen-virksomhed/etablerings-og-ivaerksaetterkonto).

Den fokuserede scenario-fil fører 8.000 kr. negativ saldo og 70.000 kr.
ophørsfortjeneste frem som indtægt og samler 12.000 kr. skibsafskrivning,
15.000 kr. infrastruktursaldo, 27.000 kr. 108 pct.-saldo, 12.000 kr.
reparationsfradrag og 30.000 kr. endeligt ophørstab. Det giver 78.000 kr.
indtægtsføring og 96.000 kr. fradrag gennem den samme § 3-regelkaskade.

Den fokuserede afståelses- og skadescenario samler 652.500 kr. i genvundne
afskrivninger og fristindtægt, 100.000 kr. i salgstab og 172.500 kr. i
nedrivnings- og skadefradrag uden at skjule dem i et nettobeløb.

Den fokuserede §§ 38-40-scenario validerer 24 grænser og kaskadeudfald i både
interpreter og kompileret kode, herunder forskellen mellem § 40-yderens fradrag
og modtagerens skattepligtige vederlag.

Det brede kvotescenarie validerer yderligere 27 forhold i begge backends:
delvis brug af engangskvoter, salg og udløb, syvårige og kortere
afskrivningsforløb, sidste års afrunding, forholdsmæssigt salg af en kvoteandel,
tidsrækkefølge, entydig FIFO, vederlagsfri tildeling, lovens udelukkelser,
tilflytningsårets saldoføring og hele § 40 D -> § 40 C -> Personskatteloven
§ 4-kæden.

Det fokuserede § 40 C-scenarie validerer 30 yderligere forhold i begge
backends, herunder de tre aktivarter, gamle mælkekvoters dobbelte saldoindgang,
delafståelser, stk. 12's blandede FIFO, ejendomstabsmodregning, 22 pct.
acontoskat, succession, ægtefællefradrag, kontant udbetaling og fremførsel til
senere indkomstår.

Det fokuserede §§ 50-69-scenarie validerer 23 yderligere forhold i begge
backends, herunder månedsslutningen i tremånedersfristen, historiske
satsgrænser, det foregående grundlagsår, overgangssaldi, § 21's tabsafskæring,
§ 24's genopførelsesværn og LOV 615/2026's 2027-skifte. Et
særskilt tværlovsscenarie fører afledte Afskrivningslov-resultater gennem
Personskatteloven § 3 og holder 98.000 kr. indtægt adskilt fra 60.000 kr.
afskrivning, 80.000 kr. tab og 25.000 kr. andet fradrag.

Afskrivningslovens kilde- og regelkorpus er dermed sammenhængende fra § 1 til
§ 69, også hvor bestemmelser er ophævede eller kun regulerer historiske
overgange. Det er ikke det samme som, at hele den danske
indkomstskattelovgivning er færdig: andre Personskattelov-bestemmelser,
afhængige love, delår, underskud, skatteloft, indeholdelse og slutopgørelsens
yderkanter skal fortsat uddybes.

Korpussets meta-kommentarer kan nu også læses maskinelt med
`runa meta --json`. Roller som `source`, `guidance` og `warning` er konventioner,
ikke indbyggede særtilfælde: enhver rolle kan pege på en almindelig Futuruna-
binding, og bindingens domænetype bliver søgbar i indekset. Sådan kan ordret
lovtekst, supplerende kilder og regelspans kobles uden at ændre programmets
semantik. Den kanoniske Personskat-beregning bruger ét kort meta-anker til ét
typet rodobjekt; feltbeskrivelser, kilder og vejledning er indlejrede værdier i
objektet og ikke ekstra syntaks i kommentaren. Ground metadata udstilles både
som læsbar Futuruna-værdi og som et
struktureret `data`-træ, så audits kan læse fx URL- eller advarselsfelter uden
at parse visningstekst. En meta-reference kan ligge ved beregningsreglen og pege
på en typet kildebinding i et rekursivt importeret kilderegister; indekset
bevarer den faktiske definitionsfil og linje. JSON-indekset adskiller desuden
`----`-markørernes linjer fra den ordrette råtekst mellem dem. En hel kildemappe kan gennemsøges
rekursivt efter vilkårlig type eller rolle, f.eks.
`--type HusdyrbeskatningslovKildeInfo` eller `--role warning`. Den historiske
korpusadvarsel til § 40 C kan derfor findes både via rollen `warning` og typen
`AfskrivningslovKorpusAdvarsel`, uden at metadatasystemet kender noget særligt
til skattelovgivning.

## Eksempel

Den nuværende scenario-fil modellerer en mand med 50.000 kr. i månedsløn,
ægtefælle med 20.000 kr. i månedsløn, 10.000 kr. i månedlig husleje, tre børn,
København, 2026 og ingen kirkeskat.

Aktuel modeludgang:

- mandens årlige skat inkl. AM efter personfradrag: 208.726 kr.
- mandens månedlige skat: ca. 17.393 kr.
- husholdningens samlede månedlige netto: 46.689 kr.
- husholdningens månedlige rådighed efter husleje: 36.689 kr.

Børneydelser, boligstøtte og anden social ydelsesret ligger uden for denne
Personskatteloven/Kildeskatteloven-slice.

## Auditfund

Den konfiskatoriske audit søger 8.064 konfigurationer. Den finder ingen
almindelig årsskat over 100 pct. af positivt indkomstgrundlag i det aktuelle
søgerum.

Den finder derimod 360 konfigurationer af skatteforhold, hvor den samlede
betalingsbelastning overstiger 100 pct. af årets indkomstgrundlag. Før man
finder høtyvene frem, er forklaringen vigtig: i alle de fund skyldes det
overført restskat m.v.; altså ikke en skjult almindelig skattesats over 100
pct., men tidligere års betalingsproblem gjort eksekverbart og auditérbart.

## Kildeprincip

Aktuel arbejdskilde er Retsinformation, LBK nr. 1284 af 14/06/2021, med sporede
ændringer og afhængige love. Den historiske kilde fra projektoplægget, LBK nr.
799 af 07/08/2019, bevares som historisk reference.

Historiske kilder må ikke lydløst drive aktuel beregning. Regler, der afhænger
af frister, valg, omgørelse, meddelelser eller andre retlige handlinger, bør
modelleres som typede juridiske resultater frem for løse boolske værdier.
