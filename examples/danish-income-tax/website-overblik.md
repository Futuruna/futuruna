# Personskatteloven i Futuruna

Dette er den ene webviste projektfil for arbejdet med dansk personskat. Selve
lovoversættelsen ligger i `examples/danish-income-tax/` som Futuruna-regler,
scenario-filer og audit-filer.

## Struktur

Futuruna-filerne følger samme gentagne form:

- original dansk lovtekst i flerlinjeblok
- kun en kort note, hvis kilden kræver det
- faktiske regler med `|`, `under` og `exception`

## Aktuel status

Projektet er beregningsegnet for en væsentlig lønmodtagervej: AM-bidrag,
kommunal skat, kirkeskat, aktieindkomst, kapitalindkomst, personfradrag,
underskud, delår, skatteloft, indeholdelse og slutopgørelse.

Det er endnu ikke en fuld implementering af hele Personskatteloven. Det næste
vigtige arbejde er at gøre de resterende kildepostur- og kategoriregler til
kildebundne beløbsregler.

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

Den finder derimod over 200 konfigurationer, hvor betalingsbelastningen
overstiger 100 pct. Det skyldes i de fund overført restskat m.v.; altså ikke en
skjult almindelig skattesats over 100 pct., men tidligere års betalingsproblem
gjort eksekverbart og auditérbart.

## Kildeprincip

Aktuel arbejdskilde er Retsinformation, LBK nr. 1284 af 14/06/2021, med sporede
ændringer og afhængige love. Den historiske kilde fra projektoplægget, LBK nr.
799 af 07/08/2019, bevares som historisk reference.

Historiske kilder må ikke lydløst drive aktuel beregning. Regler, der afhænger
af frister, valg, omgørelse, meddelelser eller andre retlige handlinger, bør
modelleres som typede juridiske resultater frem for løse boolske værdier.
