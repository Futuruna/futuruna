# Underskud overført mellem ægtefæller

Underskud efter PSL §13 overføres som indkomstbeløb. Den del, som ikke kan
fradrages i modtagerens skattepligtige indkomst, omregnes til en skattekredit
med **modtagerens satser**, ikke afsenderens. Ved delvis udnyttelse bruges
samme modtagersats til at beregne det resterende underskud.
Det følger af [Den juridiske vejledning C.A.1.2.3.2.1](https://info.skat.dk/data.aspx?oid=2061672),
sammenholdt med [PSL §13](https://www.retsinformation.dk/eli/lta/2021/1284).

Den kanoniske beregning afleder satsen fra ægtefællens allerede oplyste
kildefakta. Brugeren skal ikke levere en beregnet skat eller en ny sats.
Ukendte ægtefællefakta er stadig ukendte, ikke nul; brug
[betinget afstemning](aarsopgoerelse-afstemning.md), når de mangler.

## Uafhængige observationer

Tre fiktive cases blev aflæst 25. september 2026 i SKATs anonyme
[Beregn skatten 2025](https://www.tastselv.skat.dk/borger/beregn2025/profil.do).
Begge personer er født 1. januar 1990, gift og skattemæssigt samlevende hele
året. Modtageren bor i København og tjener 600.000 kr. Afsenderen bor i
Ballerup, har ingen løn og har 503.500 kr. i renteudgifter af anden gæld
(rubrik 44). Der er ingen øvrige indkomster, ATP, pensioner, ejendomme eller
fradragsudgifter. De almindelige arbejdsfradrag beregnes af lønnen.
Ingen private dokumenter eller login blev brugt.

Det store rentebeløb er en bevidst stresstest, ikke et typisk husholdningsbudget.
Af underskuddet bruges 493.500 kr. mod modtagerens skattepligtige indkomst;
de sidste 10.000 kr. omregnes til skattekredit.

| Kirkemedlem: modtager / afsender | Underskudskredit, kr. | Modtagers beregnede skat inkl. AM, kr. |
|---|---:|---:|
| nej / nej | 2.350,00 | 67.298,88 |
| ja / nej | 2.430,00 | 66.393,28 |
| nej / ja | 2.350,00 | 67.298,88 |

Beløbene er før betalinger og restskattetillæg. Den første case gav tidligere
67.098,88 kr. i modellen: afsenderens 25,5 % blev fejlagtigt brugt i stedet
for modtagerens 23,5 %. Det gav 200 kr. for stor kredit.

## Beregningsspor, migration og afgrænsning

`indgående_ægtefælle.par13_indkomstfradrag_kroner` viser indkomstdelen;
`par13_skattemodregning_kroner` viser den udnyttede kredit i helkroneberegningen.
I øreberegningen ses modregningen mellem `hovedskat_eksakt.før_nedsættelser`
og `efter_par13`. Brug fortsat `vurdering` og den eksakte sammenligningsskat
til en årsopgørelse, ikke helkroneprojektionen.

Direkte `.runa`-brugere skal tilføje
`LønmodtagerPar13Forhold.ægtefælle_skatteværdi_sats_basispoint` fra modtagerens
§13-beregning. Nul er kun standard i den inaktive vej uden ægtefælleoverførsel;
det er ikke en erstatning for en ukendt sats. De eksisterende fiktive audits
med ens satser angiver nu modtagerens sats eksplicit. Personskats offentlige
inputfelter er uændrede. Dette er en adfærdsrettelse i forskningsmodellen og
en kildeændring i den interne model-API, ikke ny sprogsemantik.

De tre eksterne observationer dækker ordinær helårsbeskatning i 2025.
Komponentkontroller dækker også delvis udnyttelse, modtagerens egne tidligere
underskud og manglende samliv. De dokumenterer ikke fuld administrativ
konformitet for alle år, delår, udenlandske indkomster eller afrundinger ved
delvis udnyttelse. Helkroneberegningens øvrige afrundinger er ikke ændret.

Fokuserede regressioner:

```sh
cargo test --quiet --test personskat_validity spouse_loss_uses_recipient_rates_from_source_facts -- --exact --nocapture
cargo test --quiet --test tax_parameter_domain spouse_loss_recipient_rates_preserve_capacity_and_priority -- --exact --nocapture
```

`FUTURUNA_MODEL_TEST_RUNA` kan pege på en allerede verificeret compiler fra
samme compilerrevision. Den første test kontrollerer kildefakta, gyldighed,
slutskat, kredit og underskudssaldo. Den anden kører seks nye og fire eksisterende
fortolkede komponentkontroller. Native generering af hele lønmodtagermodellen
har særskilt registrerede problemer (`td-124b83`) og er ikke valideret her.
