# Medlemsbeviser, formidlerbetalinger og provisioner

Listen `kapitalindkomst.finansielle_poster` er tre bestemte indgange efter
PSL § 4, stk. 1, nr. 5 a, 5 b og 7. Den er ikke en generel indgang til alle
værdipapirer eller beløb kaldet provision. Samme regler gælder en beregnet
ægtefælles tilsvarende liste.

## Afklar indkomstens rute før sammenligning

- **Medlemsbeviser (nr. 5 a):** angiv den allerede kildeopgjorte skattepligtige
  gevinst eller det fradragsberettigede tab med fortegn. Det er ikke salgssum,
  depotværdi eller automatisk forskellen mellem to rapporttal. Udstederen skal
  omfattes af SEL § 1, stk. 1, nr. 6, og må ikke være en investeringsforening.
- **Formidlerbetaling (nr. 5 b):** angiv et modtaget beløb med dokumenteret
  betalingsårsag, nævnt formidlertype og afsluttet klassifikation efter ABL
  § 19 C eller § 22. Det er ikke en betalt omkostning. Den dækkede omklassifikation
  til personlig indkomst efter PSL § 4, stk. 6, bevares.
- **Provision som udgift (nr. 7):** den særskilte LL § 8, stk. 3-model afgør
  fradragsretten. Et afklaret manglende fradrag kan lovligt give nul; det er
  ikke samme problem som en uafklaret indkomst, der forsvinder.

En investeringsforening er udtrykkeligt undtaget fra **nr. 5 a**, ikke dermed
fra beskatning. Skattestyrelsens
[investeringsvejledning](https://skat.dk/borger/aktier-og-andre-vaerdipapirer/skat-af-investeringsbeviser-udstedt-af-investeringsforeninger-og-investeringsselskaber)
beskriver forskellige indkomstarter og opgørelsesmetoder. Klassifikation,
beholdningsforløb og eventuelle tabsbetingelser skal afklares før valg af
modellens understøttede `aktieavance`- eller `kapitalindkomst.kursgevinst`-rute.
Flyt ikke blot nettobeløbet til renter eller løn. En anden formidler eller
betalingsårsag kræver ligeledes selvstændig afklaring; modellen gætter ikke
en alternativ behandling.

Lovgrundlaget for afgrænsningerne er
[PSL § 4, stk. 1, nr. 5 a/5 b/7, og stk. 6](https://www.retsinformation.dk/eli/lta/2021/1284).
Kildemodellens lokale resultat om, at en post falder udenfor, bevares.
Den ekstra årsopgørelseskontrol er en **modelgrænse**, ikke en ny skatteregel
eller et udsagn om, at ethvert udelukket beløb er skattepligtigt.

## Når beløbet ikke dækkes

Et ikke-nul beløb uden for den valgte nr. 5 a/5 b-gren tilbageholder den
kanoniske sammenligning. `vurdering.fejl` peger på
`kapitalindkomst.finansielle_poster`, eventuelt med ægtefællens præfiks.
Postens identifikation, oprindelige fakta og lovresultat findes fortsat i
`kapitalindkomst.finansielle_postresultater`. Kontrollér også år, fortegn,
entydige referencer og aktivklassifikation ved en fejl på denne liste.
Listen er atomisk: én ugyldig post tilbageholder hele listens bidrag.

Bevar kildefakta. Brug hverken nul, en anden udstedertype eller en bekræftelse
uden belæg til at få et tal. Fjern kun posten fra den forkerte indgang, når
den korrekte behandling er dokumenteret og medtaget præcis én gang. Er den
behandling endnu ikke dækket af modellen, er en samlet uafhængig sammenligning
fortsat ikke etableret. Et dokumenteret nul passerer denne rutekontrol, men
ophæver ikke de øvrige inputkrav.

Kontrollen gælder også delårets grundlag og et særskilt dokumenteret
helårsgrundlag, herunder ægtefællen. Læs den yderste delårsvurdering.
Se [gyldighedsvejledningen](personskat-validity.md).

Generér friske `schema`/`template`-filer efter opdateringen. Den typede
feltvejledning og dens kilder projiceres til både hovedperson og ægtefælle
og ændrer kontraktens fingerprint. Ingen personlige input migreres automatisk.
Futuruna er forskningssoftware; dette er ikke individuel skatterådgivning.
