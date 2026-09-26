# Review a report without the spouse's tax documents

You can check an årsopgørelse without reconstructing the spouse's affairs.
The question is conditional: **what would have to be true for these reported
amounts to fit together?**

[`aarsopgoerelse-afstemning.calculate.runa`](aarsopgoerelse-afstemning.calculate.runa)
exposes `afstem_årsopgørelse`. It uses exact integer arithmetic and the existing
year-specific tax parameters. No LLM evaluates the rules. A human or assistant
transcribes the report; Futuruna calculates the checks and necessary conditions.
This research workflow uses the Preview typed-calculation interface.

## Two distinct kinds of review

| Review | Inputs | What the result establishes |
| --- | --- | --- |
| Independent calculation, `beregn_personskat` | Supported source facts, including relevant spouse facts | What the encoded rules calculate from those facts |
| Conditional reconciliation, `afstem_årsopgørelse` | Observed report lines and explicitly known conditions | Whether the selected arithmetic agrees, what transfers are necessary, and whether checked bounds contradict them |

Never feed the reconciliation's inferred amounts into `PersonskatInput` as
verified facts. Matching the report this way would be circular. Necessary
conditions are not a proof that a legally valid household with those facts
exists, and neither workflow certifies the underlying documents.

The independent calculation now has an explicit `ÆgtefællegrundlagUoplyst`
default. It withholds comparison when spouse information is unavailable,
without preventing this conditional review. Do not switch it to
`UdenÆgtefælle` simply to obtain a number; see the
[input and migration guide](personskat-validity.md#manglende-ægtefælleoplysninger-er-ikke-ingen-ægtefælle).

## Run the compact review

First run the [tax-audit runtime check](../../website/public/ai-setup.md#tax-audit-runtime-check)
against the compiler you will use. The original `v0.2.0` download predates
required calculation-safety fixes; the version string alone is insufficient.
Do not continue on a failed check. It uses only synthetic data, not your report.

### Prøv først med fiktive tal

Du kan se et dansk eksempel uden at finde eller indtaste dokumenter. Brug den
absolutte sti i `RUNA_BIN`, som bestod kompatibilitetstjekket:

```sh
node examples/danish-income-tax/afstemning-demo.mjs "$RUNA_BIN"
```

Demonstrationen kører fire fiktive rapporter gennem den kompakte model: én med
betinget match, én med manglende overførselslinjer og én med en forskel på én
øre samt én, hvor en manglende linje ikke kan forklare for store kendte
overførsler. I alle fire er ægtefællens kommune og samlivsbetingelse ukendte.
De første tre viser det nødvendige indkomstfradrag på 12.000 DKK og det
nødvendige samlede skattenedslag på 380.000 øre (3.800 DKK). Det fjerde viser
en modstrid, selv om det kommunale nedslag ikke er oplyst: de kendte overførsler
er allerede én øre for store. Manglende observationer vises
som **ukendt**, ikke nul. Tallene er ikke beregnet af JavaScript eller en LLM;
JavaScript kalder Futuruna, kontrollerer de fiktive forventninger og viser svaret.

Input og fulde resultater med alle forbehold gemmes i en ny midlertidig mappe
uden for projektet; stien vises til sidst. Scriptet læser ingen personlige
dokumenter. Eksemplerne er ikke skabeloner for dine egne fakta, og et betinget
match dokumenterer ikke, at ægtefællens forhold er lovligt rekonstrueret.

### Use your own report observations

Keep the PDFs, transcriptions, and results in a private directory **outside the
checkout**. Replace `PRIVATE_WORK_DIR` below with its actual path, and keep
`RUNA_BIN` set to the exact absolute path that passed the check. From the repo:

Oplys **rapportens skattekommune for indkomståret**, ikke automatisk din
nuværende bopælskommune. Afstemningen efterprøver ikke valget ud fra
bopælsfakta. Se [flytteeksemplet og hovedreglen](pension-og-fradrag.md#skattekommunen-er-ikke-altid-din-nuværende-bopæl).

Oplys også kirkeskat for **rapportens indkomstår**, ikke medlemskab i dag.
I denne betingede indgang betyder `betaler_kirkeskat: true`, at personen var
omfattet hele eller en del af året. `false` kræver afklaret ingen kirkeskat
hele året; et nulbeløb eller ukendt status er ikke nok. Ved ind-/udmeldelse
er det beregnede kirkeloft kun en helårs-overgrænse. Afstemningen efterprøver
hverken medlemsperioden eller det præcise periodiserede nedslag, og en
betinget matchende rapport beviser ikke disse forhold. Den uafhængige
Personskat-indgang har en særskilt delårsstatus, men tilbageholder den
uafhængige sammenligning, fordi medlemsperioder endnu ikke beregnes; se
[grænsen og de officielle kilder](personskat-validity.md#kirkeskat-gælder-indkomståret-ikke-status-i-dag).

```sh
"$RUNA_BIN" template examples/danish-income-tax/aarsopgoerelse-afstemning.calculate.runa --format json --output PRIVATE_WORK_DIR/report-cases.json
# Fill the generated cases with report observations, preserving $futuruna.
"$RUNA_BIN" call examples/danish-income-tax/aarsopgoerelse-afstemning.calculate.runa --input PRIVATE_WORK_DIR/report-cases.json --output PRIVATE_WORK_DIR/report-results.json
```

### Læs resultatet på dansk

Hvis Node.js 18 eller nyere allerede er installeret, kan du vise **dine egne
gemte resultater**, ikke kun demonstrationsdata:

```sh
node examples/danish-income-tax/afstemning-resultat.mjs PRIVATE_WORK_DIR/report-results.json
```

Øverst står et samlet overblik, så en vellykket sag ikke skjuler andre sagers
modstrid eller manglende oplysninger. Visningen viser alle returnerede
kontroller, nødvendige beløb, nedre/øvre grænser, forklaringer og forbehold.
Øre vises både eksakt og som kroner med to
decimaler; DKK-felter beholder deres enhed. `null` vises som **ukendt**, og et
ukendt loft betyder ikke ubegrænset ret. En forskel på én øre forsvinder ikke
ved visningsafrunding. Case-diagnostik vises også, hvis andre sager i samme
batch blev beregnet.

Programmet læser kun den angivne lokale resultatfil. Det ændrer ingen filer,
kontakter ingen server, bruger ingen LLM og beregner ikke skatten igen. Den
gemte kontrakthash vises som identifikation, men kontrolleres ikke mod den
aktuelle model: en visning af et gammelt resultat gør det ikke aktuelt eller
ægte. Bevar den oprindelige JSON og kildehenvisningerne. Output kan indeholde
personlige beløb; del det ikke offentligt.

Exitkode **0** betyder kun, at de viste sager er betinget afstemt. **2** betyder,
at mindst én sag er ufuldstændig, modstridende, ugyldig, ikke understøttet eller
har beregningsdiagnostik. **1** betyder, at filen eller formatet ikke kan vises;
der udskrives da ingen delvis succesrapport. Ukendte resultatfelter/statusser,
gentagne JSON-felter og ikke-heltallige beløb afvises frem for at blive skjult
eller afrundet. Ingen af exitkoderne er en skattemæssig godkendelse.

The same contract supports `--format xlsx` and XLSX invocation. The optional
income bridge remains one canonical JSON field in that format. Field metadata
explains the units, signs, and completeness questions. Generated template values
are placeholders, not facts: use `null` for unknown optional values and `false`
for unconfirmed completeness. A known zero is different from an unknown value.

Transcribe only these selected observations:

- The income bridge in **whole DKK**: personal income + net capital income −
  ordinary deductions + other known income adjustments − reported taxable
  income. The residual is the required spouse loss deduction. If any bridge
  component is unknown, leave the whole bridge `null`; the printed spouse-loss
  line may independently be `null`.
- All tax lines in **øre**, excluding the separately recorded spouse
  credits: state, municipal and church personal-allowance values, and negative
  capital-income credit. Taxes are positive; other credits are negative. Do not
  include subtotals or payments. Only mark the list complete after checking it.
- The reported spouse credits as positive **øre** amounts or `null`, plus
  the person's own reported negative-capital credit when known. Municipal and
  church allowance may be supplied as one combined observed amount (see below).
- Prepaid tax, assessed tax, and surplus tax in **øre**. Payment corrections
  are signed øre: interest/allowances positive, prior refunds/offsets negative.
  The final refund is in **whole DKK**, after rounding down.

For a report with **restskat** before corrections, select the tax-owed route
below. Do not put a negative debt amount in the surplus-tax field. An amended
report's final payment can point in the opposite direction; use the separate
[corrected-payment observation](#ændret-rapport-beløb-til-betaling-eller-udbetaling).

An explicitly reviewed empty tax section has a known sum of zero. Leave the
list empty and mark it complete only after checking the report; do not invent a
zero-valued row to make reconciliation run. An unreviewed empty list remains
incomplete. Unknown observed tax or transfer amounts still stay `null`. A
confirmed empty section can also expose a contradiction with nonzero reported
tax; it is not automatically treated as missing information.

### Fortegn og enheder følger også hver enkelt række

Den genererede kontrakt har særskilte spørgsmål og kildehenvisninger til
både navn og beløb i de tre postlister. De deler typen `RapportBeløb`, men
beløbene har forskellige roller:

- Skatteposter: skatter er positive; egne fradragsværdier og lempelser er
  negative. Ægtefællenedslagene registreres særskilt, ikke én gang til her.
- Slutskatstillæg: ikke-negative beløb **før** beregning af overskydende
  skat eller restskat; ikke beløb, der allerede indgår i beregnet skat.
- Udbetalingskorrektioner: tillæg er positive; tidligere udbetalinger og
  modregninger er negative. Ikke slutskatstillæg eller årets restskattetillæg.

`12,34 kr.` indtastes som heltallet `1234` i et ørefelt; en reduktion på
samme beløb indtastes som `-1234`. Gem originalt beløb, fortegn og lokal
side/linje i en privat kildelog. Angiv ikke en lokaliseret tekst som
`"12,34"` i heltalsfeltet, og afrund ikke øre væk. Modellen kan ikke opdage
enhver forkert, men plausibel, omregning.

Ukendt er ikke nul. Hvis en posts beløb eller placering er uafklaret, behold
spørgsmålet i kildeloggen og sæt den relevante fuldstændighedsmarkering til
`false`; opfind ikke en nulrække. Entydige postnavne beskytter mod gentagne
navne, ikke mod samme dokumentbeløb under to forskellige navne.

Metadatarettelsen ændrer kontraktens fingerprint, ikke inputtyper eller
afstemningsformler. Generér en frisk skabelon og overfør kun gennemgåede
observationer; redigér ikke et gammelt fingerprint for at omgå kontrollen.
[Regressionen](../../tests/tax_report_input_metadata.test.mjs) undersøger
den faktiske kontrakt og ti fiktive indtastninger, herunder fejl og ukendte
forhold. Den er ikke en test af AI-læsning af vilkårlige PDF'er.

Neither the spouse's municipality nor year-end tax cohabitation is mandatory.
The municipal and church transfer ceilings use the **recipient's** municipality
and church-tax status. The spouse's optional municipality does not change these
bounds. A known failure of the cohabitation condition conflicts with a nonzero
spouse transfer; unknown cohabitation remains an unverified condition.

### Kommune og kirke kan stå på samme linje

En overført personfradragsværdi kan stå som én kommunal linje med kommune- og
kirkesatsen lagt sammen. Skriv da hele den positive skatteværdi i
`ægtefællenedslag.kommunalt_og_kirkeligt_personfradrag_øre`, og lad
`kommunalt_personfradrag_øre` og `kirkeligt_personfradrag_øre` være `null`.
En samlet observation på nul er også kendt nul; den kræver stadig `null` i de
to særskilte felter. Oplys ikke samme nedslag begge steder, og opfind ikke en
fordeling ud fra skattesatserne.

Hvis rapporten faktisk viser kommune og kirke hver for sig, bruges de to
særskilte felter, mens det samlede felt er `null`. Hvis beløbene ikke kendes,
forbliver de ukendte; modellen kan stadig vise det nødvendige samlede nedslag.

Lofterne følger modtagers satser efter PSL §§ 10 og 12, fortolket sammen med
[cirkulære nr. 129 af 4. juli 1994, afsnit 11.1.1–11.1.2](https://www.retsinformation.dk/eli/mt/1994/129).
Cirkulæret beskriver omregning af ubrugte skatteværdier til fradragsbeløb hos
afsender og ny beregning med modtagers egne satser. Det er ikke dokumentation
for, hvor meget en konkret ægtefælle har til overs.

Den 25. september 2026 kontrollerede vi denne beregningsretning i
[Skattestyrelsens anonyme beregner for 2025](https://www.tastselv.skat.dk/borger/beregn2025/profil.do)
med to helt fiktive profiler: begge født 1. januar 1990 og gift hele året,
400.000 DKK løn i København, en ægtefælle i Ballerup uden indkomst og uden
kirkeskat, ingen andre indkomster, fradrag eller betalinger. Kun modtagers
kirkemedlemskab varierede.

| Oplyst beregningslinje (DKK) | Modtager uden kirkeskat | Modtager med kirkeskat |
| --- | ---: | ---: |
| Overført personfradragsværdi, stat | 6.197,16 | 6.197,16 |
| Overført personfradragsværdi, kommune og kirke samlet | 12.126,00 | 12.538,80 |
| Anvendt lokal sats | 23,50 % | 24,30 % |
| Beregnet skat / restskat før procenttillæg | 113.786,98 | 115.488,58 |

Beregneren brugte således Københavns sats, ikke Ballerups, og gav også
modtagers kirkelige fradragsværdi, når afsender ikke var medlem. De præcise
rapportlinjer er bevaret som syntetiske regressioner i
[`tests/tax_report_reconciliation.rs`](../../tests/tax_report_reconciliation.rs).
Afstemningen matcher dem uden ægtefællens kommune som input. Dette er to
eksterne observationer for 2025, ikke en uafhængig kontrol af alle års satser,
delvist udnyttede personfradrag eller den fulde `beregn_personskat`-beregning.

**Eksisterende input:** Det nye valgfrie felt og den opdaterede metadata ændrer
kontraktens fingeraftryk. Generér en ny skabelon og overfør de gennemgåede
observationer; ret ikke blot det gamle fingeraftryk. Tidligere særskilte
kommune-/kirkebeløb kan bevares med det samlede felt `null`. Tidligere resultater
med afsenders sats som loft bør køres igen.

## Read the result

For a wholly synthetic income bridge of 400,000 − 10,000 − 45,000 − 333,000,
the necessary spouse loss deduction is **12,000 DKK**. That is the transferable
remainder required by this bridge, not the spouse's original loss or income.

If complete other tax lines sum to 102,000 DKK and reported assessed tax is
98,200 DKK, the four excluded spouse credits must total **3,800 DKK**. Their
individual allocation remains unknown unless separately observed. Missing
printed transfer lines do not prevent outputting this required sum.

### Når kun nogle overførselslinjer er kendt

Modellen viser også **Nødvendig sum af ikke-oplyste ægtefællenedslag**:
det nødvendige samlede nedslag minus de faktisk oplyste overførsler. Hvis kun
én af de fire poster mangler, er dette det beløb, netop den post skulle have.
Hvis flere mangler, er deres fordeling fortsat ukendt. Resultatet indsættes
aldrig som en observation; sammenligningen forbliver ufuldstændig, medmindre
der allerede er en modstrid.

Et fiktivt eksempel: Rapportens øvrige skatteposter kræver samlet 1.799,99 DKK
i overførte nedslag. De kendte nedslag er 1.000 DKK fra staten, 0 DKK fra
kirken og 800 DKK for negativ kapitalindkomst. Det kommunale nedslag er ukendt.
Det skulle derfor være −0,01 DKK. Det er en **Modstrid**, ikke blot manglende
oplysninger: et ikke-negativt overført nedslag kan ikke forklare forskellen.

Den øvre grænse summerer kun lofterne for de manglende poster. Kommune- og
kirkeloftet er kendt fra modtagers oplysninger, også uden ægtefællens kommune.
Et kendt samlet kommune-/kirkebeløb tæller én gang; de to særskilte `null`-felter
er da ikke manglende poster. Hvis kun det statslige beløb mangler, gælder kun
det statslige loft for residualet. Ingen residual
udledes, hvis de øvrige skatteposter ikke er bekræftet komplette eller den
beregnede skat er ukendt. Kendt nul og ukendt holdes adskilt.

Dette er nødvendige regnebetingelser for de eksisterende overførselsregler,
ikke nye fradrag eller dokumentation for ægtefællens forhold.
[Skattestyrelsens vejledning om ægtefællers skatteberegning](https://info.skat.dk/data.aspx?oid=1976883).

### Status and checked bounds

The output distinguishes:

- `BetingetAfstemt`: the supplied comparisons agree and no checked bound is
  violated. Unverified legal conditions remain listed under `uafklaret`.
- `Modstrid`: an arithmetic comparison or checked necessary condition fails.
- `Ufuldstændig`: comparisons cannot be completed; available necessary amounts
  are still returned. Missing values are not silently zeroed.
- `UgyldigtRapportinput`: invalid numeric bounds, duplicate/empty post names,
  negative ordinary deductions, or overlapping combined/separate local credits
  prevented reconciliation.
- `IkkeUnderstøttetÅrEllerKommune`: no supported parameter coverage.

Each check shows expected and observed amounts, unit, and **observed minus
expected** difference. Each condition shows the necessary amount, checked lower
and optional upper bound, and its explanation. An absent upper bound is
**unverified**, not unlimited entitlement. The output always sets
`uafhængig_skatteberegning_udført` to `false`. Domain statuses are result data;
successful CLI execution alone does not mean the report passed.

The allowance ceilings use the encoded PSL §§ 9–10 parameters. The capital
credit constraints use the shared PSL § 11 rate and household limit, less the
person's own reported credit when known. The inferred capital basis is a lower
bound, not a unique reconstruction of interest expenses. Loss transfer follows
the PSL § 13 income-capacity constraint; eligibility, prior own offsets and
cross-border adjustments remain unverified.
[Personskatteloven](https://www.retsinformation.dk/eli/lta/2021/1284).

An amended assessment's new surplus is not its additional refund: subtract any
reported earlier refund before comparing the whole-krone payout. This mode
checks those reported amounts, not the independent legality of interest rates,
offsets or payment dates. The legacy refund route leaves negative post-correction
settlements incomplete; it does not silently turn them into a debt calculation.
Use the explicit final-payment observation below to check that separate balance.
[Kildeskatteloven §§ 60, 62 and 62 A](https://www.retsinformation.dk/eli/lta/2024/460).

### Ændret rapport: beløb til betaling eller udbetaling

Årets overskydende skat er ikke nødvendigvis en ny udbetaling. En tidligere
udbetaling kan betyde, at personen skal betale tilbage. Omvendt kan en nedsat
restskat give en udbetaling, hvis den tidligere restskat allerede er betalt.
KSL § 62 A, stk. 2–4, skelner disse situationer. Afstemningen kontrollerer
**rapportens oplyste regnestykke**, ikke selvstændigt beløbenes retmæssighed.
[Officiel lovtekst](https://www.retsinformation.dk/eli/lta/2024/460/pdf).

Det valgfrie `betaling.slutbetaling` adskiller den samlede korrigerede saldo
fra årets restskat/overskydende skat. Tre rent fiktive eksempler uden andre
korrektioner:

| Årets beløb før korrektioner | Oplyst tidligere bevægelse | Korrigeret saldo |
| --- | --- | --- |
| Overskydende skat 2.000 kr. | Allerede udbetalt 5.000 kr. | 3.000 kr. til betaling |
| Overskydende skat 2.000 kr. | Allerede udbetalt 1.000 kr. | 1.000 kr. til udbetaling |
| Restskat 2.000 kr. | Allerede betalt 5.000 kr. | 3.000 kr. til udbetaling |

I første eksempel bliver de relevante felter inde i `betaling`:

```json
{
  "oplyst_overskydende_skat_øre": 200000,
  "restskat": null,
  "korrektioner_til_udbetaling": [
    {"navn": "Fiktiv tidligere udbetalt, side 2", "beløb_øre": -500000}
  ],
  "korrektioner_komplette": true,
  "oplyst_udbetaling_kroner": null,
  "slutbetaling": {
    "retning": {"$variant": "TilBetaling"},
    "oplyst_beløb_kroner": 3000
  }
}
```

Dette er et udsnit, ikke et fuldt input eller personlige fakta. Forskudsskat,
beregnet skat, tillæg og øvrige rapportafsnit skal stadig oplyses særskilt.
`true` betyder her en fiktiv bekræftelse på, at alle korrektioner er med.

- Vælg årets grundbeløb som før: `restskat: null` for overskydende skat;
  ellers et objekt med `oplyst_restskat_øre`, mens overskydende skat er `null`.
  Vælg ikke dette spor ud fra den senere betalingsretning.
- Vælg slutretningen fra rapporten: `TilBetaling` eller `TilUdbetaling`.
  `oplyst_beløb_kroner` er beløbets ikke-negative størrelse i hele DKK;
  `null` betyder ukendt og giver ikke en fuldført sammenligning. Ukendt
  retning skal afklares, ikke gættes fra modellens resultat.
- Korrektionernes fortegn følger altid saldoen **til personens fordel**:
  godtgørelser og særskilte tidligere indbetalinger er positive; tidligere
  udbetalinger, modregninger og oplyste skyldige renter/procenttillæg er
  negative. Fortegnet skifter ikke, fordi personen nu skal betale.
- Medtag kun særskilte observerede poster til samme samlede saldo. Ingen
  subtotaler eller genbrug af beløb, der allerede indgår i forskudsskat,
  beregnet skat eller tillæg. Ved ukendt fuldstændighed bruges `false`;
  afstemningen tilbageholder den korrigerede saldo.
- Med `slutbetaling` skal det ældre `oplyst_udbetaling_kroner` være `null`.
  Begge felter samtidig afvises, også ved nul. Uden `slutbetaling` bevares
  de hidtidige, snævrere kontroller.

Output viser årets beløb, den eksakte korrigerede øresaldo i den valgte
retning og sammenligningen i hele kroner. Retningen kontrolleres **før**
ørebeløbet omregnes til hele kroner; en forkert retning ved én øre kan
derfor ikke forsvinde ved afrunding. Kendt nul kan passe til begge retninger.
Manglende observationer er fortsat ukendte, selv når det nødvendige beløb
kan beregnes.

Sammenlign ikke denne samlede saldo med en enkelt rate eller et betalingskort.
KSL § 62 C har særskilte opkrævningsgrænser, som denne rapportkontrol ikke
beregner. Den beregner heller ikke renter, tilbageført godtgørelse, modregning,
frister eller betalingsplaner selv. `BetingetAfstemt` er derfor **ikke et krav
om betaling nu** eller en godkendelse af udbetaling. Særregler og udeladte
korrektioner skal afklares fra kilden; opfind ikke en udligningspost.

Spørgsmål, fortegn, enheder og begrænsninger følger med den genererede
kontrakt. `runa meta --json` forbinder desuden ankret
`rapport_korrigeret_betaling` med kildehenvisningen, fortolkningsgrænsen og
de konkrete beregningsregler. Den
[fokuserede regression](../../tests/tax_report_payment.test.mjs) kontrollerer
både disse metadata og faktisk output; den er ikke en test af vilkårlig
AI-læsning af PDF'er eller en uafhængig administrativ konformitetskontrol.

**Eksisterende input:** Generér frisk schema/skabelon, og overfør gennemgåede
observationer uden at ændre dem. Feltet og metadata ændrer fingeraftrykket.
JSON uden det nye valgfrie felt læses som `null` i den friske kontrakt.
Håndskrevne `.runa`-konstruktioner af `RapportBetaling` skal tilføje
`slutbetaling = None` for at bevare det hidtidige spor. Outputtypen er uændret.

## Additions before either refund or tax owed

Both routes use the same balance: **prepaid tax minus assessed tax and its
separately reported additions**. Under KSL §§ 61(1) and 62(1), those additions
include carried restskat and the specified PBL § 25 A(5)–(9) charges. They can
reduce a refund, eliminate it, or leave tax owed. This is distinct from offsets
made *after* calculating overskydende skat.
[Skattestyrelsen, A.B.4.1.2.2.3](https://info.skat.dk/data.aspx?oid=2169091),
checked September 25, 2026.

Record these positive, uniquely named øre amounts in
`betaling.tillæg_til_slutskat`. Set
`betaling.tillæg_til_slutskat_komplette` to `true` only after reviewing that
section, including when there are no additions. An empty list with `false`
means unknown, not zero: neither settlement nor payout is calculated. The
generated template starts with `false`.

For a fictional report with 98,200 DKK assessed tax, 99,000 DKK prepaid tax and
a separately reported 500 DKK carried tax amount, the surplus is **300 DKK**,
not 800 DKK. A further, separately reported 100 DKK pension charge would leave
200 DKK. A reported payout correction of +12.34 DKK then gives 212 DKK after
whole-krone truncation. This illustrates arithmetic, not entitlement to those
charges or that correction. Do not invent a balancing charge from a difference.

Keep assessed tax unchanged in both `skat` and `betaling`. Do not enter an
addition again if it is already included in assessed tax, and do not put the
same addition in `korrektioner_til_udbetaling`. Those corrections cover later
reported movements such as a previous refund, not additions that determine the
initial surplus.

## Reports with tax owed (restskat)

Set `betaling.restskat` to an object instead of `null` to reconcile the
**underlying restskat before interest and the percentage addition**. For example,
with reported assessed tax of 98,200 DKK and prepaid tax of 90,000 DKK:

```json
{
  "restskat": {"oplyst_restskat_øre": 820000},
  "tillæg_til_slutskat": [],
  "tillæg_til_slutskat_komplette": true
}
```

These are fields inside `betaling`, not a complete input document.
It is a synthetic example. A complete empty addition list is a positive
confirmation that there are no additions, not a default for missing facts.
Use `false` when completeness is unknown; use `null` for an unknown reported
restskat amount. The expected amount remains visible when the underlying
figures are known, even if the reported restskat line is missing.

The check is:

`reported assessed tax + reported additions to that tax − prepaid tax`

Under KSL § 61(1), additions at this stage include carried restskat and the
specified pension tax. Record their positive øre amounts with unique names in
`betaling.tillæg_til_slutskat` **only when they are separate from the reported assessed
tax**. For instance, a separately reported 500 DKK carried amount gives a
principal of 8,700 DKK in this example. Do not double-count an included amount.
Keep the same current assessed-tax amount in `skat` and `betaling`; the existing
cross-section check still applies. These are report observations, not verified
legal classifications or permissions to add arbitrary balancing amounts.

Interest and percentage additions **on this year's restskat** do not belong in
that list. Neither do instalments, a prior refund, or a payment made after the
assessment. This principal-only route does not model those collection/reassessment movements.
The optional `slutbetaling` observation can separately reconcile a reported
corrected balance without independently calculating those legal adjustments.
Read the report's separately labelled principal line rather than substituting
the amount on a payment slip. For an amended assessment, this is the new total
principal, not necessarily the change from the preceding assessment.

While this route is selected, keep `oplyst_overskydende_skat_øre` and
`oplyst_udbetaling_kroner` as `null`. Keep `korrektioner_til_udbetaling` empty
unless the explicit `slutbetaling` comparison is also selected.
These are inactive fields, not assumed zero observations. Supplying both routes
is rejected rather than silently dropping a payment observation. A known
negative principal contradicts the selected debt route, including when the
reported restskat is unknown. Zero is allowed for a reconciled zero balance.

`BetingetAfstemt` means the **selected** arithmetic checks agree. Without
`slutbetaling`, only the annual principal is compared in this route. With it,
reported corrections and the final balance are also compared arithmetically;
their legal basis is not independently verified. Neither choice calculates
an amount to collect now, interest, rates, due dates or minimum collection
amounts. The result keeps this limitation explicit.
The distinction follows [KSL § 61(1)–(2), § 62 and § 62 C](https://www.retsinformation.dk/eli/lta/2024/460/pdf),
checked against the official law text September 22, 2026. No new annual
interest rates or payment schedules are inferred.

### Existing templates

The shared-additions correction moves `tillæg_til_slutskat` and
`tillæg_til_slutskat_komplette` from `betaling.restskat` directly into
`betaling`; `restskat` now contains only `oplyst_restskat_øre`. This is a
Preview source/input-contract change. Regenerate the template and transfer
reviewed observations, keeping any unknown completeness as `false`. For older
refund inputs, review the additions before choosing `[]` and `true`; their
absence from the old contract did not establish that there were none. Update
authored `RapportBetaling` and `RapportRestskat` constructors as well. Do not
change stored schema hashes to bypass migration. Recalculate saved results;
the old refund check did not account for these additions.

The partial-transfer check adds a named necessary condition, not a new input
field. Consumers should find conditions by name, not assume a fixed list length
or position. Recalculate saved results: some previously incomplete reports now
show a contradiction already implied by their known transfers. A missing line
remains missing even when its necessary amount can be determined.

The confirmed-empty correction updates field-help metadata, which is included
in the contract fingerprint. If an older envelope or workbook is rejected,
regenerate a template and carry over reviewed observations; do not edit the
stored hash to bypass the check. Recalculate affected cases: historical empty
sections may have been reported as incomplete even when zero or a contradiction
could be established.

The optional `betaling.restskat` field changes the Preview model's contract
fingerprint. Regenerate the template and migrate supported observations; never
edit an old workbook's hidden fingerprint. `null` preserves the refund route,
including the earlier-refund correction. Existing JSON records may omit the
optional field after migration to the current envelope. Saved results are
historical evidence, not automatically recalculated with the new contract.

The focused regression checks both routes, including the one-øre boundary
where additions turn a refund into debt, missing amounts/additions,
mixed-route rejection and separate payout corrections:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test --quiet --test tax_report_reconciliation
```

Coverage is 2023–2026 and the supported municipal parameter table. The mode
does not independently recompute individual tax lines, resolve other spouse
mechanisms, or prove a complete household solution. Use `runa call` for typed
report inputs. The compact report model also passes native `runa check`;
`tests/tax_report_native_test.runa` compares complete interpreted and native
outputs for 28 fictional reports (21 existing reports and seven revised-payment
cases), including partial-transfer residual bounds,
missing observations, one-øre
contradictions, spouse transfers, both settlement routes and unsupported years.
This is not a native-coverage claim for the larger `personskat.calculate.runa`
model. Native invariants also cover shared additions, unknown completeness,
the refund/debt boundary, both corrected payment directions, one-øre direction
contradictions and rejected overlapping observations.

### Parameter lookup coverage

The shared national, municipal and PSL § 11 parameter helpers expose
`*_opslag` rules returning `Some(value)` or `None`. `None` means the encoded
source does not cover that input; it is never a zero rate or a neighbouring
year's value. In particular, the 2023 income-tax table does not provide the
2024–2026 property-tax table's parameters.

Existing value rules retain their names and supported results; ordinary callers
need no source migration. The source-backed guarded clauses now live in the
optional lookup families. Strict value rules extract a present value and stop
with the checked `head: empty list` runtime failure on absence, never a
replacement value. They do not prove all inputs valid. Code
accepting an uncertain year should match the optional lookup first. The report
boundary keeps its `IkkeUnderstøttetÅrEllerKommune` result, and the canonical
calculation keeps its separate actionable unsupported-year diagnostic.

No tax rates, source quotations, provenance, input schema or compiler rule-
totality policy changed in this repair. The focused parameter regression
checks every supported municipality/year, preserves historical PSL § 11
rates, and tests both execution modes' refusal to invent missing parameters:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test --quiet --test tax_parameter_domain
```
