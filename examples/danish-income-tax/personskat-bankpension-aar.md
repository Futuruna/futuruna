# Arbejdsgiverens rateopsparing: betalingsår eller godkendt år?

En almindelig arbejdsgiveradministreret rateopsparing i et pengeinstitut følger
som udgangspunkt det faktiske betalingsår og det års beløbsgrænser. Ifølge
[Den juridiske vejledning 2026-2, C.A.10.2.2.3.1](https://info.skat.dk/data.aspx?oid=2048283)
kan Skattestyrelsen godkende året for løntilbageholdelsen, når arbejdsgiveren
indbetaler de tilbageholdte bidrag det følgende år inden rimelig tid.
Vejledningen er læst 26. september 2026; det almindelige udgangspunkt er
[PBL § 19, stk. 1](https://www.retsinformation.dk/eli/lta/2024/1243#P19).

Futuruna **giver ikke godkendelsen** og beregner ikke selv, hvad rimelig tid er.
En AI må ikke sætte en selvvalgt januar- eller aprilfrist i stedet. Det er heller
ikke nok, at årets beregnede skat ville passe bedre med en tidligere placering.
Dette er et dokumenteret input til en forskningsmodel, ikke individuel rådgivning.

## Afklar behandlingen først

Feltet er `lønmodtager.pension.pbl18_indbetalinger.betaling.bank_årsplacering`.
Det samme felt findes i en beregnet ægtefælles pensionsbetalinger.

- `null` er uafklaret for en positiv, almindelig arbejdsgiver-rateopsparing.
  Modellen tilbageholder sammenligningen, også hvis forfald og betaling er
  angivet i samme år: forfaldsåret beviser ikke året for løntilbageholdelsen.
- `{"$variant":"Pbl19BankBetalingsår"}` bekræfter afklaret almindelig
  behandling uden denne særgodkendelse. Vælg ikke dette blot fordi dokumentet
  mangler. Faktisk betalingsår bevares, og almindelig timing anvendes.
- `Pbl19BankGodkendtÅr` kræver dokumenteret afgørelse, løntilbageholdelse og
  indbetaling. En verserende ansøgning eller uafklaret dækning er ikke godkendelse.

Almindelig behandling kræver ikke et nyt myndighedsdokument, som bekræfter
»ingen godkendelse«. Hvis de gennemgåede løn-/bankbilag viser, at bidragene
både er tilbageholdt og indbetalt i samme år, er den beskrevne tidligere-årsregel
ikke relevant. Et årstal på årsopgørelsen alene beviser derimod ikke begge
betalingsfakta. Interviewet skal afklare den konkrete betaling, ikke bede alle
brugere om at ansøge om en særlig godkendelse.

Private bidrag og forsikringspræmier behøver ikke dette felt. Det ændrer heller
ikke deres regler. [§ 22 E-korrektioner](personskat-pensionskorrektion.md) har
deres egne dokumentations- og fristregler; en kombination med en bankgodkendelse
er ikke understøttet og må ikke fremstilles som en almindelig betaling.

## Fiktivt eksempel på dokumenteret godkendelse

En afgørelse omfatter 50.000 kr. brutto tilbageholdt i lønnen i 2025 og indbetalt
12. januar 2026. Den godkender henføring til 2025. Betalingsposten beholder
`betalingsår = 2026`; feltet ovenfor får dette objekt:

```json
{
  "$variant": "Pbl19BankGodkendtÅr",
  "godkendelse": {
    "identifikation": "fiktiv-afgørelsespost-1",
    "kildereference": "fiktiv afgørelse, side 1",
    "løntilbageholdelse_kildereference": "fiktiv lønseddel, linje 3",
    "tilbageholdelsesår": 2025,
    "godkendt_indkomstår": 2025,
    "tilbageholdt_brutto_kroner": 50000,
    "godkendt_brutto_kroner": 50000
  },
  "indbetalingsdato": { "år": 2026, "måned": 1, "dag": 12 },
  "indbetalingen_omfattet_af_godkendelsen": true
}
```

Beløb er **før AM-bidrag**. Den almindelige betalingspost oplyser separat det
faktisk indeholdte AM-bidrag; modellen bruger netto i de relevante grænser.
Afgørelsen kan omfatte flere betalinger: gentag samme godkendelses-id og samme
godkendelsesfakta, men bevar hver betalings dato, beløb og eget post-id.
Deres samlede brutto må ikke overstige godkendelsens brutto. Brug et stabilt id
for hver særskilt afgørelsespost/ramme, ikke nye id'er til kopier af samme ramme.
Dokumenteret løntilbageholdelse kan være større end den godkendte del.

Modellen kontrollerer, at godkendt år er tilbageholdelsesåret, faktisk betaling
sker det følgende år, dato og betalingsår passer sammen, og godkendelsens
beløb ikke overstiger den dokumenterede løntilbageholdelse. Den bevarer fakta
og viser årsplaceringen særskilt. Ugyldigt grundlag kan stadig have diagnostiske
mellembeløb; kun `vurdering.slutskat_til_sammenligning_øre` er sammenligningsfeltet.

Denne rute gælder almindelig `Pbl18Rateopsparing` via
`Pbl18Arbejdsgiverindbetaling`. En godkendelse indsat på private betalinger,
forsikring eller særlige § 15 A/B-ordninger tilbageholder sammenligningen.
Den afgrænsning er modeldækning, ikke en påstand om, at myndigheden aldrig kan
godkende andre ordninger. Brug ikke den ældre aggregerede arbejdsgiverydelse
til at skjule en uafklaret årsplacering; dens summer repræsenterer ikke denne
afgørelses dokumentation. Flyt heller ikke løn til et andet år uden selvstændigt
kildegrundlag. Modellen autentificerer ikke afgørelser eller id'er, opdager
ikke samme dokument under forskellige id'er og afstemmer ikke flere personers
eller indkomstårs separate inputfiler.

## Migration og kontrol

Generer frisk schema/skabelon. `bank_årsplacering` er et nyt valgfrit JSON-felt,
men ikke valgfri afklaring for den relevante bankbetaling. Tidligere gemte
arbejdsgiver-rateopsparinger uden feltet kræver gennemgang; kopier ikke
eksemplets godkendelse eller en fiktiv bekræftelse til en rigtig sag.
`.runa`-konstruktører af `Pbl18Årsbetaling` angiver feltet eksplicit.

[Komponentgrænser](../../tests/personskat_pension_bank_year_test.runa) og
[kanoniske scenarier](../../tests/personskat_pension_bank_year.test.mjs) dækker
årsvalg, ukendte/modstridende fakta, fælles godkendelsesramme, rategrænse,
ægtefælle og genererede kildespor. Det er kildebaserede modelkontroller, ikke
observationer af virkelige afgørelser eller ny uafhængig SKAT-konformitet.
