# Børnebidrag: hvis skat beregner vi?

Begynd med at afklare, hvem årsopgørelsen vedrører. Et børnebidrag kan gå ind
på en forælders konto uden at være forælderens skattepligtige indkomst.
Skattestyrelsen henfører den skattepligtige del over normalbidraget til barnet
og barnets årsopgørelse.
[Skattestyrelsens børnebidragsvejledning](https://skat.dk/borger/fradrag/boernebidrag-og-aegtefaellebidrag/fradrag-for-boernebidrag).

Personskat er forskningssoftware til kontrol af oplyste fakta, ikke individuel
rådgivning. Den kan ikke fastslå personers identitet eller bilagenes ægthed.
Beregningskontrakten er [Preview](../../docs/feature-stages.md).

## Vælg den rigtige person og rolle

Input ligger i `lønmodtager.personlig_indkomst.underholdsbidrag.bidrag`.

| Hvad gennemgår du? | Input i den pågældende persons opgørelse |
| --- | --- |
| Forælderens betalte børnebidrag | `rolle = Bidragsyder`, `modtager = Barn`. Barnets fakta og bidragsbetingelser bruges til at beregne et eventuelt fradrag. |
| Barnets modtagne børnebidrag | Barnets **egen** sag: `rolle = Bidragsmodtager`, `modtager = Barn`. Fødselsdatoen i bidraget skal være den samme som personens faktiske dato i `lønmodtager.pension.fødselsdato`. |
| Forælderen administrerer barnets modtagne penge | Indtast ikke beløbet som forælderens egen indkomst i denne gren. Hold barnets opgørelse adskilt. |
| Personens eget modtagne ægtefællebidrag | `rolle = Bidragsmodtager`, `modtager = ÆgtefælleEllerTidligereÆgtefælle`. Det er en anden indkomsttype end barnets bidrag. |

Barnets fødselsdato bruges, selv om der ikke er nogen pension. Udfyld også
barnets egne indkomst-, fradrags- og øvrige fakta; kopier ikke forælderens løn
eller skatteforhold. Børne- og ungeydelse og børnetilskud er ikke samme ydelse
som et underholdsbidrag og skal ikke omklassificeres til denne gren.

Bevar aftale/afgørelse, betalt beløb, forfalds- og betalingsdato, bopælsforhold
og identitetsoplysninger. En lokal bilagsreference behøver ikke indeholde CPR;
feltet om identitet oplyst til Skatteforvaltningen er en bekræftelse af det
forhold, ikke et CPR-felt. Flere betalinger for samme barn og forfaldsmåned
samles i én kildeunderbygget månedsrække efter den eksisterende kontrakt.
Brug ikke et ønsket fradrag eller en ønsket slutskat som kildefakta.

## Hvad viser kontrollen?

Kontrollen `lønmodtager.personlig_indkomst.underholdsbidrag` afviser modtaget
børnebidrag, når barnets og skatteyderens fødselsdatoer er forskellige. Det
gælder også en aktiv ægtefælles gren. Ved fejl er
`vurdering.slutskat_til_sammenligning_øre` tomt; beløb i delresultaterne er da
kun diagnostik, ikke skat der må sammenlignes med årsopgørelsen.

Kontrollen kræver ikke, at en **betalende forælder** har barnets fødselsdato.
Den ændrer heller ikke reglerne for personens modtagne ægtefællebidrag.
En ens fødselsdato beviser ikke, at to poster vedrører samme person. Du skal
stadig afklare identiteten og de øvrige betingelser fra kilderne. Ret ikke
datoer eller roller alene for at få en kontrol til at bestå.

I den fiktive [kanoniske prøve](../../tests/personskat_maintenance_recipient.test.mjs)
er et offentligt fastsat martsbidrag på 3.000 kr. i 2025 til et barn født i
2012 registreret korrekt som 1.397 kr. personlig indkomst hos barnet, uden
AM-bidrag. Yderens tilsvarende fradrag er 2.816 kr. Det er forskellige størrelser,
ikke beløb der skal være ens. Beløbene er ikke generelle skattebesparelser.

## Eksisterende sager

Ingen nye inputfelter er nødvendige. Den forbedrede feltvejledning ændrer dog
Preview-kontraktens fingeraftryk: generér en ny skabelon, gennemgå roller og
overfør faktiske oplysninger. Kopier ikke blot det nye fingeraftryk til gamle
sager. Tidligere output kan have været markeret gyldigt trods en åbenlys
forælder/barn-konflikt; beregn berørte sager igen.

Brug den [kontrollerede compiler](../../website/public/ai-setup.md#tax-audit-runtime-check)
og den [danske resultatvisning](personskat-validity.md#læs-dit-gemte-resultat-på-dansk).
Den [eksisterende lovmodel](ligningsloven-par10-par11-underhold.runa) bevarer
lovtekst og kilder for LL §§ 10–11. Ændringen 25. september 2026 er en
personsammenhængskontrol, ikke en ny beregning af normalbidrag eller en fuld
validering af alle underholds-, restancesager og børns skatteforhold.
