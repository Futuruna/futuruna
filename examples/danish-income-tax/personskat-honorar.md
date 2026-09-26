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
  "skattepligtig_værdi_kroner": 50000,
  "udgifter": {
    "$variant": "PsHonorarudgifterOplyst",
    "poster": [],
    "fuldstændige": true
  }
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

## Udgifter holdes adskilt fra bruttovederlaget

Indtast ikke blot et nettohonorar: det ville også ændre modellens AM-grundlag.
Feltet `udgifter` behandler dokumenterede løbende egne udgifter ved
honoraraktiviteten. Fradraget reducerer personlig indkomst, ikke AM-grundlaget
eller beskæftigelses-/jobfradragsgrundlaget. Det er ikke et lønmodtagerfradrag
med bundgrænse. Se [C.C.1.2.3](https://info.skat.dk/data.aspx?oid=2048532) og
[SKATs honorarmodtagervejledning](https://skat.dk/erhverv/egen-virksomhed/afklar-virksomhedens-skatteforhold).

For ovenstående fiktive honorar kan `udgifter` fx erstattes med:

```json
{
  "$variant": "PsHonorarudgifterOplyst",
  "poster": [{
    "identifikation": "fiktivt-materialebilag-1",
    "indkomstår": 2025,
    "art": { "$variant": "PsHonorarLøbendeUdgift" },
    "egen_udgift_kroner": 10000,
    "dokumenteret": true,
    "vedrører_vederlaget": true,
    "ikke_fratrukket_andetsteds": true
  }],
  "fuldstændige": true
}
```

Beløbet er den gennemgåede egen udgift efter refusion og udskillelse af privat
andel. Bevar opdelingen i bilagsoversigten; modellen autentificerer den ikke.
En kendt privat udgift (`PsHonorarPrivatUdgift`) giver nul fradrag. Andre eller
uafklarede udgiftstyper tilbageholder sammenligningen, ikke et godkendt nul.
En rubrik 29-total er en observation til sammenligning, ikke automatisk en ny
udgift oven i bilagene. AM-bidrag og allerede betalt B-skat er ikke udgifter her.

Saml samme aktivitets årsvederlag i én dokumenteret bruttoopgørelse, når
udgifterne er fælles. Bland ikke selvstændige aktiviteter. Brug én stabil
reference pr. udgift og medtag den kun én gang, også på tværs af honorarrækker
og andre fradragsgrene. Gentagne id'er i honorarrækker afvises mekanisk;
forskellige id'er er ikke bevis på, at bilagene er forskellige.

Vælg `PsHonorarudgifterUoplyst`, når udgifterne ikke er afklaret. En tom liste
med `fuldstændige: true` betyder **bekræftet ingen udgifter**, ikke manglende
bilag. En skabelon eller en manglende rapportlinje er ikke sådan bekræftelse.

Den almindelige gren dækker ikke afskrivninger, satsberegnet kørsel,
erstatninger, udenlandske særregler eller omperiodisering. Den tilbageholder
også sammenligningen, hvis udgifterne overstiger vederlaget efter den
alders- og årsbestemte AM-beregning. Det er en **modelgrænse, ikke et juridisk
fradragsloft**: C.C.1.2.3 henviser til SKM2025.490.ØLR, som underkender
kildeartsbegrænsning, og varsler et styresignal. Modellen hverken klipper
udgiften, nægter generel fradragsret eller beregner underskudsfremførsel.
Disse tilfælde kræver særskilt behandling; ændr ikke fakta for at få et match.

I et voksent fiktivt 2025-tilfælde med 600.000 kr. løn, 50.000 kr. honorar og
10.000 kr. understøttede udgifter skal AM fortsat være 52.000 kr., mens
personlig indkomst efter AM og disse udgifter bliver 588.000 kr. At skrive
40.000 kr. i bruttofeltet ville i stedet give AM på 51.200 kr. og personlig
indkomst på 588.800 kr. [Den kanoniske regression](../../tests/personskat_honorar_expenses.test.mjs)
kontrollerer denne forskel samt ægtefælle- og delårsforløb. Det er ikke
uafhængig kontrol mod en officiel beregner eller en rigtig årsopgørelse.

## Læs vurderingen, ikke kun en nulpost

Hvis løn eller virksomhedsindtægt står i honorarlisten, bevarer komponenten
`PersonligtArbejdsvederlagUdenForPar2Stk1Nr2` den oprindelige klassifikation.
Den samlede årsberegning tilbageholder sammenligningen:

- `vurdering.status` er `UgyldigtBeregningsgrundlag`.
- `vurdering.slutskat_til_sammenligning_øre` er `null`.
- `vurdering.fejl` peger på honorarlisten og forklarer, at indkomsten ikke er
  skattefri. De resterende beløb er diagnostik, ikke en godkendt delsum.

Samme kontrol gælder en beregnet ægtefælle. Delårsberegningens eksisterende
kontrol af det samlede personlige indkomstgrundlag medtager også denne fejl.
Fakta om ansættelse eller virksomhed må ikke omskrives til honorar alene
for at få et resultat; ret placeringen efter kildegennemgang og kontrollér
mod dobbelt medregning.

[Komponentkontrollen](../../tests/personskat_work_income_route_test.runa) og
[årsregressionen](../../tests/personskat_work_income_route.test.mjs) skelner
gyldig lovafgrænsning fra en komplet årsberegning. Det er modelkontroller,
ikke uafhængig verifikation af en virkelig årsopgørelse.

## Eksisterende input

Generér en frisk skabelon og overfør gennemgåede fakta; redigér ikke et gemt
`schema_hash` for at omgå kontrollen. Se de konkrete migrationskrav i
[kompatibilitetsvejledningen](../../docs/compatibility-guides/0.2.x.md#preview-and-experimental-notes).
