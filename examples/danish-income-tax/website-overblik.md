# Personskatteloven i Futuruna

Denne webside er et dansk overblik over arbejdet med at omsætte
Personskatteloven og nødvendige afhængige regler til Futuruna. Websitet er ikke
selve korpusset. Det viser højst denne ene projektfil, mens lovfiler, audits og
scenarier ligger i `examples/danish-income-tax/`.

I Futuruna-filerne følger strukturen den juridiske kilde tæt:

- original dansk lovtekst i flerlinjeblok
- eventuel kort note, hvis kilden kræver forklaring
- faktiske Futuruna-regler med `|`, `under` og `exception`

## Et samlet sprog til lov og ret

Futuruna gør det muligt at skrive juridiske regler i en form, der stadig ligner
lov: typer for de juridiske begreber, `|`-regler for det der gælder, `under` for
betingelser, `exception` for undtagelser og `?` for audits.

Det gør lovteksten læsbar som jura, men også eksekverbar som beregning.

## Personskatteloven som indkomstskat

Personskatteloven er en reel prøve, fordi dansk indkomstskat ikke er én enkelt
formel. Den trækker på AM-bidrag, kommunal skat, kirkeskat, kildeskat,
forskudsregistrering, slutopgørelse, aktieindkomst, kapitalindkomst,
ægtefælleregler, underskud og afhængige love.

Målet er at gøre den samlede danske indkomstskattelovgivning udtrykkelig i ét
sprog, så den både kan beregne almindelige skatteforløb og bruges til audits af
retlige knæk.

## Eksempel

Den nuværende scenario-fil modellerer en fiktiv mand med 50.000 kr. i månedsløn,
ægtefælle med 20.000 kr. i månedsløn, 10.000 kr. i månedlig husleje, tre børn,
København, 2026 og ingen kirkeskat.

I den aktuelle model giver det:

- mandens årlige skat inkl. AM efter personfradrag: 208.726 kr.
- mandens månedlige skat: ca. 17.393 kr.
- husholdningens samlede månedlige netto: 46.689 kr.
- husholdningens månedlige rådighed efter husleje: 36.689 kr.

Børneydelser, boligstøtte og anden social ydelsesret er ikke med i denne slice.

## Audits

Futuruna kan også søge efter hårde eller mærkelige skatteforhold. Den aktuelle
konfiskatoriske audit søger 8.064 konfigurationer og finder ingen tilfælde, hvor
den almindelige årsskat overstiger 100 pct. af positivt indkomstgrundlag. Den
finder derimod over 200 betalingsbelastnings-tilfælde over 100 pct.; i den
nuværende audit kræver de alle overført restskat m.v.

Det betyder: før man finder høtyvene frem, er fundet ikke en skjult almindelig
skattesats over 100 pct. Det er restskat og betalingsbelastning fra tidligere år,
formaliseret og gjort synlig.

## Status

Projektet er forbi prototypefasen. Der findes en beregningsegnet første del
for almindelige lønmodtagerforløb, kapitalindkomst, aktieindkomst, personfradrag,
underskud, delår, skatteloft, indeholdelse og slutopgørelse.

Det er stadig ikke en fuld implementering af hele Personskatteloven. Den
vigtigste resterende opgave er at gøre de tilbageværende kildepostur- og
kategoriregler til kildefaste beløbsregler.

## Kildeprincip

Den aktuelle arbejdskilde er Retsinformation, LBK nr. 1284 af 14/06/2021, med
sporede ændringer og afhængige love. Den historiske kilde fra projektoplægget,
LBK nr. 799 af 07/08/2019, bevares som historisk reference og audit-linje.

Historiske kilder må ikke lydløst drive aktuel beregning. Når en regel afhænger
af frister, valg, omgørelse, meddelelser eller andre retlige handlinger, bør den
modelleres som et typet juridisk resultat frem for som løse boolske værdier.

## Websitegrænse

Websitet skal forklare projektet og dets status. Det skal ikke gengive alle
lovfilerne som én lang artikel. Den fulde implementering skal læses, køres og
auditeres i Futuruna-projektet.
