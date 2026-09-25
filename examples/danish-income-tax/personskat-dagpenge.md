# Dagpenge og det ekstra kørselsfradrag

Har du både løn og dagpenge i løbet af året? Beløbene skal ikke nødvendigvis
behandles ens. Denne lille beregning viser den personlige indkomst uden nyt
AM-bidrag og dagpengenes bidrag til indkomstgrundlaget for det ekstra
kørselsfradrag. Den beregner ikke selve dagpengeretten eller hele din skat.

Modellen dækker dokumenterede danske arbejdsløshedsdagpenge, G-dage,
sygedagpenge og barselsdagpenge for 2023–2026. Det er forskningssoftware og
en Preview-beregningskontrakt, ikke individuel skatterådgivning.

## Prøv med fiktive oplysninger

Brug først [kontrollen af beregningsprogrammet](../../website/public/ai-setup.md#tax-audit-runtime-check).
Fra projektmappen kan du lave en lille skabelon uden hele Personskat-arbejdsbogen:

```sh
./target/release/runa template examples/danish-income-tax/personskat-dagpenge.calculate.runa \
  --format json --output /tmp/futuruna-dagpenge.json
```

Bevar skabelonens `$futuruna`-del. Erstat kun `cases[0].input` med disse
**fiktive** fakta:

```json
{
  "identifikation": "fiktiv-dagpengepost",
  "indkomstår": 2026,
  "kildereference": "fiktivt-udbetalingsbilag",
  "art": {
    "$variant": "ArbejdsløshedsdagpengeEfterDanskArbejdsløshedsforsikringslov"
  },
  "indkomst_før_skat_kroner": 100000
}
```

Kør beregningen:

```sh
FUTURUNA_CALCULATION_JOBS=1 ./target/release/runa call \
  examples/danish-income-tax/personskat-dagpenge.calculate.runa \
  --input /tmp/futuruna-dagpenge.json
```

Resultatet skal have `input_gyldigt: true`, personlig indkomst uden AM på
100.000 kr. og `ll9c_aftrapningsindkomst_kroner: 100000`. Det sidste er et
**indkomstgrundlag**, ikke et fradrag eller en skattebesparelse.

Et menneske eller en assistent kan hjælpe med at indtaste og klassificere
bilagene. Selve resultatet følger af Futurunas aritmetik og regler, ikke af
en LLM-vurdering.

## Hvilke oplysninger skal du bruge?

Brug indkomstårets dokumenterede beløb **før A-skat, efter eventuel
ATP-bortseelse** — ikke bankens nettoudbetaling. Beløbsfeltet er i hele kroner;
modellen fastlægger ikke en ny afrundingsregel for bilag i øre. ATP-bidrag
oplyses særskilt under `pension.atp` i Personskat og må ikke trækkes fra
dagpengebeløbet igen. ATP kan have sin egen allerede indeholdte AM; udsagnet
om intet nyt AM på dagpengene betyder ikke, at ATP er bidragsfrit.
Se [Skattestyrelsens ATP-vejledning](https://info.skat.dk/data.aspx?oid=2048235).

Løn under sygdom eller barsel hører fortsat til lønnen. En refusion til
arbejdsgiveren er ikke din ekstra indkomst. Brug ikke også den samme
dagpengepost som løn eller som en PBL § 49-udbetaling.

| Dokumenteret ydelse | Personlig indkomst uden nyt AM | Indgår i kørselsfradragets indkomstgrundlag? |
| --- | --- | --- |
| Danske arbejdsløshedsdagpenge eller G-dage | Ja | Ja |
| Sygedagpenge | Ja | Ja, med nedenstående undtagelser |
| Barselsdagpenge | Ja | Ja, med nedenstående undtagelse |
| Syge-/barselsdagpenge, der erstatter B-indkomst | Ja | Nej |
| Sygedagpenge som frivillig sikring efter § 45 | Ja | Nej |

Afgrænsningen følger [LL § 9 C, stk. 4](https://www.retsinformation.dk/eli/lta/2025/1500)
og [den juridiske vejlednings indkomstafgrænsning](https://info.skat.dk/data.aspx?oid=2061745).
Skattepligt og manglende AM på selve dagpengene er beskrevet i
[C.A.2.2.1](https://info.skat.dk/data.aspx?oid=2061678) og
[Skattestyrelsens AM-vejledning](https://skat.dk/borger/am-bidrag).

Ved sygedagpenge oplyses både `erstatter_b_indkomst` og
`frivillig_sikring_efter_par45`; ved barselsdagpenge kun det første. Brug
`true` eller `false` efter dokumentationen og `null` ved ukendt. En kendt
undtagelse er tilstrækkelig til at udelukke beløbet fra kørselsgrundlaget.
Ellers bevares et uafklaret grundlag som `null`, ikke nul kroner.

## Brug i den samlede Personskat-beregning

Den nye variant `PersonskatLovbestemteDagpenge` ligger i den eksisterende liste
`lønmodtager.personlig_indkomst.ordinære_forhold.forenings_og_arbejdsløshedsydelser`.
Dens `fakta` er samme objekt som i det lille eksempel. Den aktive ægtefælle
har samme mulighed. Generér en ny Personskat-skabelon, og overfør gennemgåede
fakta; gamle skabeloners fingeraftryk passer ikke til den udvidede kontrakt.

Hver post skal have korrekt år, en kildehenvisning og entydig identifikation.
Identiteten kontrolleres på tværs af listens varianter. To forskellige
identifikationer beviser dog ikke, at der er to forskellige udbetalinger.

Den generelle A-kassevariant efter PBL § 49 er bevaret, men dokumenterer ikke
alene det lovgrundlag, som LL § 9 C kræver. Et positivt beløb i den gren kan
derfor gøre det ekstra kørselsfradrag uafklaret. Privat arbejdsløshedsforsikring
skal stadig bruge sin særskilte PBL § 49-variant; den må ikke omklassificeres
til danske lovbestemte dagpenge for at få beregningen til at passe.

I `ligningsfradrag.befordring` viser:

- `aftrapningsindkomst_afklaret`, om indkomstgrundlaget er afklaret. Hvis den
  er falsk og årsgrundlaget ellers er gyldigt, er `aftrapningsindkomst_kroner`
  kun den kendte undergrænse, ikke et præcist årsbeløb.
- `lavindkomsttillæg_afklaret`, om tillægget alligevel kan fastslås. Hvis der
  ikke er et relevant kørselsgrundlag, eller den kendte indkomst allerede
  aftrapper tillægget til nul, kan nul fastslås uden at gætte ydelsesarten.

Når den manglende oplysning kan ændre tillægget, tilbageholdes det samlede
sammenligningsbeløb. Et diagnostisk nul i fradragsfelterne er da **ikke** en
konklusion om manglende fradragsret. Læs altid `vurdering` først.

Selvstændiges indkomstgrundlag genbruger nu Personskats årlige AMBL §§ 4–5-
resultat, frem for blot at summere positive enkeltvirksomheder. Det ændrer
ikke de underliggende regler eller krav til dokumentation af underskud og
beskatningsordning. De fokuserede tests dækker bl.a. årlig modregning og
fremførsel; det er ikke uafhængig verifikation af alle virksomhedssituationer.

## Afgrænsning

Den lille dagpengeberegning omfatter ikke efterløn, andre sociale ydelser,
udenlandske ordninger, tilbagebetalinger eller omperiodisering. Den kontrollerer
heller ikke autenticiteten eller fuldstændigheden af dine bilag. Det kan ikke
løses ved at indtaste et restbeløb fra den skat, du forsøger at kontrollere.
Kendte afrundingsforbehold og øvrige modelgrænser gælder fortsat; se
[Personskats gyldighedsvurdering](personskat-validity.md).
