# Read the calculation status before comparing tax totals

## Læs dit gemte resultat på dansk

Når du har udfyldt kildefakta og kørt den kanoniske beregning med den compiler,
der bestod [runtime-tjekket](../../website/public/ai-setup.md#tax-audit-runtime-check),
kan du læse resultatet lokalt. Kræver allerede installeret Node.js 18 eller nyere:

```sh
# Gem personlige filer uden for projektmappen.
"$RUNA_BIN" call examples/danish-income-tax/personskat.calculate.runa --input PRIVATE_WORK_DIR/cases.json --output PRIVATE_WORK_DIR/results.json
node examples/danish-income-tax/personskat-resultat.mjs PRIVATE_WORK_DIR/results.json
```

Visningen starter med et overblik over **alle** sager. Ugyldige sager viser
fejlede inputstier og forklaringer, men ingen diagnostiske beløb som brugbar
skat. Manglende beløb bliver aldrig til nul. Sager, der slet ikke kunne
beregnes, beholder deres diagnostik, også når andre sager lykkedes.

For beregnede sager vises modellens tilladte sammenligningsbeløb i eksakte øre
og kroner med to decimaler samt udvalgte indkomst- og fradragsbeløb i DKK.
Delbeløbene kan overlappe; de skal ikke summeres. **Slutskat er ikke restskat
eller en udbetaling**, og visningen sammenligner ikke med et observeret beløb
i årsopgørelsen. Alle kontroller og forbehold fra `vurdering` bevares.
Ved afstemning af foreløbige skatter skal der også skelnes mellem indeholdte,
pålignede og faktisk betalte beløb; se [kreditvejledningen](personskat-skattekreditter.md).
Et gyldigt positivt tal er ikke i sig selv en korrekt klassificeret kredit.
Negative rå kreditbeløb eller en negativ § 55-tilbagebetaling tilbageholder
sammenligningen med kontrollen `årsopgørelse.kreditter`; fortegn omskrives ikke.
Kontrollernes forklaringer er modeltekster, som også kan beskrive et
fejlscenarie ved en bestået kontrol. Læs kontrollens status som dens udfald.
En aktiv ægtefælle indgår i modelkontrollerne, men ægtefællens egne tal,
betalingsafregningen og den fulde detailberegning findes fortsat i JSON.

Programmet er en læsevisning: ingen netværk, filændringer, LLM eller ny
skatteberegning. Det kontrollerer den understøttede ydre resultatstruktur,
vurderingens interne sammenhæng og de viste beløb, ikke alle underfelter i den
fulde beregning. Ukendte ydre felter, vurderingsfelter og statusser afvises,
ligesom gentagne JSON-felter og beløb, der ikke er eksakte heltal.
Kontrakthashen vises, men kontrolleres ikke mod den aktuelle model. Et gammelt
eller ændret resultat bliver ikke aktuelt eller autentisk ved at blive vist.
Bevar original JSON og kildehenvisninger; output kan være personfølsomt.

Exitkode **0** betyder kun *beregnet med forbehold*, **2** betyder mindst én
ugyldig sag eller beregningsdiagnostik, og **1** betyder afvist fil/format.
Ved kode 1 vises ingen delvis succesrapport. Ingen kode godkender skatteforhold.
Filer over 16 MiB afvises; beregn mindre batches frem for at slette forbehold.
Denne visning understøtter `beregn_personskat` og `beregn_personskat_delår`,
ikke den separate
[grøn-check-beregning](personskat-groen-check.md) eller
[betingede rapportafstemning](aarsopgoerelse-afstemning.md#læs-resultatet-på-dansk).
For den særskilte [delårsberegning](personskat-delaar.md) bruger visningen
den yderste `vurdering` og `slutskat_efter_par14_øre`. Den viser udvalgte
periode-/årsgrundlag særskilt og aldrig indlejrede skattetotaler som slutskat.
`delårsresultat.vurdering` er et mellemtrin, ikke den endelige delårskonklusion;
en gyldig mellemregning kan ikke ophæve en fejl i den yderste vurdering.
En afregningsfejl i en mellemregning erstatter heller ikke den endelige
afregningskontrol. De fulde mellemregninger findes fortsat i JSON.
Den almindelige indgang
modtager ikke skattepligtsperiodens datoer; afklar derfor helår/delår før
valg af beregningsvej.

## Folkepensionsalder og ejendomsskat

Ved årets ejendomsværdiskat skal ejerens og en aktuel samlevende ægtefælles
folkepensionsstatus stemme med deres respektive
`lønmodtager.pension.fødselsdato`. Modellen udleder datoen efter
[socialpensionslovens § 1 a](https://www.retsinformation.dk/eli/lta/2024/1123)
med den [daterede ændring](https://www.retsinformation.dk/eli/lta/2025/703),
og kontrollerer, om alderen er nået ved indkomstårets udgang som krævet af
[ejendomsskattelovens § 25](https://www.retsinformation.dk/eli/lta/2023/678).
Det er ikke nødvendigvis 67 år, og datoen er ikke første pensionsudbetaling.

En forkert status **eller dato** tilbageholder sammenligningen. Fejlen peger på
`ejendomsskatter.person.ejer_folkepensionsalder` eller
`samlevende_ægtefælles_folkepensionsalder`, eventuelt under
`ægtefælle.MedÆgtefælle.fakta`, og viser forventet status og afledt dato.
Kontrollér den oprindelige kilde ved uenighed; ændr ikke fødselsdatoen for at
få et ønsket nedslag. Den afledte forventning er betinget af den oplyste
fødselsdato, ikke en bekræftelse af dokumentets sandhed. Rå diagnostiske
beløb bevares, men må ikke præsenteres som gyldig slutskat.

Ægtefællens alder kan være relevant, selv om denne ikke ejer nogen ejendom.
I ægtefællens ejendomsgren henviser partnerfeltet omvendt til hovedpersonen.
Årets kontrol kræver ikke ejendomsaldersfakta ved tom ejendomsliste eller alene
grundskyld, og et ikke-samlevende partnerfelt bruges ikke som nedslagsgrundlag.

Et oplyst **eget rabatgrundlag fra 2024** vedrører samme ejer. Her afstemmes
`kontekst_2024.kildefakta.ejer_folkepensionsalder` mod ejerens fødselsdato for
**2024**, også i en senere årsberegning. Hvis folkepensionsalderen først nås i
2025, må denne status ikke kopieres tilbage til 2024. Dette gælder begge
personers egne ejendomsgrene og også et historisk ejendomsværdiskattegrundlag
for en bolig, som nu udlejes. Fejlen peger på det valgfrie JSON-felt
`eget_rabatgrundlag_2024`; forklaringen navngiver ejendommens identifikation,
det indre aldersfelt og den forventede status/dato. Ret kildegrundlaget, ikke
den afledte skat.

Historien kan påvirke overgangsrabatten efter
[ESL §§ 35-40](https://www.retsinformation.dk/eli/lta/2023/678), herunder
den tidligere ejendomsværdiskats stigningsbegrænsning efter
[EVSL §§ 9 og 9 a](https://www.retsinformation.dk/eli/lta/2020/1590).
Kontrollen bekræfter kun sammenhængen i aldersoplysningen; den verificerer
ikke historiske indkomster, vurderinger eller alle § 9-kvalifikationer.

En historisk ægtefælle, et **overtaget** rabatgrundlag og pensionistsuccession
efter § 25, stk. 3, har stadig selvstændige personfakta. Disse personidentiteter
udledes ikke af den aktuelle husstand. En tidligere ægtefælles alder må derfor
ikke erstattes med ejerens eller en ny ægtefælles alder. Brug den dokumenterede
overdragelsesrute, hvis grundlaget faktisk stammer fra en anden person; flyt
ikke et eget grundlag til denne rute for at undgå en kontrol. En yngre
længstlevende kan fortsat have et særskilt dokumenteret successionsgrundlag.

## Historiske ydelser og stigningsbegrænsning

Et 2024-rabatgrundlag kan kræve andre personfakta end folkepensionsalder.
Den tidligere [EVSL § 9, stk. 1](https://www.retsinformation.dk/eli/lta/2020/1590)
omfatter også visse ydelsesmodtagere og deres samlevende ægtefæller.
[Lov nr. 2202/2020 § 11](https://www.retsinformation.dk/eli/lta/2020/2202)
tilføjede tidlig pension. Dette ændrer valget mellem §§ 9 og 9 a i den
historiske sammenligning, **ikke** retten til aldersnedslag efter § 8 eller
den nuværende ESL § 25.

Under `tidligere_ejendomsværdiskat.historisk_begrænsning` findes
`par9_ydelsesgrundlag`. Det er et valgfrit objekt med `ejer` og `ægtefælle`:

- `null` for hele objektet eller `EjskEvslPar9YdelseUoplyst` for en person
  betyder **ukendt**, ikke ingen ydelse.
- `EjskEvslIngenPar9Ydelse` kræver afklaret fravær af de relevante ydelser.
- `EjskEvslEfterlønVedÅretsUdgang` og `EjskEvslFleksydelseVedÅretsUdgang`
  vedrører modtagelse ved udgangen af **2024**, efter de respektive love.
- `EjskEvslModtagerSocialYdelse` indeholder `ydelse` og `fødselsdato`.
  Ydelsen er `EjskEvslFørtidspension`, `EjskEvslSeniorpension`,
  `EjskEvslTidligPension` eller
  `EjskEvslInvaliditetsydelseMedBistandsEllerPlejetillæg` efter § 9, stk. 1,
  nr. 2. Førtids-, senior- og tidlig pension omfatter også lovens forskud.
  Modellen kontrollerer en gyldig dato og mindst 60 år ved udgangen af 2024.
  Den efterprøver ikke myndighedens tilkendelse af selve ydelsen.

En ukendt relevant person tilbageholder sammenligningen, hvis valget mellem
lofterne ikke allerede er afgjort af en kendt kvalifikation. Ejerens kendte
efterløn kan fx være tilstrækkelig uden partnerens ydelsesoplysninger.
En allerede tilstrækkelig folkepensionskvalifikation kræver heller ikke
yderligere ydelsesfakta. En ikke-samlevende historisk partners ydelse bruges
ikke. Historisk partner og et overtaget rabatgrundlag kan vedrøre andre
personer end den aktuelle husstand; kopier ikke aktuelle fakta bagud i tiden.
I eget grundlag kontrolleres ejerens supplerende fødselsdato også mod
`lønmodtager.pension.fødselsdato`.

Oplysningerne behøves ikke, når intet historisk loft anvendes. Men manglende
2023-sammenligningsbeløb må **ikke** omfortolkes til, at loftet ikke gælder:
afklar beløbet og dets lovbestemte opgørelsesgrundlag i kildematerialet.
Den eksisterende indgang modtager dette grundlag; den rekonstruerer ikke hele
2023-beregningen eller alle særlige tilpasninger i §§ 9 og 9 a.

Fiktivt eksempel: ejer under folkepensionsalderen med dokumenteret efterløn
ved udgangen af 2024, tidligere sammenligningsskat 4.000 kr., beregnet gammel
skat før loft 7.820 kr. og ny sammenligningsskat 8.160 kr. § 9 giver 4.500 kr.
og dermed 3.660 kr. i rabat. Ingen pensionsaldersdato ændres for at opnå
nedslaget. Det er et kildebaseret modeleksempel, ikke et eksternt match
eller en konstateret fejl i en virkelig årsopgørelse.

Historisk længstlevendesuccession har et selvstændigt kildegrundlag, som
beskrevet nedenfor. Den må ikke udledes af nutidige pensionsaldersfakta.

## Historisk længstlevendesuccession

`tidligere_ejendomsværdiskat.succession` adskiller gamle
[EVSL § 8, stk. 3, og § 9, stk. 3](https://www.retsinformation.dk/eli/lta/2020/1590)
fra `ny_lov_nedslagsfakta.pensionistsuccession` efter
[ESL § 25](https://www.retsinformation.dk/eli/lta/2023/678).
Gamle regler lader succession ophøre fra indkomståret **efter** nyt ægteskab;
den nye § 25 bruger selve ægteskabsåret. Gamle regler bruger fortsat beboelse
efter dødsfald, mens den nye ordlyd bruger rådighed og også nævner plejehjem.
Forskellen i beboelse/rådighed og ægteskabsår forklares i
[L 113's bemærkninger til § 25, trykte sider 153–155](https://www.ft.dk/ripdf/samling/20222/lovforslag/l113/20222_l113_som_fremsat.pdf).
En plejehjemsdato må ikke indtastes som en dødsdato; uafklaret anvendelse af
ældre regler eller praksis skal forblive uafklaret, ikke blive et afslag.

Vælg mellem:

- `EjskEvslSuccessionUoplyst`: det gamle grundlag er ukendt.
- `EjskEvslIngenSuccession`: afklaret fravær af gammelt successionsgrundlag,
  ikke blot manglende dokumentation eller fravær af nuværende ægtefælle.
- `EjskEvslLængstlevende`: `fakta` indeholder dødsdato og særskilte oplysninger
  om, at ægtefællerne ikke var separerede ved dødsfaldet, fortsat beboelse,
  ejendommens tilhørsforhold til en af ægtefællerne, afdødes personkreds efter
  §§ 8/9 og eventuelt nyt ægteskab.

De fem boolske oplysninger i `fakta` bruger `null` for ukendt. Nyt ægteskab
har tre alternativer: `EjskEvslNytÆgteskabUoplyst`,
`EjskEvslIntetNytÆgteskab` og `EjskEvslNytÆgteskabIndgået` med `dato`.
Oplys det første nye ægteskab efter dødsfaldet, også hvis dette ægteskab siden
er ophørt. Datoer skal være gyldige og kronologiske; et dødsfald efter 2024
kan ikke bruges i dette 2024-grundlag. Kalenderårsmodellen genberegner ikke
forskudte indkomstår.

**Afgrænsning af kildefakta:** `afdødes_par8_personkreds` og
`afdødes_par9_personkreds` er særskilt dokumenterede historiske kvalifikationer
før dødsfaldet. Denne indgang genberegner ikke afdødes alder, ydelsestildeling
eller hele tidligere skat. En udbetalt pension eller en linje med nul i nedslag
afgør ikke alene personkredsen. Brug en afklaret historisk opgørelse eller
begrundelse i den private kildelog; gæt ikke et ja for at opnå et beløb.
Kan kvalifikationen ikke fastslås, bevares `null`. Det er en betinget
beregning med dette kildegrundlag, ikke uafhængig validering af afdødes skat.

§ 8-personkreds indebærer § 9-personkreds. `true` for § 8 og `false` for § 9
afvises som modstrid; `true` for § 8 er tilstrækkeligt ved ukendt § 9.
Omvendt kan en ydelsesbaseret § 9-kvalifikation give stigningsbegrænsning uden
§ 8-aldersnedslag. En afklaret manglende nødvendig betingelse kan afgøre
succession som ikke opfyldt, selv om andre betingelser er ukendte. Hvis ejer
eller historisk samlevende ægtefælle allerede opfylder aldersbetingelsen,
behøves irrelevant viden om afdøde ikke. Ellers tilbageholdes sammenligningen,
når relevant succession er uafklaret. Kontrollen gælder også overtagne
rabatgrundlag og en beregnet ægtefælles ejendomme.

Fiktivt eksempel: yngre ejer, kvalificeret tidligere ægtefælle død i 2023,
fortsat beboelse og nyt ægteskab i 2024. Med gammelt beregningsgrundlag
850.000 kr. og tidligere sammenligningsskat 4.000 kr. giver § 9 et loft på
4.500 kr. Det er et modeleksempel, ikke en fejl konstateret i en virkelig
årsopgørelse.

En moderne succession må ikke kopieres automatisk til det historiske felt,
og ukendt må ikke blive `EjskEvslIngenSuccession`.

## Model-owned validity assessment

Finansielle indkomstposter uden for den valgte nr. 5 a/5 b-gren er ikke
automatisk skattefri. Ikke-nul beløb tilbageholder sammenligningen; afklarede
udgifter uden fradragsret behandles anderledes. Se
[vejledningen til finansielle poster](personskat-finansielle-poster.md).

Et ejendomsresultat, som ikke omfattes af PSL § 4, stk. 1, nr. 6, er heller
ikke automatisk skattefrit. Et ikke-nul beløb i denne forkerte indgang
tilbageholder års- og delårssammenligningen, også for en beregnet ægtefælle.
Se [ejendomsvejledningen](personskat-ejendomsdrift.md); ret indkomstruten ud fra
kilderne, ikke ejendommens faktiske anvendelse for at opnå et resultat.

Honorarer og anden B-indkomst kræver også den rigtige indgang. Løn og
virksomhedsindtægt indsat i honorarlisten bliver ikke automatisk flyttet
eller skattefri; den samlede sammenligning tilbageholdes. Se
[honorarvejledningen](personskat-honorar.md) om kildefakta, bruttobeløb,
udgifter og dobbelt medregning.

`beregn_personskat` returns `vurdering` alongside its existing breakdown. This
is a model-owned validity assessment, not a compiler guess based on Boolean
field names. The same checks apply to the taxpayer and an active spouse.

If `vurdering.status` is `UgyldigtBeregningsgrundlag`,
`vurdering.slutskat_til_sammenligning_øre` is `null`. Do not present any of the
other numerical fields as a reliable tax result. They remain available only to
diagnose the failed calculation. `vurdering.fejl` gives input paths and reasons;
`vurdering.kontroller` includes the passing checks too.

`vurdering.kontrolgrundlag` separates `beregning` (source and calculation
checks) from `afregning` (payment-settlement checks for this calculation stage).
The flat `kontroller` list is their ordered concatenation; the complete
assessment requires a nonempty calculation group and no failed checks in either
group. The path alone does not select a group: source-credit and settlement
checks can both have the path `årsopgørelse`.

The Danish viewer validates this grouping against the flat list and labels both
groups. It still accepts older saved assessments without groups, without making
them current or authentic. Fresh schemas/templates are required after this
output-contract change; transfer the same reviewed facts and recalculate.

For example, a structurally valid date object containing all zeroes is not a
valid birthday. It now fails the `lønmodtager.pension.fødselsdato` check even if
the downstream arithmetic happens to produce a plausible tax amount. An invalid
spouse birthday is reported against
`ægtefælle.MedÆgtefælle.fakta.lønmodtager.pension.fødselsdato`.

The birthday is required even without pension contributions: it also determines
the AM contribution rate. From 2026 the youth exemption lasts through the year
of turning 17, while the ordinary rate applies throughout the year of turning
18. The preliminary pension income basis must preserve the same actual date as
the final wage calculation. See the [Danish age-boundary example](pension-og-fradrag.md#fødselsdato-er-ikke-kun-et-pensionsfelt).

`BeregnetMedForbehold` means the listed input checks passed. Its comparison
amount is the existing `slutskat_øre`, unchanged. It does **not** establish that
the underlying documents are true, all facts were supplied, or the complete
relevant law has been encoded. `samlet_modeldækning_bekræftet` remains `false`
and `forbehold` lists the remaining qualifications, including known deduction
coverage gaps. Only compare the modeled portion until coverage is established.

A successful CLI exit means execution and serialization succeeded, not that
the domain assessment is valid. JSON and XLSX expose the same status and nullable
comparison amount. Do not replace an absent comparison amount with zero.

The ordinary entry accepts an externally supplied green-check credit; this does
not independently establish entitlement. For source-derived green check and its
settlement effect, use the additive
[Personskat with green check](personskat-groen-check.md) entry. It retains the
canonical checks and withholds the settlement when its extra facts are unknown,
unsupported or inconsistent. It is a separate entry, not a replacement for the
ordinary calculation.

## Manglende ægtefælleoplysninger er ikke ingen ægtefælle

`ægtefælle` begynder nu med `ÆgtefællegrundlagUoplyst`. Bevar dette valg,
når forholdet eller de nødvendige ægtefællefakta ikke er afklaret. Det er en
oplysningsstatus, ikke en ny civilstand. En manglende årsopgørelse betyder
hverken ingen ægtefælle, nul ægtefælleindkomst eller ingen overførsler.

Vælg `UdenÆgtefælle` ud fra afklarede forhold, ikke fordi den anden persons
dokumenter mangler. `MedÆgtefælle` kræver de relevante personfakta og
samlivsforhold for indkomståret. Skattestyrelsens
[vejledning om ægteskab og skat](https://skat.dk/borger/forskudsopgoerelse/aegteskab-skilsmisse-og-skat)
beskriver overførsler mellem ægtefæller; modellens kildeanker henviser også
til personskatteloven og ændringsloven. Oplysningskontrollen ændrer ikke
disse skatteregler eller deres beregningsformler.

Uoplyst grundlag giver en fejl ved `ægtefælle.$variant` og et `null`
sammenligningsbeløb. Øvrige beløb er kun diagnostik uden rekonstruerede
ægtefællefakta. Kontrollen bevares både i delårsberegningens periode- og
dokumenterede helårsgrundlag og i den integrerede grøn-check-beregning,
som også tilbageholder kredit og betalingsafregning.

Du kan stadig bruge den [betingede rapportgennemgang](aarsopgoerelse-afstemning.md)
uden ægtefællens årsopgørelse. Den viser, hvad udvalgte rapportbeløb kræver,
ikke hvad ægtefællens faktiske indkomst og fradrag var. Brug aldrig dens
udledte overførsler som uafhængige input til Personskat.

**Preview-migration:** Lav en frisk skabelon; typen og metadata ændrer
kontrakthashen. Bevar tidligere `UdenÆgtefælle`/`MedÆgtefælle` kun efter
kontrol mod kildefakta. Udskift ikke blot hash, og migrér ikke ukendte forhold
til fravær. Eksisterende `.runa`-matches over typen skal håndtere den nye
variant. En angivet afklaret variant dokumenterer ikke i sig selv sandhed
eller fuldstændighed; modellen kan stadig ikke opdage enhver manglende fakta.

## Which checks are combined

Payroll-funded group-life premiums and the bounded PBL19/PBL56 aggregate now
require a documented gross amount for employment/job deductions, separately
from net personal income. Unknown gross withholds comparison for taxpayer and
spouse; it is not silently treated as zero. See the
[group-life input and migration guide](personskat-gruppeliv.md).

The canonical calculation combines explicit domain summaries for dates/year and
municipality, personal income, commuting and deductions, pension contributions
and payments, capital income, self-employed AM contributions, shares, foreign
social contributions, CFC, spouse business allocation, carried losses, debt
relief, negative share-tax credits, DIS relief, property tax, foreign relief and
settlement credits/selected payment reconciliation. These reuse the component
models' active-branch validation; a false eligibility decision is not itself
invalid input. Inactive spouse and optional payment branches are not required.

These are the enumerated checks, not a claim that every possible defect in the
research model has been ruled out. The public `beregn_personskat` entry checks
the main person's and any active spouse's tax year before evaluating tax.
Years outside the current 2023–2026 boundary produce a case diagnostic naming
the field, supplied year, supported years and
[parameter source model](skatteaar-parametre.runa), with no result for that case.
Other cases in the same batch may still succeed. Do not change a correct year
merely to pass the check: that year needs its own source-backed model. This is
a model-coverage limit, not a finding about tax liability.

Other unsupported inputs that fail before a result can be constructed still
produce CLI diagnostics. Native checks now pass for the state-tax source module
and the [focused employment-deduction audit](beskaeftigelsesfradrag.md), but this
does not establish native support for the full Personskat calculation. Remaining
code-generation limitations mean this boundary is exercised through `runa call`.
The year guard uses `assert_with_message` and therefore requires a compiler built
after that builtin was added, not the original 0.2.0 binary.

Low-level year-parameter tables expose `*_opslag` helpers with `Some(...)` or
`None`. Their existing value helpers stop when a required parameter is absent;
they do not invent a zero rate. A legally phased-out rate of zero remains a
known value. A known rate does not establish an indexed threshold for an
uncovered future year. These helpers do not expand the canonical 2023–2026
input boundary, and guarded low-level calculation helpers still require their
stated preconditions.

The municipality check establishes that a rate exists, not that the supplied
municipality is legally the right one. `lønmodtager.kommune` and the active
spouse's corresponding field mean the **tax municipality for the income year**,
not necessarily current residence. The ordinary rule uses residence on
September 5 of the preceding year; exceptions need their own facts. See the
[Danish moving-house example and source](pension-og-fradrag.md#skattekommunen-er-ikke-altid-din-nuværende-bopæl).

## Lønrettelser og negative beløb

`lønmodtager.bruttoløn_kroner` er personens afklarede almindelige **årsbeløb**
med feltets pensions-/ATP-afgrænsning, ikke en enkelt lønseddel eller rettelsespost.
Den ordinære gren kræver nu et ikke-negativt beløb. En negativ værdi bevares i
diagnostikken, men tilbageholder sammenligningen med en fejl på lønfeltet.
Det gælder også en aktiv ægtefælle og de kanoniske grundlag i delårsberegningen.
Andre indkomster eller ægtefællens indkomst gør ikke et negativt løninput gyldigt.
Kontrollen gælder det oprindelige input før afledte § 25 A-løntillæg, ikke den
samlede personlige indkomst eller lovlige underskud i andre grene.

En negativ eIndkomst-post kan være **forskellen** til en tidligere indberetning.
Den er ikke i sig selv et negativt årsbeløb. Ved en afklaret rettelse bruges det
dokumenterede korrigerede årsbeløb for det rigtige år, uden at trække rettelsen
fra igen. Eksempel: oprindelig årsløn 600.000 kr. og dokumenteret nedsættelse
50.000 kr. giver 550.000 kr., hvis kilderne fastslår, at beløbene vedrører samme
år, samme løngrundlag og ingen andre ændringer. Har årsopgørelsen allerede
550.000 kr., er rettelsen ikke yderligere 50.000 kr. i fradrag.
Se [eIndkomst 14.1](https://info.skat.dk/data.aspx?oid=2386660) og
[14.4](https://info.skat.dk/data.aspx?oid=2386664).

**Tilbagebetalingsåret er ikke automatisk indkomståret.** Skattestyrelsens
[juridiske vejledning C.A.3.1.1.3](https://info.skat.dk/data.aspx?oid=1976766)
skelner mellem løn modtaget med urette, som reguleres i de oprindelige
indkomstår, og senere indtrufne omstændigheder, hvor et tab normalt hører til
året, hvor det konstateres. Derfor er et banktræk eller et negativt beløb alene
ikke tilstrækkeligt til at vælge år, fradragstype eller AM-behandling.
Dette er en afgrænsning af den ordinære inputgren, **ikke** en regel om, at
løntilbagebetaling aldrig giver fradrag. Modellen afgør ikke her
tilbagebetalingskravets retlige grundlag, genoptagelse eller særlige tabsforløb.

AI'en skal bevare det originale bilag og dets fortegn. Er det korrekte årsbeløb
eller korrektionsgrundlag uafklaret, må den hverken bruge nul, absolut værdi,
flytte beløbet til en anden indkomstgren eller tilpasse det til den forventede
skat. Brug eventuelt den [betingede rapportgennemgang](aarsopgoerelse-afstemning.md)
til at undersøge rapportens interne sammenhæng; det fastslår ikke fradragsretten.
En ikke-negativ værdi beviser omvendt ikke, at kilderne er komplette eller korrekt
periodiserede: positive årsbeløb kan også skjule en forkert nettomodregning.

Feltets type er uændret. Regenerér schema/template efter metadataændringen,
gennemgå berørte fakta og genberegn gemte resultater. Den
[fokuserede regression](../../tests/personskat_salary_input.test.mjs) adskiller
nul, positiv løn, et korrigeret årsbeløb, negativ løn med SU, ægtefælleinput og
lovlig negativ kapitalindkomst. Det er model-/inputkontrol, ikke en ny uafhængig
bekræftelse af årsopgørelsers korrekthed.

Delårsgrenen bevarer de kanoniske person-/kildekontroller fra den eksplicitte
gruppe `kontrolgrundlag.beregning` i begge grundlag, også for en aktiv ægtefælle.
Kildekontroller på `årsopgørelse` er ikke undtaget. Den ordinære afregningsgruppe
overføres ikke: den endelige afregning kontrolleres mod skatten efter § 14.
Forkert betalingsretning, forkert indkomstår eller ugyldige afregningsfakta
tilbageholder nu den yderste sammenligning. Den rå skat forbliver diagnostik,
ikke en godkendt skat eller et beløb til udbetaling. Den
[lille kontroltest](../../tests/personskat_partyear_input_controls_test.runa)
adskiller trinnene uden at fortolke kontrolstiers navne; den
[kanoniske regression](../../tests/personskat_settlement_stage.test.mjs)
kontrollerer begge beregningsgrundlag og den afsluttende afregning.
Det er modelkonsistens, ikke ny uafhængig administrativ konformitet.

## Kirkeskat gælder indkomståret, ikke status i dag

Afklar kirkeskat for **det år, der beregnes**, for både hovedperson og en aktiv
ægtefælle. En udmeldelse i 2026 siger ikke, at personen var uden kirkeskat i
2025. [Kirkeministeriets vejledning på borger.dk](https://www.borger.dk/kultur-og-fritid/medlemskab-af-folkekirken)
beskriver registreringen af udmeldelsesdatoen og ophør af kirkeskat.
[Danmarks Statistiks KISKAT-beskrivelse](https://www.dst.dk/da/Statistik/dokumentation/Times/personindkomst/kiskat)
angiver, at en udmeldt person kun betaler for medlemsdelen af året. Kilderne
er gennemgået 25. september 2026; de fastlægger ikke her den præcise
periodiserings- og afrundingsalgoritme.

`lønmodtager.kirkeskat` har nu fire udtrykkelige statusser:

| Status | Betydning og modelresultat |
| --- | --- |
| `KirkeskatUoplyst` | Ikke afklaret; sammenligningsbeløbet tilbageholdes. Dette er skabelonens startværdi, ikke ingen kirkeskat. |
| `IngenKirkeskatHeleÅret` | Afklaret ingen kirkeskat hele indkomståret; den eksisterende beregning uden kirkeskat bruges. |
| `KirkeskatHeleÅret` | Afklaret kirkeskat hele indkomståret; den eksisterende helårsberegning bruges. |
| `KirkeskatEnDelAfÅret` | Kendt delårsforhold, men periodiseringen er endnu ikke modelleret; sammenligningsbeløbet tilbageholdes. |

Ind-/udmeldelse i året må ikke omklassificeres til en helårsstatus for at
opnå det nærmeste rapportbeløb. En tilbageholdt sammenligning betyder ikke
skattefrihed eller en fejl i årsopgørelsen.

Et beløb på nul er ikke i sig selv bevis for ingen kirkeskattepligt:
indkomstgrundlag og personfradrag påvirker også beløbet. Brug ikke ændret
indkomst, skattekommune eller sats til at efterligne en medlemsperiode.
[Delårsskattepligt efter PSL § 14](personskat-delaar.md) er en anden
problemstilling og tilføjer ikke medlemsperioder til denne model.

Kontrollen ved `lønmodtager.kirkeskat` og den tilsvarende aktive ægtefællesti
tilbageholder nu sammenligningen mekanisk for uoplyst status og delårsforhold.
Status bevares ved ægtefællefordeling og i § 14-forløbet; den afledte
grøn-check-indgang tilbageholder også sin betalingsafregning. Modellen
autentificerer stadig ikke dokumenterne eller opdager en urigtigt oplyst
helårsstatus. Ved
[betinget rapportafstemning](aarsopgoerelse-afstemning.md) kan de oplyste
tal stadig undersøges, men kirkeloftet er kun en helårs-overgrænse og
godkender ikke periodens faktiske nedslag.

### Migrering fra det boolske felt

Dette er en ændring af Preview-inputkontrakten: `betaler_kirkeskat` er
erstattet af `kirkeskat`, også for en ægtefælle og i indlejrede Personskat-input.
Generér en frisk skabelon og overfør gennemgåede kildefakta; ret ikke blot
`schema_hash`. JSON angiver fx `"kirkeskat": {"$variant": "KirkeskatHeleÅret"}`.
Det gamle boolske felt afvises af den nye kontrakt.

Et tidligere `true` eller `false` må kun oversættes til den tilsvarende
helårsstatus, hvis fakta faktisk var afklaret for hele indkomståret. Var det
en skabelonværdi eller et gæt, bruges `KirkeskatUoplyst`; gjaldt det en del af
året, bruges `KirkeskatEnDelAfÅret`. Skatteformlerne for de to understøttede
helårsstatusser er uændrede. Lavniveaumodulets boolske input og den særskilte
betingede rapportafstemnings `betaler_kirkeskat` er ikke ændret.

## Særlige DIS-skattepligtspositioner

En skattepligtsposition kan være lovlig og have et beregneligt delresultat,
uden at den kan indgå i den kanoniske årsopgørelsessammenligning.
Aktiv DIS-fritagelse ved begrænset skattepligt, kulbrinteskattepligt og
dødsbolempelse er endnu ikke understøttet i denne sammenligning. Det gælder
også for en aktiv ægtefælle. Den eksisterende kontrol tilbageholder beløbet;
forklaringen ved `lønmodtager.personlig_indkomst.sømandsbeskatning` angiver nu
udtrykkeligt, at det er en modelbegrænsning, ikke i sig selv en fejl i
kildefakta eller årsopgørelsen.

Et fiktivt kulbrinteeksempel med 100.000 kr. og en konsistent voksenalder
har et beregneligt AM-delbeløb på 8.000 kr. og et nulresultat efter den
modellerede § 5 b-lempelse. Alligevel er det kanoniske sammenligningsbeløb
`null`, ikke nul.

Bevar den faktiske skattepligtsposition. Vælg ikke fuld skattepligt eller
fjern en indkomst alene for at få en beregning til at bestå. De rå
komponenttal er diagnostik; de udgør ikke en valideret fuld årsopgørelse.
En afgrænset [betinget rapportafstemning](aarsopgoerelse-afstemning.md) kan
stadig være relevant for de regnestykker, den dækker, men den verificerer
ikke den manglende særregimeberegning.

Lovgrundlaget for positionerne findes i
[SØBL §§ 5 og 5 b](https://www.retsinformation.dk/eli/lta/2023/1181/pdf) og
[kulbrinteskattelovens § 21, stk. 2](https://www.retsinformation.dk/eli/lta/2025/477/pdf).
Begrænsningen ovenfor tilhører modellen, ikke loven. Kilder kontrolleret
25. september 2026. Den [fokuserede regression](../../tests/personskat_dis_coverage.test.mjs)
kontrollerer hovedperson, ægtefælle, aldersmodstrid og en almindelig sag samt
fire genererede felters vejledning og kildespor. Forny Preview-skabeloner
efter metadataændringen; indkomsttyper og gyldighedsbeslutninger er uændrede.

## Commuting input checks

`lønmodtager.ligningsfradrag.befordring` rejects negative counts for all four
bridge/transport combinations, including Øresund public transport. Negative
counts are invalid facts, not zero crossings. Commuting records describe
disjoint groups of travel days. Their non-negative day counts, taken together,
must fit within the income year's calendar: 366 days in 2024, 365 in 2023, 2025
and 2026. Two individually plausible 220-day rows cannot describe distinct
groups in one year. Negative rows cannot offset excess days. The active spouse
has a separate calendar bound and a prefixed diagnostic; the two people's days
are not added together. Invalid facts withhold the final comparison amount and
remain visible in the result, rather than being truncated to the calendar limit.

This calendar bound is not an allowance to claim every day. Use actual travel
days, excluding home-working, holiday and sick days; see
[SKAT's commuting guidance](https://skat.dk/borger/fradrag/koerselsfradrag/koerselsfradrag-befordringsfradrag).
Passing the calendar bound does not establish whether travel dates overlap. For
multiple workplaces on the same day, use the legally relevant daily total;
do not count the same travel twice or apply the 24-km exclusion twice.

For ferry/flight travel, enter the full documented ticket expense and only the
land kilometres. The model subtracts any unused daily 24-km threshold from the
ticket expense. See the [Danish ferry/flight guide](personskat-faerge-og-fly.md)
for day grouping, source examples and the exact-øre result trace.

The calendar-day and negative-count checks change acceptance of invalid facts,
not the input/result types. The later ferry/flight correction adds result-trace
fields and changes the contract fingerprint. Regenerate affected templates,
correct source facts and recalculate saved results. Do not substitute zero or
shorten a claimed period merely to make the checks pass.

The extra low-income deduction now composes documented Danish unemployment
benefits, G-days, sickness benefits and maternity benefits, preserving the
B-income and voluntary-insurance exclusions in
[LL §9 C(4)](https://www.retsinformation.dk/eli/lta/2025/1500).
It also reuses the annual AMBL §§4–5 business basis, not a sum of each business's
positive amount. See the [Danish benefit guide](personskat-dagpenge.md) for the
new source-fact variant and its compact calculation.

`aftrapningsindkomst_afklaret` distinguishes an established income basis from
a known lower bound; `lavindkomsttillæg_afklaret` separately establishes whether
the deduction can be determined. An unclassified positive generic A-kasse
payment, or missing sickness/maternity exclusion facts, withholds the final
comparison if it could change the extra deduction. With no eligible commuting
basis, or a deduction already fully phased out by known income, zero can be
established without guessing. Private insurance and other non-AM income are
not interchangeable with statutory benefits. These are source-model tests,
not independent verification against every official benefit/business profile.

## Øvrige lønmodtagerudgifter

Kontrollen `lønmodtager.ligningsfradrag.øvrige_lønmodtagerudgifter` gælder også
en aktiv ægtefælle. En negativ erhvervsandel eller en andel over 100 % er
ugyldige fakta og tilbageholder sammenligningsbeløbet. Det er ikke det samme
som en gyldig udgift uden fradragsret: 0 % erhvervsbrug eller almindeligt tøj
kan give nul fradrag uden at gøre input ugyldigt. De oprindelige fakta bevares;
en ugyldig række må ikke blot ignoreres for at få et årsresultat.

Ved AI-assisteret indtastning:

- Andelen er i basispoint: 50 % er `5000`, ikke `50`. Brug en underbygget
  brugsandel, ikke afskrivningssatsen. Ukendt er ikke 0 %.
- Driftsmiddelbeløbet er årets beregnede afskrivning **før erhvervsandel**,
  ikke automatisk købsprisen. Denne gren efterprøver ikke hele
  afskrivningsopgørelsen eller dokumenterer, at aktivet er fradragsberettiget.
- Oplys beløb før refusion og den fælles bundgrænse; modellen foretager disse
  reduktioner. Rubrik 58 på årsopgørelsen er allerede efter bundgrænsen.
  Kopiér ikke det nettobeløb ind som en rå udgift, og udled ikke kildefakta
  baglæns fra det fradrag, der skal efterprøves.

Fiktivt eksempel uden refusion: 20.000 kr. i beregnet afskrivning med 50 %
erhvervsbrug giver 10.000 kr. før bundgrænsen og 2.700 kr. efter 2025-grænsen
på 7.300 kr., når der ikke er andre omfattede udgifter. Indtast altså `20000`
og `5000`, ikke `10000` eller rubrikbeløbet `2700` som afskrivning.
Se [SKATs vejledning om øvrige lønmodtagerudgifter](https://skat.dk/borger/fradrag/arbejdsrelaterede-fradrag/arbejdstoej-faglitteratur-og-kurser-med-mere)
og [den kildeforbundne model](ligningsloven-par9-loenmodtagerudgifter.runa).

Der tilføjes ikke nye obligatoriske inputfelter. Den ændrede felthjælp ændrer
dog kontrakthashen: generér en ny skabelon og overfør gennemgåede fakta.
Ret aldrig en umulig eller ukendt andel til en vilkårlig gyldig værdi for at
få beregningen til at fortsætte. Med kun rapportens beløb kan den
[betingede afstemning](aarsopgoerelse-afstemning.md) stadig være relevant;
den erstatter ikke de manglende udgiftsfakta.

## Union-fee taxpayer status

This is an individual assessment, not a company tax return. With active
union-fee records, `Ll13JuridiskPerson` fails the control at
`lønmodtager.ligningsfradrag.faglige_kontingenter.skatteyderstatus`, including
under the active spouse prefix. The deduction summary is invalid and the final
comparison amount is withheld; diagnostic component amounts are not usable tax.
An empty fee section stays neutral. The shared LL §13 model still supports
legal persons, and the individual self-employed branch is not removed.

The status describes the taxpayer, not whoever paid the bill. An employer's
payment does not turn an employee into a legal person or establish entitlement
to the uncapped branch. Check the actual taxpayer status and any employment
income treatment; do not switch status just to obtain a matching deduction.
See [DJV C.A.4.3.1.3](https://info.skat.dk/data.aspx?oid=2061770), including the
company-paid membership example, and the source text in
[the LL §13 model](ligningsloven-kontingenter-gaver.runa).

## Child-maintenance recipient

Child-maintenance receipts have a separate recipient check at
`lønmodtager.personlig_indkomst.underholdsbidrag`: when `Bidragsmodtager` and
`Barn` are selected, the child's full birth date must match the assessed
person's own birth date. A parent receiving funds on the child's behalf is not
thereby the taxable child. The same check applies to an active spouse; payer
and adult-alimony routes remain distinct. Matching dates are a necessary
consistency condition, not proof of identity. See the
[Danish maintenance guide](personskat-underholdsbidrag.md).

## Alder ved arbejdsudleje

Rækkerne i `lønmodtager.personlig_indkomst.arbejdsudleje` vedrører den samme
person som resten af personsagen, ikke en virksomheds samlede medarbejderliste.
Hver rækkes `alder_ved_indkomstårets_udløb` skal stemme med personens
`lønmodtager.pension.fødselsdato`: indkomståret minus fødselsåret. Det gælder
også i en aktiv ægtefælles egen del af inputtet og ved valg af ordinær skat.

Fra 2026 gælder AM-satsen på nul til og med året, hvor personen fylder 17.
En 18-årsdag i december betyder derfor den almindelige sats for hele året,
ikke blot for dagene efter fødselsdagen.
[LOV 96/2025 § 1 og § 7, stk. 4](https://www.retsinformation.dk/eli/lta/2025/96/pdf),
kontrolleret 25. september 2026.

Fiktiv illustration: En person født i 1990 er 36 ved udgangen af 2026.
En arbejdsudlejerække med alder 17 modsiger fødselsdatoen og tilbageholder
sammenligningsbeløbet med en alderskontrol i `vurdering.fejl`. De øvrige rå
resultater er da diagnostik, ikke en gyldig beregning. Ens alder beviser ikke,
at oplysningerne tilhører samme person; kildefakta skal stadig gennemgås.

Ret ikke en kildeoplysning blot for at vælge en lavere sats. Afklar, om det
er datoen, alderen, året eller placeringen af posten, der er forkert.
Skatteformler og inputtyper er uændrede, men den nye kontrol og metadata
ændrer Preview-fingerprintet: generér en ny skabelon og overfør gennemgåede
fakta. Den [fokuserede regression](../../tests/personskat_labour_hire_age.test.mjs)
dækker begge retninger af aldersmodstrid, ægtefælle, ordinært valg og
2025/2026-grænsen; den er ikke en ekstern årsopgørelsesgodkendelse.

## Recurring-gift agreements

The control `lønmodtager.ligningsfradrag.gaver` also checks that LL §12
payments identify their agreement and confirm the supported ordinary payment
history. Split payments share one annual agreement limit; inconsistent agreement
or recipient facts withhold the comparison, including for an active spouse.
The new `par12_aftaler` result explains the grouping and cap. Per-payment
provisional amounts are not additive annual deductions. Regenerate templates
and review the two new required facts; do not invent agreement identities or
silently confirm an unknown payment history. See the
[Danish gift guide](personskat-gaver.md) for sources, migration and the boundary
for arrears/aconto and other special histories.

## Foreign-employment allocation

The required `lønmodtager.ligningsfradrag.arbejdsfradrag_udland` input starts
unknown. A documented common condition can rule out the exclusion for all
relevant employment; otherwise [source/period allocation](beskaeftigelsesfradrag.md)
must reconcile to the model's independently derived work-deduction basis.
Unknown facts, mismatched sums and an attempted blanket exclusion fail the
canonical validity check, including for an active spouse.

Only the affected employment income is excluded from the basis for ordinary,
job, senior and single-parent employment deductions. AM, personal income and
the LL §9 L pension basis are unchanged. The internal all-or-nothing Boolean
remains `false` because the source-specific exclusion has already been applied;
it is no longer an assumption that the person's foreign facts are false.
See [LBK 1500/2025, §§9 J–9 L](https://www.lovtidende.dk/api/pdf/250970) and
[L 238, 2017–18, notes to §1 no. 3, pp. 15–16](https://www.ft.dk/ripdf/samling/20171/lovforslag/l238/20171_l238_som_fremsat.pdf).

The component result exposes the before/excluded/after amounts and controls.
Source truth, treaty residence, foreign relief and complete tax coverage remain
separate questions. [External rounding observations](skatdk-fradrag-oere-ekstern.md)
now support the whole-øre projection before upward whole-krone rounding in
selected 2025/2026 cases, with three explicit disagreements and unverified older
years. This is not a claim of complete administrative conformance. The
`vurdering.forbehold` output also carries this qualification, including for
invalid inputs. It does not change any amount, make an invalid input usable,
or introduce a comparison tolerance. Do not infer that a small difference is
necessarily rounding, or that it proves the taxpayer's return wrong.
Input and result types are unchanged by this qualification. Recalculate saved
results to include it; merely reopening an old output does not add new warnings.
The [part-year entry](personskat-par14.calculate.runa) also preserves and
reconciles the exclusion before recomputing annual deductions; check its own
`input_gyldigt` rather than using a diagnostic scalar total.

## Updating existing clients

Generate fresh schemas/templates with the current model, transfer reviewed
source facts and recalculate. Do not edit a saved contract hash or a workbook's
hidden fingerprint to bypass validation. Changes to types, outputs and metadata
can all require this step; see the
[compatibility guide](../../docs/compatibility-guides/0.2.x.md) for migration
details. Clients must check `vurdering` before comparing any scalar totals.

[Spouse loss-credit transfers](underskud-modtagersats.md) use the recipient's
rates, including for conversion back to unused losses. Direct `.runa`
constructors of `LønmodtagerPar13Forhold` must supply that derived §13 rate.
[Spouse personal-allowance transfers](personfradrag-samordning.md) use the
donor's allowance amounts after own-tax offsets and the recipient's rates,
not the donor's tax-credit amounts. Read the linked rounding and coverage limits
before treating either calculation as full administrative conformance.

The required [single-parent benefit input](ligningsloven-par9j-enlig.md) starts
unknown, not as confirmed nonreceipt. Preserve unknowns when reviewing older
inputs.

The required [service/handyman input](boligjob.md) also starts as unknown.
Active invoice facts and spouse allocations must pass their component and
household consistency checks; a known ineligible expense is distinct from an
incomplete or unsupported calculation.

Regenerate templates for the required foreign-employment input too. Preserve
unknowns until the facts are available, and update source allocations when a
pension or wage scenario changes the work-deduction basis. Fictional scenario
helpers are not suitable defaults for a real person's treaty-residence facts.

Pension payout completeness is explicit too. Both Boolean fields under
`lønmodtager.pension.udbetalingsoplysninger` start as `false` (unknown or
incomplete), never as confirmed absence. The current year's facts must be
complete before comparing tax. Prior history only needs further clarification
if it can still change the modeled LL §9 L deduction. A known eligible prior
payout may already establish the condition; otherwise the model evaluates the
same source rule with and without the current payout offset, including caps and
rounding. The output keeps `foregående_oplysninger_komplette` distinct from
`foregående_oplysninger_tilstrækkelige`: irrelevant unknowns remain unknown.
An active spouse and the separate part-year calculation have the same gate.
See the [Danish pension interview and migration guide](pension-og-fradrag.md).

The required `lønmodtager.pension.atp` input starts as `AtpUoplyst`, not
confirmed absence. Employer-reported ATP, public-benefit ATP, SUPP paid to ATP
and mandatory pension savings have different deduction bases. Complete source
rows must preserve the reported gross/net amounts and payment identities;
missing net amounts or duplicate payments with other pension routes withhold
comparison. An active spouse and part-year calculation use the same gate.
Regenerate older templates and review actual ATP facts, even without company
pension. The source rows and derived bases remain visible in `pension.atp_resultat`.

Never fill a missing birthday or other unknown fact with a plausible substitute.
When unavailable spouse facts prevent an independent household calculation, use
the [conditional report reconciliation](aarsopgoerelse-afstemning.md) instead.

The focused canonical regression is:

```sh
cargo test --quiet --test personskat_validity -j 1
```

For model-only iteration, `FUTURUNA_MODEL_TEST_RUNA` may name a verified existing
binary from the same compiler revision. Leave it unset for focused compiler
regressions to exercise Cargo's debug binary. The canonical
[mint gate](../../docs/mint-gate.md) builds the current optimized compiler first
and pins these model tests to that fresh artifact, including in CI; it never
trusts an inherited binary override.
