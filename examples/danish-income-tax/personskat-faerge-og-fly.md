# Færge og fly i kørselsfradraget

Futuruna beregner fradraget fra dine transportfakta. Indtast ikke årsopgørelsens
færdige fradrag som billetudgift: så bliver sammenligningen cirkulær.
Modellen er forskningssoftware, og beregningsgrænsefladen er Preview.

## Hvilke oplysninger skal du bruge?

Under `lønmodtager.ligningsfradrag.befordring.forhold` angives:

- `arbejdsdage`: de faktiske rejsedage for netop denne række.
- `daglige_befordringskilometer`: dagens samlede normale landtransport frem og
  tilbage, før og efter færgen/flyet. Medregn **ikke** færgens eller flyets km.
- `særlig_transport.faktisk_dokumenteret_udgift_kroner`: den samlede dokumenterede
  billetudgift for rækkens rejsedage i indkomståret, før bundgrænsen trækkes fra.
  Gem betalingsdokumentation, også for et medtaget transportmiddel.
- `særlig_transport.geografiske_forhold_tidsforbrug_økonomisk_rimelighed_kræver_transporten`:
  om den særlige transport opfylder LL § 9 C, stk. 1's betingelser. En købt billet
  er ikke i sig selv bevis for fradragsretten.

Samme oplysninger bruges for en eventuel ægtefælle under
`ægtefælle.MedÆgtefælle.fakta`.

Brug ensartede dage pr. række: samme daglige landafstand og samme daglige
billetudgift. Opdel forskellige rejsemønstre i adskilte grupper af dage;
fordel ikke samme dags landtransport og færge på hver sin række. Ellers kan den
daglige bundgrænse eller nulgrænse blive anvendt forkert. Oplys ikke et
gennemsnit, der skjuler dage med billetudgift under bundgrænsen. Årskort og
komplekse rejsekæder kræver en dokumenteret fordeling; modellen udleder ikke
rejsedatoer eller fordeling fra en samlet kvittering.

Rækkernes dagtal må tilsammen ikke overstige indkomstårets 365 eller 366
kalenderdage. Kontrollen gælder hver person særskilt, også ægtefællen.
To rækker med hver 220 dage afvises; modellen fjerner eller flytter ikke dage
for at få dem til at passe. Et lavere samlet antal beviser ikke, at dagene er
adskilte. Afstem grupperne med kørebogen eller andre dokumenterede rejsefakta.

## Et konkret eksempel

Du har 100 ens rejsedage, 16 km landtransport pr. dag og betaler 120 kr. pr. dag
for nødvendig færgetransport. Billetinput er **12.000 kr.** i begge år:

| Beregning | 2025 | 2026 |
| --- | ---: | ---: |
| Manglende km til bundgrænsen | 24 − 16 = 8 | 24 − 16 = 8 |
| Reduktion pr. dag | 8 × 2,23 = 17,84 kr. | 8 × 3,17 = 25,36 kr. |
| Billetfradrag for 100 dage | 10.216 kr. | 9.464 kr. |

Ved 24 km landtransport er bundgrænsen allerede brugt. Ved længere
landtransport lægges kilometerfradraget over 24 km til billetfradraget.
Er billetten billigere end den resterende bundgrænse, bliver billetfradraget
nul, ikke negativt. Uden faktiske rejsedage gives intet billetfradrag.

Beløbene er **fradrag**, ikke skattebesparelse. Et eventuelt ekstra
lavindkomstfradrag beregnes bagefter på årets samlede grundlag. Der sker ikke
en ny 24-km-reduktion i dette tillæg.

## Se beregningen i resultatet

I `ligningsfradrag.befordring.forholdsresultater` bevares de oprindelige `fakta`.
Et beregnet forhold indeholder `ligningslov9c_resultat` med:

- `særlig_transport_resterende_bundgrænse_øre`: den resterende bundgrænse for
  rækkens rejsedage, i øre, når transporten er berettiget.
- `særlig_transportfradrag_øre`: billetfradraget efter reduktion og nulgrænse,
  før modellens eksisterende afrunding til hele kroner.
- `særlig_transportfradrag_kroner` og `afstandsfradrag_kroner`: de anvendte
  delbeløb; kilderne følger med i `særlig_transport_kilder`.

Den resterende bundgrænse kan overstige billetudgiften; den er ikke et negativt
fradrag. Kig også på den samlede
[`vurdering`](personskat-validity.md): et delresultat er ikke tilladelse til
at sammenligne slutskat, hvis andre nødvendige fakta er ugyldige eller uafklarede.

Eksisterende input skal regenereres mod den aktuelle kontrakt og gennemgås.
Hvis du tidligere selv trak bundgrænsen fra billetinput, skal du tilbage til
det dokumenterede beløb, ikke justere det for at opnå et bestemt resultat.

## Kilder og afgrænsning

[LL § 9 C, stk. 1–3](https://www.retsinformation.dk/eli/lta/2025/1500) og
[Den juridiske vejledning 2026-2, C.A.4.3.3.1.3](https://info.skat.dk/data.aspx?oid=2061740)
fastlægger faktisk dokumenteret udgift og den daglige bundgrænse, som først
bruges på landtransporten. De offentliggjorte færge- og flyeksempler er
kontrolgrundlaget; 2025-eksemplet findes også på
[SKATs borgervejledning](https://skat.dk/borger/fradrag/koerselsfradrag/yderligere-information-om-koerselsfradrag).
2026-satsen følger [lov nr. 616 af 30. juni 2026](https://www.retsinformation.dk/eli/lta/2026/616).
Borgervejledningens 2026-beløb var stadig baseret på 2,28 kr. ved kontrollen
25. september 2026; den aktuelle juridiske vejledning anvender 3,17 kr.

Modellen anvender den almindelige sats til bundgrænsen. Det er fortolkningen
af stk. 2 sammenholdt med stk. 3, hvor yderkommunetillægget gælder befordring
**over** 24 km; der er ikke fundet et særskilt officielt regneeksempel for
yderkommunens billetreduktion. Forhøjet sats på fradragsberettiget landtransport
ændres ikke. Årsafrundingen til nærmeste krone er modellens eksisterende
konvention, ikke en ny uafhængig verifikation af SKATs afrundingspraksis.
