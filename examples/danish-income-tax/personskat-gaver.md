# Gaver: én aftale har ét årligt aftalebeløb

Betaler du i flere rater under samme bindende gaveaftale, skal modellen kende
sammenhængen. To betalinger på 6.000 kr. under én aftale på 10.000 kr. giver
her højst 10.000 kr. i § 12-grundlag før det fælles indkomstloft — ikke 12.000 kr.
Det er et **fradrag**, ikke en skattebesparelse på 10.000 kr.

Guiden gælder den ordinære gavegren i Personskat for 2023–2026. Modellen er
forskningssoftware, ikke individuel rådgivning; `schema`, `template` og `call`
er [Preview](../../docs/feature-stages.md). Den kontrollerer oplyste fakta,
ikke bilagenes ægthed eller om alle personens skatteforhold er dækket.

## Hvad skal du oplyse?

Start med aftalen, årets betalinger og foreningens indberetning. Brug ikke
fradraget på årsopgørelsen til at konstruere et passende aftalebeløb.
Gaver ligger i `lønmodtager.ligningsfradrag.gaver.gaver`:

| Oplysning | Betydning |
| --- | --- |
| `identifikation` | Entydig reference til den enkelte betaling; indtast ikke samme betaling to gange. |
| `art.aftale_identifikation` | Ved `Ll12BindendeLøbendeYdelse`: samme stabile reference på alle betalinger under samme aftale. Forskellige reelle aftaler får forskellige referencer, også hos samme forening. Brug ikke CPR eller en ny aftalereference for hver rate. |
| `art.forfalden_årlig_ydelse_efter_aftalen_kroner` | Hele årets forfaldne aftalebeløb, gentaget på hver rate — ikke ratens andel af beløbet. |
| `art.ordinært_betalingsforløb_bekræftet` | Bekræft først efter gennemgang: betalingerne vedrører årets fortsat reelle forpligtelse uden tidligere års restancer, aconto eller særlige aftaleforløb. Uafklaret er `false`, ikke automatisk ingen fradragsret. |

Bevar også år, betalt beløb, modtager, godkendelse, indberetning og aftalevilkår.
Modtager- og aftalefakta skal stemme overens inden for samme aftalereference.
Om den enkelte betaling er indberettet, kan derimod variere mellem raterne.
Aftalereferencer må ikke være tomme eller have indledende/afsluttende mellemrum.
En lokal reference kan knyttes til bilaget uden at lægge personlige dokumenter
eller CPR-numre i projektet. Feltet `navn_og_cpr_på_aftalen` er en bekræftelse
af aftalens indhold, ikke en anmodning om selve CPR-nummeret.

Almindelige § 8 A-gaver og § 8 H-forskningsgaver bruger fortsat deres egne
varianter; de kræver ikke en § 12-aftalereference. En overbetaling flyttes ikke
automatisk mellem gavetyper. Skattestyrelsen beskriver aftalebeløbets grænse og
særskilt aftale/indberetning ved ændret behandling af overskuddet. Det er
foreningen, ikke giveren, der indberetter gaven.
[Skattestyrelsens gavevejledning](https://skat.dk/borger/fradrag/fradrag-for-gaver-og-bidrag-til-velgoerende-foreninger).

## Læs aftalesporet før det samlede fradrag

`ligningsfradrag.gaver.par12_aftaler` viser pr. aftale betalingsreferencer,
betalt beløb, årets forfaldne beløb, kvalificerende betalinger og grundlaget
efter aftaleloftet. For de to fiktive 6.000 kr.-betalinger ovenfor vises
12.000 kr. betalt og 10.000 kr. efter aftaleloftet. Det eksisterende fælles
§ 12-indkomstloft anvendes derefter på summen af aftalernes grundlag.

`gaveresultater` bevarer de enkelte betalinger, men deres
`fradragsgrundlag_før_årsloft_kroner` er foreløbige beløb. **Læg ikke disse
§ 12-rækkebeløb sammen som årets fradrag.** Brug aftalesporet og årsresultatets
`par12_fradrag_kroner` / `samlet_fradrag_kroner`, og kun når gyldigheden tillader det.

Manglende aftalereference, modstridende aftalefakta eller ubekræftet ordinært
forløb fejler kontrollen `lønmodtager.ligningsfradrag.gaver`. Personskat
tilbageholder da `vurdering.slutskat_til_sammenligning_øre`. Et diagnostisk
nulfradrag er ikke en afgørelse om manglende fradragsret. Det samme gælder
en aktiv ægtefælles gavegren; resultatet ligger under
`ægtefælle.grundlag.ligningsfradrag.gaver`. Den
[danske resultatvisning](personskat-validity.md#læs-dit-gemte-resultat-på-dansk)
viser fejlede kontroller, før nogen skattesammenligning foretages.

Tidligere års restancer, procentaftalers aconto/efterregulering, ophør og
andre særlige forløb kræver særskilt behandling. Afgrænsningen er modellens,
ikke en påstand om at loven afskærer disse betalinger. Den juridiske vejledning
beskriver betaling/forfald og vurderingen af en reel forpligtelse, også ved
underbetaling. [DJV C.A.4.3.5.1](https://info.skat.dk/data.aspx?oid=2061726).
Lovgrundlaget er den kildebevarede [LL § 12-model](ligningsloven-kontingenter-gaver.runa)
med henvisning til [ligningsloven](https://www.retsinformation.dk/eli/lta/2025/1500).

## Eksisterende input skal gennemgås igen

Ændringen 25. september 2026 tilføjer to påkrævede felter på
`Ll12BindendeLøbendeYdelse` og ændrer kontraktfingeraftrykket. Generér en ny
skabelon med den [kontrollerede compiler](../../website/public/ai-setup.md#tax-audit-runtime-check),
og overfør kun gennemgåede kildefakta. Ret ikke blot det gamle fingeraftryk,
og udfyld ikke automatisk aftalereferencer eller bekræftelsen med `true`.
Andre gavetyper har ingen nye aftalefelter, men det samlede kontraktfingeraftryk
er også ændret for deres skabeloner.

Permanente kontroller ligger i
[komponentprøven](../../tests/personskat_recurring_gifts_test.runa) og
[den kanoniske prøve](../../tests/personskat_recurring_gifts.test.mjs).
De bruger kun fiktive fakta og kontrollerer bl.a. delbetalinger, separate aftaler,
fælles loft, modstridende fakta og ægtefællens gren. Det er modelregressioner,
ikke uafhængig validering af alle gaveforløb hos Skattestyrelsen.
