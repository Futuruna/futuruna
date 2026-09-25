# Pension: tilbagebetaling og ny indbetaling

En tilbagebetalt pensionsbetaling må ikke tælles som både den oprindelige
indbetaling og en ny indbetaling. Ved en dokumenteret PBL § 22 E-korrektion
kan en genindbetaling desuden høre til det oprindelige betalingsår.
Det er kildefakta og datoer, ikke et frit valg af fradragsår.

Denne forskningsmodel bruger [PBL §§ 18, stk. 1, 19, stk. 1, og 22 E](https://www.retsinformation.dk/eli/lta/2024/1243)
samt [Den juridiske vejledning C.A.10.2.7.2](https://info.skat.dk/data.aspx?oid=2048432).
Lovteksten og typet proveniens ligger i
[§ 22 E-modulet](pensionsbeskatningsloven-par22e.runa).

## Hvad skal bilagene vise?

Bevar den oprindelige betalings id, dato, ordningstype og bruttobeløb;
tilbagebetalingens eget id, dato og bruttobeløb; om pengene faktisk blev
tilbagebetalt til indbetaleren; og datoen for genindbetalingen. Angiv en privat
kildereference, som gør oplysningerne genfindelige. Et ikke-tomt id eller en
reference er ikke dokumentautentifikation. Ordningstypen på den nye betaling
kan godt være forskellig fra den oprindelige.

Beløbene er hele **bruttokroner**, før AM-bidrag. Også tilbagebetaling til en
arbejdsgiver omfatter AM-bidrag, jf.
[indberetningsvejledningens afsnit 4.1](https://info.skat.dk/data.aspx?oid=2288101).
Genindbetalingens eventuelle AM-beløb angives fortsat særskilt på betalingsposten.
Modellen udleder ikke lønkorrektion, tilbageført AM eller arbejdsgiverens egen
skat af disse beløb. Fraktionelle kroner må ikke afrundes uden kildegrundlag.

## Frister og skatteår

- Tilbagebetalingen skal ligge senest 30 kalenderdage efter den oprindelige
  indbetaling og senest 19. januar året efter. Den tidligste af datoerne gælder.
- Genindbetaling inden **samme oprindelige frist** kan beholde det oprindelige
  indbetalingsår. Tilbagebetalingen starter ikke en ny 30-dagesperiode.
- En rettidig tilbagebetaling med senere genindbetaling er ikke i sig selv
  ugyldig. Genindbetalingen følger da almindelige tidsregler.
- Forsikringspræmiers rettidige forfaldsregel har fortsat forrang. Den er ikke
  en generel bankregel; se [betalingsår og forfaldsår](pension-og-fradrag.md#betalingsår-og-forfaldsår).

Modellen kontrollerer sammenhæng og frister ud fra de angivne fakta. Den
autentificerer ikke betalinger eller banklukkedage. Den eksisterende oplysning
om forsikringsbetaling senest den bankjusterede 1. april kræver stadig belæg.
En oplyst genindbetaling i januar kan dog ikke samtidig angives som efter
den følgende aprilfrist; den direkte modstrid afvises.
§ 22 E-ruten gælder oprindelige indbetalinger fra 1. januar 2021. Datoformatets
tekniske grænse er år 9999, med oprindelig betaling senest i år 9998; det er
ikke en lovbestemt udløbsdato. Årsberegningens øvrige årsdækning ændres ikke.

## Eksempel på det nye felt

Dette er et **fiktivt feltudsnit**, ikke en komplet `runa call`-fil. Brug en
frisk JSON-skabelon og dens kontrakt. En bankbetaling på 40.000 kr. den
22. december 2025 tilbagebetales den 5. januar 2026 og genindbetales den
19. januar. På den nye posts `betaling` bevares `betalingsår: 2026`, mens
`hidrører_fra_par22e_tilbagebetaling` sættes til `true`. Feltet
`par22e_genindbetaling` får:

```json
{
  "tilbagebetaling": {
    "identifikation": "tilbagebetaling-1",
    "kildereference": "pensionsbilag, side 1, linje 2-4",
    "oprindelig_indbetaling_identifikation": "oprindelig-betaling-1",
    "oprindelig_ordning": {"$variant": "Pbl22ERateopsparing"},
    "oprindelig_indbetalingsdato": {"år": 2025, "måned": 12, "dag": 22},
    "oprindeligt_indbetalt_brutto_kroner": 40000,
    "tilbagebetalingsdato": {"år": 2026, "måned": 1, "dag": 5},
    "tilbagebetalt_brutto_kroner": 40000,
    "tilbagebetalt_til_indbetaleren": true
  },
  "genindbetalingsdato": {"år": 2026, "måned": 1, "dag": 19}
}
```

Fristen er 19. januar 2026. Bankpostens skattemæssige indbetalingsår bliver
2025, uden at bilagets betalingsår omskrives. Hvis kun genindbetalingen flyttes
til 20. januar, bliver bankpostens år 2026. Et samlet fradrag afhænger stadig
af de øvrige betingelser og fælles årsgrænser.

§ 18's fradragsår er særskilt fra faktisk betaling: de eksisterende § 15 A-
kontroller af oprettelse, indbetalingsperiode og seneste indbetaling omskrives
ikke til et andet faktisk år. Dato-/årstesten er ikke en ny fuld konformitetstest
af kombinationer med ophørspension eller sportspension.

Almindelige betalinger bruger `false` og `par22e_genindbetaling: null`.
`true` uden fakta, `false` med fakta, ukendt oprindelig ordning, ugyldige datoer,
forkert faktisk betalingsår og tilbagebetaling efter fristen tilbageholder
årsberegningens sammenligningsbeløb. Manglende fakta må ikke omdøbes til en
almindelig betaling for at få beregningen til at køre.

## Delte beløb og afgrænsning

Ved flere genindbetalinger fra samme tilbagebetaling gentages dens **samme id
og samme fakta**. Deres samlede brutto må ikke overstige tilbagebetalingen.
Flere tilbagebetalinger fra samme oprindelige betaling skal have konsistente
oprindelige fakta og må samlet ikke overstige dens brutto.
Hvis den oprindelige post også indgår i årslisten, bruger den det oprindelige
id og kun den ikke-tilbagebetalte rest. En helt tilbagebetalt post må ikke
bevares med sit gamle fradragsbeløb. Kontrollen kan ikke finde samme bilag
skjult under andre id'er eller korrektioner udeladt fra input. Beløbsdelingen
kontrolleres inden for den enkelte persons årsliste, ikke mellem særskilte
personers beregninger.

Ruten er ikke § 22 D-overførsel, § 22 F-fejlkorrektion eller en rettelse af en
forkert registrering uden faktisk tilbagebetaling. Overskydende udbetaling af
afkast, kæder hvor en genindbetaling selv tilbagebetales igen, og særskilt
godkendt tidligere bankår kræver anden behandling; de er ikke implementeret
her. Afklar og hold en uunderstøttet sag tilbage i stedet for at tilpasse fakta.
Modellen giver ikke tilladelse til at hæve en pension og afgør ikke civilretlig
ret til tilbagebetaling.

## Migration og efterprøvning

Typede beregninger er Preview. Nye typer og metadata ændrer kontrakthashen:
generer frisk schema og skabelon, overfør kun gennemgåede kildefakta og beregn
berørte resultater igen. I `.runa`-konstruktører af `Pbl18Betaling` og
`Pbl18Årsbetaling` tilføjes det nye valgfrie felt, normalt `None`.
Udeladt valgfrit felt i JSON betyder fravær, ikke dokumenteret korrektion.

[Datogrænserne](../../tests/personskat_pension_redeposit_test.runa) og
[den kanoniske regression](../../tests/personskat_pension_redeposit.test.mjs)
bruger fiktive kilder og kontrollerer også fælles vejledning for hovedperson
og ægtefælle. De er ikke nye uafhængige SKAT-observationer eller bevis for
vilkårlige AI'ers læsning af pensionsbilag.
