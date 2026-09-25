# Personskat med kildeafledt grøn check

[Denne beregning](personskat-groen-check.calculate.runa) forbinder grøn check med
Personskats indkomst-, pensions- og fradragsberegning. Du indtaster ikke personlig
indkomst og nettokapitalindkomst igen: de udledes af de samme kildefakta som skatten.
Kreditten indsættes én gang i slutopgørelsen. Reglerne kører i Futuruna, ikke en LLM.

Det er en **særskilt Preview-indgang for ordinære helår 2023–2026**, ikke en
ændring af eksisterende sager. Skattemodellen er researchsoftware, ikke individuel
rådgivning eller dokumentautentifikation. Hvis du kun har rapportens indkomsttal,
er den [kompakte grøn-check-kontrol](groen-check.md) fortsat en mindre indgang.
Hvis ægtefællens fakta mangler, kan den [betingede rapportkontrol](aarsopgoerelse-afstemning.md)
vise nødvendige beløb uden at opfinde ægtefællens indkomst.

Et faktisk kørt, **fiktivt** eksempel: 2025, født i 1950, løn på 302.000 DKK,
København, ingen kirkeskat, ægtefælle, børn, kapitalindkomst eller pensionsudbetalinger.
Uden pensionsindbetaling beregner modellen personlig indkomst efter AM til
277.840 DKK og grøn check til 1.005 DKK. Med en fradragsberettiget privat
ratepensionsindbetaling på 1.000 DKK bliver indkomsten 276.840 DKK og kreditten
1.285 DKK: lavindkomsttillægget på 280 DKK vender tilbage. Indbetalingen er
stadig en udgift til pension, ikke frie penge. Det er et modelresultat fra
integrationstesten, ikke en eksternt verificeret skatteansættelse eller anbefaling.

## Brug eksisterende Personskat-fakta

Brug en compiler, som består [runtime-kontrollen](../../website/public/ai-setup.md#tax-audit-runtime-check).
Fra checkoutens rod:

```sh
runa template examples/danish-income-tax/personskat-groen-check.calculate.runa --entry beregn_personskat_med_grøn_check --format json --output /absolut/privat/sti/input.json
runa call examples/danish-income-tax/personskat-groen-check.calculate.runa --entry beregn_personskat_med_grøn_check --input /absolut/privat/sti/input.json --output /absolut/privat/sti/resultat.json
```

Erstat stierne med en eksisterende privat mappe uden for checkouten. Udfyld input
før `call`; skabelonens standardværdier er ikke viden om din situation.

1. Bevar den nye skabelons kontrakt. Under hver sags `input.personskat` placeres
   det eksisterende, gennemgåede Personskat-inputobjekt — ikke det gamle resultat
   eller hele det gamle inputdokument. Brug den [kanoniske gyldighedsguide](personskat-validity.md)
   til indkomst-, pensions-, fradrags- og ægtefællefakta.
2. Vælg `MedEksaktÅrsopgørelse` under `personskat.årsopgørelse`. Alle beløb her
   er øre. Bevar dokumenterede betalinger og øvrige fakta. Feltet
   `kreditter.energiafgiftskompensation_øre` skal være 0: den nye indgang udleder
   denne kredit. Gem et tidligere oplyst rapportbeløb i observationen nedenfor.
3. Ved `AfregnOverskydendeSkat` skal
   `afregningsfakta.fakta.modsvarer_energiafgiftskompensation_øre` også være 0.
   Den nye indgang udleder udelukkelsen fra procentgodtgørelsen. Disse to nuller
   er bevidste pladsholdere for afledte felter, **ikke antagelser om nul ret**.
   Et allerede indsat beløb bliver afvist, ikke lagt til igen eller ignoreret.
4. Udfyld `grøn_check_fakta`: skattepligt 1. januar, forskerordning, eventuel
   offentlig tidlig pension ved årets udløb, helårsforløb, skattemæssigt samliv,
   eventuelt PBL § 16-tillæg, komplet børneliste og lokal kildereference.
   [Børne- og pensionsbetingelserne](groen-check.md#kør-lokalt) er de samme som
   i den kompakte beregning. Folkepensionsalderen udledes af Personskats fødselsdato.
5. `oplyst_samlet_grøn_check_øre` er en valgfri rapportobservation, som alene
   sammenlignes med summen af pensionistbeløb, børnebeløb og tillæg. `null` er
   ukendt; 1.285,00 DKK skrives `128500`. Observationen påvirker aldrig retten.

Samlivet kontrolleres mod Personskats ægtefællegren. Et helårsvalg udleder
ægtefællens nettokapitalindkomst fra den allerede beregnede ægtefælle; der
kræves ikke endnu et rapporttal. Ukendte ægtefællefakta må stadig ikke udfyldes
med nul for at få en samlet skatteberegning.

## Læs resultatet i denne rækkefølge

- `vurdering`: Ved `UgyldigtBeregningsgrundlag` er både
  `slutskat_til_sammenligning_øre` og `årsopgørelse` `null`. Læs alle `fejl`.
  Eventuelle øvrige delbeløb er diagnostik, ikke en samlet konklusion.
- `grøn_check_grundlag`: viser den afledte personlige indkomst efter AM,
  nettokapitalindkomst, fødselsdato, år og øvrige anvendte fakta.
- `grøn_check`: viser komponenter, uafklarede forhold og en eventuel forskel
  mellem rapporten og modellen. Forskellen er **oplyst minus beregnet**.
  En forskel tilbageholder ikke en ellers gyldig beregning og ændrer ikke dens
  kredit; den skal undersøges, ikke automatisk kaldes en myndighedsfejl.
- `årsopgørelse`: har samme beregnede struktur som den eksisterende kanoniske
  årsopgørelse. `input.kreditter.energiafgiftskompensation_øre` viser den udledte
  kredit; `resultat` viser restskat eller overskydende skat. `afregning` beregnes
  kun med de valgte betalingsfakta. Uden betalingsafregning er resultatet ikke
  en påstand om beløbet, der bliver udbetalt på en bestemt dato.

Slutskat **før kreditter** ændres ikke af grøn check. Det er kreditten og dermed
restskat/overskydende skat, der ændres. Betalingsretningen kontrolleres efter
kreditten, så en lille restskat godt kan blive til overskydende skat.

Grøn-check-delen af en tilbagebetaling indgår ikke i grundlaget for almindelig
procentgodtgørelse efter [KSL § 62, stk. 2](https://www.retsinformation.dk/eli/lta/2024/460).
Den afledte udelukkelse er højst den endelige overskydende skat; de eksisterende
KSL-regler anvendes derefter. Andre udelukkelser, renter og restancer leveres
fortsat som særskilte fakta. Det beviser ikke, at alle disse øvrige fakta er
korrekt opgjort eller indbyrdes afstemt.

Den nuværende KSL-model har afregningssatser for 2023–2025, ikke 2026. For 2026
kan skatten og kreditten beregnes med `UdenSlutopgørelsesafregning`, men en valgt
betalingsafregning tilbageholdes, indtil det relevante satsgrundlag er dækket.
Skift ikke et korrekt indkomstår for at få en udbetalingsberegning.

## Afgrænsning og kompatibilitet

Denne samlede indgang kræver ordinært helår med fuld skattepligt, ingen
forskerskatteordning og kendt nul PBL § 16-tillæg. Personskats almindelige
PSL § 7-grundlag har ikke et positivt kapitalpensionstillæg; vi må derfor ikke
beregne en tilsyneladende fuldstændig skat, hvis det faktisk foreligger.
`null` tilbageholder sammenligningen, og et positivt tillæg kræver særskilt modelarbejde.

Delår, migration, dødsår, grænsegængere, samliv kun en del af året samt de
[åbne ægtefællekapital- og særlige børneydelsesgrene](groen-check.md#kendte-grænser--ingen-opdigtet-nulret)
tilbageholder den samlede slutopgørelse. Gyldige kontroller er ikke fuld
lovdækning: `samlet_modeldækning_bekræftet` forbliver `false`.
For ydelse delvis til barnet selv fra 2026 kan den samlede indgang dog bruges,
når den kompakte beregning viser, at den åbne loftsfortolkning ikke kan ændre
børnebeløbet. Ét berettiget barn i denne gren giver fx 240 DKK før aftrapning.
Kan loftet ændre beløbet, forbliver både grøn check og slutopgørelsen tilbageholdt.

Den gamle `beregn_personskat` og dens input-/resultattyper er uændrede; den
gamle indgang accepterer fortsat en **ekstern** grøn-check-kredit uden at bevise
retten til den. Gamle input migreres kun ved bevidst valg af den nye indgang og
en ny skabelon. Den betingede rapportafstemning ændres heller ikke automatisk.

Den permanente [integrationstest](../../tests/personskat_green_check.test.mjs)
bruger kun fiktive fakta og sammenligner med den eksisterende kanoniske
årsopgørelse. Den dækker løn/pension/kapital, børn, kendt nul, ukendte forhold,
dobbelt kredit, betalingsretning, ægtefæller og godtgørelsesgrundlag. Det er
regressionsdækning, ikke nye eksternt verificerede skatteansættelser.
