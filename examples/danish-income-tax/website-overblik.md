# Personskatteloven i Futuruna

Denne webside er et dansk overblik over arbejdet med at omsætte
Personskatteloven og nødvendige afhængige regler til Futuruna. Websitet er ikke
selve korpusset. Det viser kun denne ene projektfil, mens lovfiler, audits og
scenarier ligger i `examples/danish-income-tax/`.

I Futuruna-filerne følger strukturen den juridiske kilde tæt:

- original dansk lovtekst i flerlinjeblok
- eventuel kort note, hvis kilden kræver forklaring
- faktiske Futuruna-regler med `|`, `under` og `exception`

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
