# Førtidspension, seniorpension og tidlig pension i skatteberegningen

Brug pensionsmeddelelsens lovgrundlag og opdeling, ikke blot ordet »pension«.
Den nye indgang dækker ordinære danske udbetalinger i 2023–2026. Den beregner
skattemæssig indkomst, ikke pensionsret eller ydelsens størrelse.

| Dokumenteret ydelse | Behandling i modellen |
| --- | --- |
| Førtidspension efter nye regler, seniorpension eller tidlig pension | Personlig indkomst uden AM |
| Gamle førtidspensionsreglers grundbeløb, pensionstillæg og erhvervsudygtighedsbeløb | Personlig indkomst uden AM |
| Gamle reglers invaliditetsbeløb, invaliditetsydelse, førtidsbeløb og ekstra tillægsydelse | Skattefrit |
| Gamle reglers bistands-/plejetillæg, personligt tillæg, helbredstillæg og § 62-tillæg | Skattefrit |

Kilder: [SKATs pensionsafsnit](https://info.skat.dk/data.aspx?oid=1976824),
[LL § 7, nr. 8](https://www.retsinformation.dk/eli/lta/2025/1500),
[samme fritagelsesliste i 2023](https://www.retsinformation.dk/eli/lta/2023/42)
og [Udbetaling Danmarks forklaring af gamle regler](https://www.borger.dk/oekonomi-skat-su/pension-og-efterloen/foertidspension-gamle-regler).
Det er altså vigtigt ikke at forveksle **erhvervsudygtighedsbeløb** med
**invaliditetsbeløb**. De behandles forskelligt.

## Prøv den lille beregning

Brug [det kontrollerede beregningsprogram](../../website/public/ai-setup.md#tax-audit-runtime-check).
Fra projektmappen:

```sh
./target/release/runa template examples/danish-income-tax/personskat-socialpension.calculate.runa \
  --format json --output /tmp/futuruna-socialpension.json
```

Bevar `$futuruna`. Erstat kun `cases[0].input` med disse **fiktive** fakta:

```json
{
  "identifikation": "fiktiv-socialpension",
  "indkomstår": 2025,
  "kildereference": "fiktiv-pensionsmeddelelse",
  "art": {"$variant": "SocialSeniorpension"},
  "indkomst_før_skat_kroner": 180000,
  "ordinær_dansk_udbetaling_uden_korrektioner": true
}
```

```sh
FUTURUNA_CALCULATION_JOBS=1 ./target/release/runa call \
  examples/danish-income-tax/personskat-socialpension.calculate.runa \
  --input /tmp/futuruna-socialpension.json
```

Resultatet klassificerer 180.000 kr. som personlig indkomst uden AM. Det er
ikke den samlede skat. Vælges i stedet den dokumenterede art
`GammelFørtidspensionInvaliditetsbeløb`, står beløbet i `skattefrit_beløb_kroner`.
Ukendt art, manglende kilde eller uafklaret afgrænsning giver ugyldigt input;
diagnostiske nuller er ikke en afgørelse om skattefrihed.

## Samlet Personskat og ATP/SUPP

Vælg `PersonskatSocialpensionsudbetaling` i
`lønmodtager.personlig_indkomst.ordinære_forhold.forenings_og_arbejdsløshedsydelser`.
Variantens `fakta` har formen ovenfor. Samme indgang findes hos ægtefællen.
Alle 14 dækkede ydelsesarter fremgår af den genererede kontrakt og
[kildemodellen](personskat-socialpension.runa). Opret én post pr. art; medtag
ikke både en samlet pension og dens dele. Dubletter med samme identifikation
afvises, men nye navne gør ikke samme betaling til forskellige betalinger.

Brug årets indkomst **efter eventuel bortseelse for eget ATP/SUPP, før A-skat**.
Det er hverken bankens nettobeløb, ydelsens maksimum eller månedssatsen.
Pensionsopsparing er et særskilt forhold: ATP, SUPP til ATP og obligatorisk
pension har egne indgange under `pension.atp`. Den [eksisterende ATP-model](personskat-atp.runa)
bevarer indberettet brutto/netto og de forskellige fradragsgrundlag. Indtast
ikke samme bidrag som privat pensionsfradrag, og træk ikke ATP/SUPP fra
pensionsindkomsten igen. SUPP hos et andet pensionsinstitut kræver sin rette
pensionsindbetalingsgren, ikke en opdigtet ATP-post.

Ydelsen giver ikke arbejdsfradrag og tæller ikke som en PBL § 20-udbetaling,
der modregnes i ekstra pensionsfradrag. ATP-indbetalinger kan stadig have
betydning for pensionsfradraget; det er en anden regel end indkomstbeskatningen.
Se [pensionsinterviewet](pension-og-fradrag.md) og
[LL §§ 9 J–9 L](https://www.retsinformation.dk/eli/lta/2025/1500).

Brug ikke denne indgang til privat invalidepension, efterløn, delpension,
udland, efterbetaling, tilbagebetaling, omperiodisering eller efterlevelsespension.
[Folkepension har sin egen indgang](personskat-folkepension.md).
Modellen her afgør heller ikke, om mere arbejde eller privat pension ændrer
din offentlige ydelse. Det kræver en særskilt ydelsesberegning.

For [grøn check](personskat-groen-check.md) skal modtagelse ved årets udløb
oplyses særskilt: en pensionsbetaling tidligere på året beviser ikke status
ved årets udløb. Læs altid `vurdering` før den samlede sammenligning.

## Kontrol og opdatering

To fiktive profiler blev aflæst i [SKATs årsberegner for 2025](https://www.tastselv.skat.dk/borger/beregn2025/profil.do)
den 25. september 2026: født 01.01.1960, ugift, København, ingen kirkeskat,
helår, offentlig førtids-/senior-/tidlig pension valgt, 180.000 kr. i rubrik 16A.
Uden løn viste den 45.594,84 kr. i beregnet skat; med 100.000 kr. løn viste
den 83.373,54 kr. før grøn check og slutafregning. Øvrige beløbsfelter var
tomme i denne fiktive nulprofil, herunder ATP; det er ikke en antagelse om
en virkelig pensionsmodtagers ATP-forhold. Ingen login/persondata blev brugt,
og ingen versionsbetegnelse blev registreret.

Den permanente [integrationstest](../../tests/personskat_validity.rs),
`public_early_pensions_preserve_old_exemptions_and_separate_atp_bases`, bruger
disse eksterne beløb. Den gamle ordnings opdeling, særskilte ATP/SUPP-poster,
ægtefæller, kørsel og ugyldige fakta er modelregressioner, ikke flere eksterne
observationer. Den offentlige beregner klassificerer ikke selv de 14 typer.

Den nye variant ændrer Preview-kontraktens fingeraftryk: generer en ny
skabelon og overfør gennemgåede kildefakta. Maskinstien er bevaret; gamle
resultater ændres ikke af at genåbnes. [Model- og afrundingsforbehold](personskat-validity.md)
består. Dette er forskningssoftware, ikke individuel skatte- eller pensionsrådgivning.
