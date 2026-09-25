# Honorarer og B-indkomst: indkomsten må ikke forsvinde mellem felterne

Et beløb kaldet »honorar«, »freelance« eller »B-indkomst« afgør ikke alene,
hvordan indkomsten skal behandles. Afklar først indkomstens art og forholdet
til betaleren. Bevar aftale, bilag, indkomstår og den relevante indberetning
som kilder — uden CPR-numre i projektet.

Personligt arbejdsvederlag uden for både ansættelse og selvstændig virksomhed
har en særskilt AM-gren efter AMBL § 2, stk. 1, nr. 2. Løn og virksomhed
følger andre regler; at falde uden for netop denne bestemmelse gør ikke
indkomsten skattefri. Afgrænsningen kræver en konkret vurdering, ikke blot
en titel på fakturaen. Se [Den juridiske vejledning C.A.3.1.2](https://info.skat.dk/data.aspx?oid=1976769)
og [C.A.12.3](https://info.skat.dk/data.aspx?oid=1976913).

## Vælg felt ud fra kildefakta

Følg [runtime-tjekket](../../website/public/ai-setup.md#tax-audit-runtime-check)
og generér det aktuelle `beregn_personskat`-input efter
[gyldighedsvejledningen](personskat-validity.md). Gem personlige input og
resultater uden for projektet. Arbejdsvederlagene ligger under:

```text
lønmodtager.personlig_indkomst.ordinære_forhold.personlige_arbejdsvederlag
```

Denne liste behandler vederlag klassificeret som `PsArbejdeUdenAnsættelse`.
For et dokumenteret almindeligt pengehonorar uden udgifter er en række fx:

```json
{
  "identifikation": "fiktivt-foredrag-bilag-1",
  "indkomstår": 2025,
  "forhold": { "$variant": "PsArbejdeUdenAnsættelse" },
  "vederlagsform": { "$variant": "PsArbejdsvederlagIPenge" },
  "skattepligtig_værdi_kroner": 50000
}
```

Dette er **fiktivt** og kun et rækkeuddrag, ikke en komplet inputfil.
Brug bruttovederlaget før AM-bidrag; allerede betalt B-skat er betaling,
ikke en reduktion af indkomsten. Kopiér ikke både et honorarbilag og den
samme indberettede indkomst som to betalinger. Samme beløb må heller ikke
allerede ligge i `bruttoløn_kroner`, øvrig personlig indkomst eller en
virksomhedspost. Identiske række-id'er afvises, men forskellige id'er beviser
ikke, at betalingerne er forskellige.

SKATs [vejledning om B-indkomst](https://skat.dk/borger/b-indkomst) skelner
mellem indkomst, forskudsregistrering og indbetaling. Rubrik 12/15 på
årsopgørelsen og felt 210/207 på forskudsopgørelsen er ikke samme slags
feltnumre. En rubrik-total kan omfatte forskellige indkomstarter; den må ikke
automatisk erstatte gennemgåede kildefakta.

- **Ansættelse:** `PsArbejdeIAnsættelse` i honorarlisten er en afgrænsning,
  ikke automatisk flytning til lønfeltet. Gennemgå den almindelige
  [løn-, pensions- og ATP-afgrænsning](pension-og-fradrag.md). Medregn den
  dokumenterede indkomst præcis én gang i den relevante løn-/ydelsesgren.
- **Selvstændig virksomhed:** `PsSelvstændigtArbejde` flytter ikke beløbet
  til virksomhed. Den relevante virksomhedsgren kræver eget indkomst- og
  udgiftsgrundlag; et bruttovederlag er ikke automatisk virksomhedens overskud.
- **Uafklaret relation:** honorarlisten har ikke en ukendt relation. Afvent
  afklaring før en uafhængig skattesammenligning; vælg ikke en vilkårlig
  variant. [Betinget rapportafstemning](aarsopgoerelse-afstemning.md) kan
  stadig undersøge rapportens egne tal uden at bevise klassifikationen.

Har honoraret tilknyttede udgifter, er den enkle række ikke et komplet
grundlag. Den har ikke et særskilt honorarudgiftsfelt. Indtast ikke blot et
nettohonorar: det ville også ændre modellens AM-grundlag. Flyt heller ikke
udgiften til et vilkårligt lønmodtagerfradrag for at opnå samme slutbeløb.
Udgiftsbehandlingen kræver særskilt kilde- og modelafklaring. Naturalier,
udenlandske forhold og korrektioner kræver tilsvarende deres relevante regler.

## Læs vurderingen, ikke kun en nulpost

Hvis løn eller virksomhedsindtægt står i honorarlisten, bevarer komponenten
`PersonligtArbejdsvederlagUdenForPar2Stk1Nr2` den oprindelige klassifikation.
Den samlede årsberegning tilbageholder nu sammenligningen:

- `vurdering.status` er `UgyldigtBeregningsgrundlag`.
- `vurdering.slutskat_til_sammenligning_øre` er `null`.
- `vurdering.fejl` peger på honorarlisten og forklarer, at indkomsten ikke er
  skattefri. De resterende beløb er diagnostik, ikke en godkendt delsum.

Samme kontrol gælder en beregnet ægtefælle. Delårsberegningens eksisterende
kontrol af det samlede personlige indkomstgrundlag medtager også denne fejl.
Fakta om ansættelse eller virksomhed må ikke omskrives til honorar alene
for at få et resultat; ret placeringen efter kildegennemgang og kontrollér
mod dobbelt medregning.

En fiktiv 2025-modelkontrol viste fejlen: 600.000 kr. løn plus 50.000 kr.
placeret som ansættelse i honorarlisten gav tidligere samme godkendte skat
som kun 600.000 kr. løn. Den forkert placerede indkomst var udeladt.
Det korrekt klassificerede honorar giver fortsat et samlet AM-grundlag på
650.000 kr. og AM på 52.000 kr. i dette voksne kontroltilfælde.
[Komponentkontrollen](../../tests/personskat_work_income_route_test.runa) og
[årsregressionen](../../tests/personskat_work_income_route.test.mjs) skelner
gyldig lovafgrænsning fra en komplet årsberegning. Det er modelkontroller,
ikke uafhængig verifikation af en virkelig årsopgørelse.

## Eksisterende input

Gyldighed og vejledning ændres, men ikke skatteformler eller inputtyper.
Den nye metadata ændrer kontraktens fingeraftryk: generér frisk skabelon og
overfør gennemgåede fakta, ikke blot et nyt `schema_hash`.
Et tidligere gyldigt resultat med indkomst i den forkerte gren skal genåbnes;
nulbidraget fra komponenten må ikke bruges som dokumentation for skattefrihed.
