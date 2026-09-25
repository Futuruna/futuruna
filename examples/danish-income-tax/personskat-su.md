# SU og studiejob: kontrollér indkomsten uden at gøre SU til løn

SU-stipendium er personlig indkomst, men ikke løn med AM-bidrag. Det giver
heller ikke i sig selv beskæftigelses- eller jobfradrag. SU-lån er lån, ikke
skattepligtig indkomst. De to beløb skal derfor stå hver for sig — også når
de går ind på samme bankkonto. Se [SU's skattevejledning](https://www.su.dk/su/naar-du-faar-su/skat),
[personlig indkomst](https://info.skat.dk/data.aspx?oid=2061678) og
[låns behandling efter SL § 5, litra c](https://info.skat.dk/data.aspx?oid=1924995).
Det nærmere grundlag for skattepligt og AM-fritagelse fremgår af
[Den juridiske vejlednings SU-afsnit](https://info.skat.dk/data.aspx?oid=1976822);
[arbejdsfradragene](https://skat.dk/borger/fradrag/arbejdsrelaterede-fradrag/beskaeftigelses-og-jobfradrag)
bygger på løn og virksomhedsindkomst, ikke SU-stipendiet.

Futurunas afgrænsede SU-model dækker dokumenterede ordinære danske SU-poster
for 2023–2026. Den beregner **ikke**, hvor meget SU du har ret til, dit
SU-fribeløb, lånerenter eller et tilbagebetalingskrav. Udlandsforhold,
tilbagebetalinger og omperiodisering tilbageholdes som uden for denne model.
Det er en Preview-beregningskontrakt og forskningssoftware, ikke individuel
skatterådgivning.

## Start med en lille beregning

Brug først [kontrollen af beregningsprogrammet](../../website/public/ai-setup.md#tax-audit-runtime-check).
Fra projektmappen:

```sh
./target/release/runa template examples/danish-income-tax/personskat-su.calculate.runa \
  --format json --output /tmp/futuruna-su.json
```

Bevar `$futuruna`-delen i skabelonen. Erstat kun `cases[0].input` med disse
**fiktive** oplysninger:

```json
{
  "identifikation": "fiktivt-stipendium",
  "indkomstår": 2025,
  "kildereference": "fiktiv-SU-meddelelse",
  "art": {"$variant": "SuStipendiumEfterDanskSuLov"},
  "beløb_før_skat_kroner": 80000,
  "uden_udlandsforhold_og_korrektioner": true
}
```

```sh
FUTURUNA_CALCULATION_JOBS=1 ./target/release/runa call \
  examples/danish-income-tax/personskat-su.calculate.runa \
  --input /tmp/futuruna-su.json
```

Resultatet skal vise `input_gyldigt: true`, personlig indkomst uden AM på
80.000 kr. og nul i kørselsfradragets aftrapningsgrundlag. Det er en
indkomstklassifikation, ikke en beregnet skat. Vælges i stedet
`SuLånEfterDanskSuLov`, står beløbet som `låneudbetaling_uden_for_indkomst_kroner`,
mens personlig indkomst er nul. Uafklaret art eller afgrænsning er ugyldigt,
ikke et bevis for nul indkomst.

## Fra dine bilag til Personskat

Brug årets dokumenterede stipendium **før A-skat**, ikke månedsbeløbet eller
nettobeløbet på kontoen. Dokumenterede skattepligtige SU-tillæg kan indgå som
stipendium. En efterbetaling vedrørende tidligere år kræver særskilt
periodisering og ligger uden for denne korte indgang; se også
[handicaptillæggets skatteforhold](https://www.su.dk/handicaptillaeg/udbetaling-af-handicaptillaeg).
SU har ikke ATP; eventuel ATP fra studiejobbet oplyses særskilt som lønrelateret
ATP. Andre stipendier, skoleydelse og lønnet ph.d.-arbejde er ikke automatisk SU.

SU-lånsposten er årets dokumenterede låneudbetaling, ikke samlet restgæld,
afdrag eller renter. Renter må ikke tastes som negativ SU. Denne model
fastslår ikke rentefradrag eller ændrer gælden.

I den samlede Personskat-skabelon vælges `PersonskatUddannelsesstøtte` i
`lønmodtager.personlig_indkomst.ordinære_forhold.forenings_og_arbejdsløshedsydelser`.
Samlingens danske etiket nævner nu SU; maskinstien er bevaret af hensyn til
eksisterende klienter. Variantens `fakta` er objektet ovenfor. Ægtefællen har
samme indgang. Generér en frisk skabelon, og overfør gennemgåede oplysninger;
den nye variant ændrer kontraktens fingeraftryk.

Opret forskellige poster for stipendium og lån med deres egne identiteter og
kildehenvisninger. Samme betaling må ikke stå som både SU og løn eller som
to ydelser. Modellen kontrollerer dublerede identiteter i ydelseslisten, men
kan ikke bevise, at forskellige identiteter er forskellige betalinger.

Løn fra studiejobbet står fortsat under løn og kan give arbejdsfradrag. SU
udelades fra indkomstgrundlaget for det ekstra kørselsfradrag efter
[LL § 9 C, stk. 4](https://info.skat.dk/data.aspx?oid=2061745). Det giver ikke
automatisk ret til fradrag for ture til uddannelsen: studentergrenen fra 2026
har særskilte krav til bl.a. bopæl og SU-modtagelse. Indkomstposten her beviser
ikke, at de krav er opfyldt.

Læs altid `vurdering` før slutskatten. Manglende kilde, forkert år, negativt
beløb, ukendt art, dubletter eller uafklaret afgrænsning kan tilbageholde det
samlede sammenligningsbeløb. En tom liste er heller ikke dokumentation for,
at du ikke modtog SU. Et menneske eller en assistent indtaster kildefakta;
Futurunas regler og heltalsaritmetik beregner resultatet.

## Uafhængige kontroltal fra årsberegneren

Den 25. september 2026 blev to fiktive profiler aflæst i
[SKATs anonyme årsberegner for 2025](https://www.tastselv.skat.dk/borger/beregn2025/profil.do).
Begge er ugifte, født 01.01.1990, fuldt skattepligtige hele året, i København,
uden kirkeskat, virksomhed, bolig, pension, ATP, andre indkomster eller
fradrag. Det er eksplicitte fiktive fravær, ikke standardværdier for en borger.
Løn står i rubrik 11, stipendium i rubrik 16A. Ingen login eller private
oplysninger blev brugt. Der blev ikke registreret en versionsbetegnelse.

| Løn, kr. | SU-stipendium, kr. | AM, kr. | Beskæftigelsesfradrag, kr. | Personlig indkomst, kr. | Skattepligtig indkomst, kr. | Kommuneskat før personfradrag, kr. | Beregnet skat inkl. AM, kr. |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 80.000 | 0 | 0 | 80.000 | 80.000 | 18.800,00 | 10.084,84 |
| 100.000 | 80.000 | 8.000 | 12.300 | 172.000 | 159.700 | 37.529,50 | 47.863,54 |

Den permanente [integrationstest](../../tests/personskat_validity.rs),
`su_grants_and_loans_reach_canonical_tax_without_wage_deductions`, bruger
disse aflæste beløb. Dens yderligere SU-låne-, ægtefælle-, kørsels- og
ugyldighedscases er modelregressioner, ikke flere eksterne observationer.
Kontrollen er før grøn check, forudbetalinger og slutafregning; den beviser
ikke restskat eller tilbagebetaling. Kendte afrundingsforbehold og øvrige
[modelgrænser](personskat-validity.md) gælder fortsat.
