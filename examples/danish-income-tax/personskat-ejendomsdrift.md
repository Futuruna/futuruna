# Ejendomsresultat: ikke kapitalindkomst betyder ikke skattefrit

`kapitalindkomst.ejendomsdrift` er en afgrænset indgang til personskattelovens
§ 4, stk. 1, nr. 6. Den er **ikke** en fælles indgang til alle lejeindtægter.
Futuruna bruger ejendomskategori, beliggenhed, anvendelse og årets
skattemæssige driftsresultat; et menneske eller en AI skal underbygge fakta.

## Når den valgte regel ikke omfatter beløbet

Komponenten bevarer det oprindelige beløb i
`ejendomsdrift_resultat.par4_resultat.ikke_omfattet_af_par4_nr6_kroner`.
Et ikke-nul beløb dér tilbageholder den kanoniske årsberegnings sammenligning:

- `vurdering.slutskat_til_sammenligning_øre` bliver `null`.
- `vurdering.fejl` peger på `kapitalindkomst.ejendomsdrift`, eller samme sti
  under en beregnet ægtefælles fakta.
- Kapitalgrundlagets `alle_input_gyldige` bliver falsk. En særskilt
  delårsberegning bevarer grænsen i både perioden og et dokumenteret helår,
  også ved en aktiv ægtefælle.

Det gælder både overskud og underskud. Et bekræftet nulresultat udløser ikke
denne rutefejl; det beviser ikke, at øvrige ejendomsforhold er fuldstændige.
Komponentens lovklassifikation og beløb ændres ikke. Andre beløb i et ugyldigt
resultat er kun diagnostik, ikke brugbare skattebeløb.

Et fiktivt kontrolforløb med 600.000 kr. i løn og 20.000 kr. i erhvervsmæssigt
udlejet ejerboligs overskud gennem denne gren tilbageholder sammenligningen.
Et overskud, der ikke omfattes af kapitalindkomstreglen, må ikke forsvinde
fra årsberegningen; en anden indgang kræver sine egne kildefakta.

## Vælg indgang ud fra kilden

[Skattestyrelsens vejledning](https://skat.dk/borger/bolig-og-ejendomme/udlejning-af-bolig/du-udlejer-en-bolig-som-du-ikke-selv-bor-i)
forklarer, at erhvervsmæssig udlejning af en ejerbolig som udgangspunkt giver
personlig indkomst med AM-bidrag. Fritagelse for ejendomsværdiskat er ikke
fritagelse for indkomstskat. Leje-/andelsboliger og tofamilieejendomme kan have
andre regler; brug ikke én blanketklassifikation på alle boliger.

Ved dokumenteret almindelig virksomhed findes indgangen
`lønmodtager.personlig_indkomst.ordinære_forhold.virksomheder_uden_virksomhedsordning`.
Den kræver sine egne indtægts-, udgifts- og ordningsfakta. Afklar dem før
omplacering; det enkelte nettobeløb er ikke tilstrækkeligt til automatisk
at udfylde alle felterne. Kontrollér særskilt renter, ejendomsskatter og
eventuelle andre ordninger. Samme indkomst må kun medregnes én gang.

Et særskilt fiktivt kontrolforløb oplyser 30.000 kr. i leje og 10.000 kr. i
afklarede fradragsberettigede driftsudgifter, almindelig virksomhed uden
virksomhedsordningen og ingen renter eller andre ejendomsskatter i eksemplet.
Med den samme løn på 600.000 kr. giver den eksisterende virksomhedsindgang
220.078,38 kr. i modelleret samlet skat, heraf 49.600 kr. i AM. Overskuddet
på 20.000 kr. bliver altså medregnet. Det er modeloutput, ikke et nyt match
med en officiel beregner; opdelingen 30.000/10.000 må ikke gættes ud fra et
virkeligt nettobeløb.

Skift ikke ejendomskategori eller anvendelse, og skriv ikke nul, for blot at
få en sammenligning. Opgør ikke selv en ny fradragsret ud fra komponentens
udelukkelse. Ved manglende grundlag kan
[betinget rapportafstemning](aarsopgoerelse-afstemning.md) stadig besvare
snævrere spørgsmål uden en uafhængig fuld genberegning.

## Kilder, kontrakt og afgrænsning

Den eksisterende kildemodel i [kapitel 1](kapitel-01-indkomst.runa) bevarer
PSL § 4, stk. 1, nr. 6, og henvisningen til ejendomsskattelovens § 3,
herunder ændringslovene og den tidsafhængige nummerering. Typede
feltbeskrivelser følger ejendomsgrenen i både hovedpersonens og ægtefællens
kontrakt med lovkilder, vejledning og ruteforbehold.

Generér schema/skabelon fra den aktuelle model, og følg
[migrationsvejledningen](../../docs/compatibility-guides/0.2.x.md)
ved genbrug af tidligere sager. Bevar oprindelige kildefakta privat.
De [fokuserede tests](../../tests/personskat_property_route.test.mjs)
er model- og indgangskontroller med opdigtede fakta, ikke dokumentautentifikation,
uafhængig officiel skatteberegning eller fuld dækning af udlejning. Typede
`@ calculate`-kontrakter er fortsat Preview; skattemodellen er forskningssoftware.
