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
underskud, delår, skatteloft, indeholdelse, slutopgørelse og dele af
Ligningslovens fradragsregler.

Det er endnu ikke en fuld implementering af hele Personskatteloven. Det næste
vigtige arbejde er at gøre de resterende kildepostur- og kategoriregler til
kildebundne beløbsregler.

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
og eventuelle arbejdsgiveraftale opfylder lovens betingelser.
Personskatteloven § 3, stk. 2, nr. 7 er nu også koblet til de beregnede
henlæggelser efter Virksomhedsskatteloven §§ 22 b og 22 d, herunder procent- og
beløbslofter, udligningsskat, bundet konto og rettidigt indskud.
Personskatteloven § 3, stk. 2, nr. 8 og 9 modtager nu tilsvarende typede
resultater fra Ligningsloven §§ 9 B og 8 O. § 9 B-reglerne afgør bl.a.
60-dages-perioder, Skatterådets kilometersatser, skattefri eller personlig
godtgørelse, § 9 C-henvisning og den kundeopsøgende undtagelse for flere
arbejdsgivere. § 8 O-reglerne skelner mellem ydelseskredsen før 2026 og den
udvidede kreds fra 2026, begrænser fradraget til tidligere beskattede beløb og
afskærer dobbeltfradrag. Kilderne er [Ligningsloven, LBK nr. 1500/2025](https://www.retsinformation.dk/eli/lta/2025/1500),
[LOV nr. 198/2025](https://www.retsinformation.dk/eli/lta/2025/198) og
[Skatterådets BEK nr. 1333/2025](https://www.retsinformation.dk/eli/lta/2025/1333).

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
afskrivning efter Statsskatteloven. Afskrivningslovens aktuelle beregningsslice
omfatter §§ 1-40 D: anskaffelse og benyttelsesændring,
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
forhold. § 40 D omsætter handelsværdien ved indtræden af dansk
skattepligt eller dansk DBO-hjemsted til en anskaffelsesbevægelse på § 40 C-
saldoen. Derfor går § 40 A- og § 40 B-beløb gennem Personskatteloven § 3, mens
§ 40 D kun når kapitalindkomsten gennem § 40 C og Personskatteloven § 4.

Kun fradrag for en
selvstændig erhvervsdrivende person går videre som nr. 10-fradrag. En
skattepligtig fortjeneste for en fysisk person går i stedet videre som en
særskilt personlig indkomstpost. Modellen bruger de offentliggjorte 2026-
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

Kvotescenariet validerer yderligere 27 forhold i begge backends: delvis brug af
engangskvoter, salg og udløb, syvårige og kortere afskrivningsforløb, sidste års
afrunding, forholdsmæssigt salg af en kvoteandel, tidsrækkefølge, entydig FIFO,
vederlagsfri tildeling, lovens udelukkelser, tilflytningsårets saldoføring og
hele § 40 D -> § 40 C -> Personskatteloven § 4-kæden.

Denne del er beregningsegnet, men endnu ikke hele Afskrivningsloven. Det
resterende dependency-arbejde omfatter § 40 C's regler om ældre mælkekvoter,
ejendomstab, acontoskat og FIFO, §§ 41-49 samt de historiske overgangsregler om
bl.a. anskaffelsessummer, afståelse og ophør.

Korpussets meta-kommentarer kan nu også læses maskinelt med
`runa meta --json`. Roller som `source`, `guidance` og `warning` er konventioner,
ikke indbyggede særtilfælde: enhver rolle kan pege på en almindelig Futuruna-
binding, og bindingens domænetype bliver søgbar i indekset. Sådan kan ordret
lovtekst, supplerende kilder og regelspans kobles uden at ændre programmets
semantik. Ground metadata udstilles både som læsbar Futuruna-værdi og som et
struktureret `data`-træ, så audits kan læse fx URL- eller advarselsfelter uden
at parse visningstekst. JSON-indekset adskiller desuden `----`-markørernes linjer fra den
ordrette råtekst mellem dem. En hel kildemappe kan gennemsøges rekursivt efter
vilkårlig type eller rolle, f.eks. `--type HusdyrbeskatningslovKildeInfo` eller
`--role warning`.

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
