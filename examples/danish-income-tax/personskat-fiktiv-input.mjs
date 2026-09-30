// Fills the remaining `{"$fill": ...}` placeholders of a Personskat template
// for one invented person: a Danish resident employee with no property,
// capital income, business, shares, losses, special regimes or other
// deductions. Every choice below is a stated fact about that fictional person
// only; it is never an intake default for real tax documents. Lists that
// remain placeholders are declared empty for the same reason. Any placeholder
// not covered here fails the fixture instead of being guessed.
const v = ($variant, fields = {}) => ({ $variant, ...fields });

const facts = {
  'aktieavance.ordinært_aktieår': v('UdenOrdinærtAktieår'),
  'ejendomsskatter.person.egen_udbytteindkomst_kroner': 0,
  'ejendomsskatter.person.ægtefælles_udbytteindkomst_kroner': 0,
  'ejendomsskatter.person.samlevende_ægtefælles_folkepensionsalder': v('EjskFolkepensionsalderIkkeOpnået'),
  'ejendomsskatter.person.skattemæssigt_hjemsted': v('EjskFuldtSkattepligtigEfterKildeskattelovensPar1'),
  'kapitalindkomst.ejendomsavance': v('UdenEjendomsavance'),
  'kapitalindkomst.ejendomsdrift': v('UdenEjendomsdriftEfterPar4Nr6'),
  'kapitalindkomst.fremleje': v('UdenFremlejeEfterLigningslov15Q'),
  'kapitalindkomst.kursgevinst': v('UdenKursgevinst'),
  'kapitalindkomst.renter.ligningslov6': v('UdenLigningslov6Kurstab'),
  'kapitalindkomst.renter.ligningslov6a': v('UdenLigningslov6AFradrag'),
  'kapitalindkomst.renter.næringsstatus': v('IkkeNæring'),
  'kapitalindkomst.renter.renteindtægter_kroner': 0,
  'kapitalindkomst.renter.renteudgifter_kroner': 0,
  'kapitalindkomst.virksomhedskapital.medarbejderaktier': v('UdenKapitalafkastEfterVirksomhedsskattelov22C'),
  'kapitalindkomst.virksomhedskapital.selvstændig_arbejdsmarkedsbidrag': v('AmblPar4UdenVirksomhedsordning', { fremført_negativ_personlig_indkomst: [] }),
  'kapitalindkomst.virksomhedskapital.selvstændig_beskatningsordning': v('UdenVirksomhedsEllerKapitalafkastordning'),
  'ligningslov33.hovedperson': v('UdenLigningslov33'),
  'ligningslov33.ægtefælle': v('UdenLigningslov33'),
  'ligningslov33.ligningslov33a_hovedperson': v('UdenLigningslov33A'),
  'ligningslov33.ligningslov33a_ægtefælle': v('UdenLigningslov33A'),
  'lønmodtager.ligningsfradrag.arbejdsløshed_efterløn_og_fleksydelse.skattepligtsposition': v('Pbl49FuldtSkattepligtigOgHjemmehørendeIDanmark'),
  'lønmodtager.ligningsfradrag.faglige_kontingenter.skatteyderstatus': v('Ll13Lønmodtager'),
  'lønmodtager.ligningsfradrag.fiskerfradrag.valg': v('Ll9GFravælgFiskerfradrag'),
  'lønmodtager.ligningsfradrag.rejser.dobbelt_husførelse': v('Ll9AIntetFradragForDobbeltHusførelse'),
  'lønmodtager.ligningsfradrag.rejser.personrolle': v('Ll9AAlmindeligLønmodtager'),
  'lønmodtager.ligningsfradrag.rejser.ølogi': v('UdenØlogifradrag'),
  'lønmodtager.ligningsfradrag.sømandsfradrag.valg': v('FravælgSømandsfradrag'),
  'lønmodtager.ligningsfradrag.øvrige_lønmodtagerudgifter.skatteyderstatus': v('Ll9Stk1Lønmodtager'),
  'lønmodtager.pension.aktiepensionsfradrag_valg': v('UdenAktiepensionsfradragIAktieindkomst'),
  'lønmodtager.pension.pbl18_livrentevalg': v('Pbl18FordeltFradrag'),
  'lønmodtager.pension.pbl18_selvstændig_overskud.kursgevinster_kroner': 0,
  'lønmodtager.pension.pbl18_selvstændig_overskud.kurstab_kroner': 0,
  'lønmodtager.pension.pbl18_selvstændig_overskud.renteindtægter_kroner': 0,
  'lønmodtager.pension.pbl18_selvstændig_overskud.renteudgifter_kroner': 0,
  'lønmodtager.pension.pbl18_selvstændig_overskud.skattepligtigt_overskud_før_vsl22b_kroner': 0,
  'lønmodtager.pension.pbl18_selvstændig_overskud.udbytteindtægter_kroner': 0,
  'lønmodtager.pension.pbl18_selvstændig_overskud.udelukkede_afståelsesindkomster_kroner': 0,
  'lønmodtager.personfradrag_alder_status': v('Fyldt18EllerGift'),
  'lønmodtager.personlig_indkomst.etableringskonto': v('UdenEtableringskontoindskud'),
  'lønmodtager.personlig_indkomst.sømandsbeskatning.dødsboskattegrundlag': v('Søbl5IntetDødsboskattegrundlag'),
  'lønmodtager.personlig_indkomst.sømandsbeskatning.kulbrinteskattegrundlag': v('Søbl5BIntetKulbrinteskattegrundlag'),
  'negativ_aktieskat_fremførsel.hovedperson': v('UdenFremførtNegativAktieskat'),
  'negativ_aktieskat_fremførsel.ægtefælle': v('UdenFremførtNegativAktieskat'),
  'skatteforhold': v('StandardSkatteforhold'),
  'udenlandske_sociale_bidrag': v('UdenUdenlandskeSocialeBidragEfterLigningslov8M'),
  'underskudsforhold': v('StandardUnderskudsforhold'),
  'årsopgørelse': v('UdenÅrsopgørelse'),
};

const isPlaceholder = (value) => value !== null && typeof value === 'object'
  && !Array.isArray(value) && Object.hasOwn(value, '$fill');

// Replaces placeholders in `personskat` in place. The caller fills the facts it
// varies (year, salary, municipality, birth date, spouse, ...) first and states
// the person's own folkepensionsalder, which follows from the birth date.
// `extra` states further facts by path relative to `personskat`.
export function fillFictionalPersonskat(personskat, { folkepensionsalder, extra = {} }) {
  const own = { ...facts, 'ejendomsskatter.person.ejer_folkepensionsalder': folkepensionsalder, ...extra };
  const unfilled = [];
  const walk = (node, path) => {
    for (const [key, value] of Object.entries(node)) {
      const at = path ? `${path}.${key}` : key;
      if (isPlaceholder(value)) {
        if (Object.hasOwn(own, at)) node[key] = structuredClone(own[at]);
        else if (value.$fill.startsWith('List(')) node[key] = [];
        else unfilled.push(`${at}: ${value.$fill}`);
      } else if (value !== null && typeof value === 'object') walk(value, at);
    }
  };
  walk(personskat, '');
  if (unfilled.length) throw new Error(`fictional Personskat fixture leaves unfilled: ${unfilled.join('; ')}`);
  return personskat;
}
