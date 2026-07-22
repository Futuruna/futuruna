# Research Log: The Danish Constitution in Futuruna

## Session 1: Chapters I-II + Adversarial Analysis

### What we built
- `grundlov.runa` — Foundational types and preamble metadata
- `kapitel-01.runa` — Chapter I: Form of State (§§ 1-4), 4 paragraphs, 0 exceptions
- `kapitel-02.runa` — Chapter II: The King (§§ 5-11), conditions, consent-gating, fallback chains
- `analyse.runa` — Adversarial review: 10 issues, 4 funny consequences
- `den-muslimske-konge.runa` — The Muslim King Paradox: a constitutional state machine bug

### Compiler improvements made along the way
- **Cross-file module imports now work** — originally via `@ use grundlov::*`; prefer `@ import ./grundlov` now that `@ use` is reserved for Rust imports
- **`Stmt::For` loop support** — added interpreter eval + codegen for `for` loops
- **`Expr::Handle` match arms** — added exhaustiveness for effect handler expressions

### Key findings

**Constitutional patterns discovered:**

| Pattern | Count | Examples |
|---------|-------|---------|
| SKAL (absolute) | 6 | §§ 1-4, 6-8 |
| KAN IKKE (unbreakable) | 1 | § 10.2 (debt prohibition) |
| KAN IKKE + samtykke | 2 | §§ 5, 11 |
| FASTSAETTES (delegated) | 1 | § 9 |
| Fallback chains | 1 | § 8 (oath → absence → Council → special law) |

**The Muslim King Paradox — a genuine type error in the Grundlov:**
- § 6 says "Kongen" must be Lutheran — does NOT extend to Troelfoelger
- § 7 explicitly extends age requirement to Troelfoelger with "det samme gaelder"
- § 6 has no such extension clause — deliberate or oversight
- § 8 stk. 4 fires automatically for sworn heirs — no religious check
- A Muslim heir can legally swear the oath, accede, and violate § 6
- Every transition is legal. The resulting state is illegal.
- State machine bug: transition rule doesn't check state invariant
- No removal mechanism in §§ 1-11 for this scenario

**Futuruna's `under` keyword would fix it:**
```tau
> tiltraed(arving: Troelfoelger) -> Monark
    under arving.trossamfund == EvangeliskLuthersk,
          arving.alder >= 18,
          arving.forsikring == AfgivetSomTroelfoelger
```
Preconditions on transitions vs invariants on states — Futuruna makes this distinction explicit.

**Other notable findings:**
- The King is constitutionally the least free Dane
- The Folkekirke has zero power but constrains the monarchy (paradox)
- § 10.2 is the hardest sentence — no exception mechanism exists
- § 9 gives Folketinget nuclear option: elect king + rewrite succession
- If the Evangelical Lutheran Church ceased to exist, the monarchy becomes logically impossible
- § 1 is open-world ("alle dele") but our Rigsdel type is closed — modeling gap
- Binary gender assumption (1953) revealed by Koen = Mand | Kvinde

### Language observations for legal encoding
- Default logic (`| rule`, `| exception`) maps perfectly to legal defaults + overrides
- ADTs map to legal entities and institutions
- Named fields map to legal properties
- `under` conditions map to legal prerequisites
- Closed-world assumption (what's not stated is not granted) matches legal interpretation
- Missing: multi-file module system (now partially working), date literals, legal assertion syntax
- Want: `@ § 6` paragraph metadata annotations, completeness checking

---

## Session 1 (cont.): Chapter III

### What we built
- `kapitel-03.runa` — Chapter III: The King and Ministers (§§ 12-27), 151 statements, 21 functions, 19 types

### Key findings

**The countersignature rule (§ 14) — the heart of Danish democracy:**
- King's signature alone = invalid (no legal force)
- Minister's signature alone = invalid (no royal authority)
- BOTH together = valid, minister bears ALL responsibility
- Creates a deadlock-free mutual dependency: neither can act alone
- This single mechanism transforms formal monarchy into parliamentary democracy

**Power flow chain (§§ 12→13→14→15):**

| Step | Paragraph | Mechanism |
|------|-----------|-----------|
| 1 | § 12 | King has formal supreme authority |
| 2 | § 12 | But exercises it THROUGH ministers |
| 3 | § 13 | King is ansvarsfri; ministers bear responsibility |
| 4 | § 14 | King's signature only valid WITH minister's countersignature |
| 5 | § 15 | Folketinget can remove ministers (no confidence) |

Result: Folketing controls ministers → ministers control the King's actions → formal supremacy is ceremonial → **parliamentary democracy in monarchical clothing**.

**Samtykke (consent) pattern — now tallied across all 27 paragraphs (7 occurrences):**

| § | Subject | Consent of |
|---|---------|------------|
| 5 | Regent abroad | Folketinget |
| 11 | Allowance abroad | Folketinget |
| 19 | Territory/obligations/significance/termination | Folketinget |
| 19 | Military force | Folketinget |
| 20 | Sovereignty delegation | 5/6 flertal or referendum |
| 24 | Pardon Rigsret-convicted minister | Folketinget |
| 27 | Transfer civil servant | The civil servant |

**Self-defense (§ 19 stk. 2) is the ONLY action that never requires consent.** Denmark can go to war without asking anyone first. It can do almost nothing else.

**§ 20 has the highest threshold in the entire constitution:** 5/6 majority (150/179 seats) for sovereignty delegation. Even constitutional amendments (§ 88) don't require 5/6.

**Other notable findings:**
- § 22 has a 30-day deadline for royal assent — but no consequence for missing it (gap)
- § 23 emergency decrees must NOT conflict with the constitution (hard constraint)
- § 16: King can independently impeach ministers at Rigsretten — one of his few genuine powers
- § 18: King has effective veto over Ministerraad decisions (can demand full Statsraadet)
- § 13 "fredhellig" (sacrosanct) means the King literally cannot be prosecuted, sued, or held accountable — the cost is total dependence on willing ministers

### New patterns discovered in Chapter III

| Pattern | Count | Examples |
|---------|-------|---------|
| Countersignature (mutual dependency) | 1 | § 14 |
| Samtykke (consent-gating) | 4 new | §§ 19, 20, 24, 27 |
| Hard constraint (KAN IKKE) | 1 | § 23 (no conflict with constitution) |
| Deadline | 1 | § 22 (30 days for royal assent) |
| Escape valve | 1 | § 15 (PM chooses resign vs. election) |
| Delegation to law | 2 | §§ 13 (ministeransvar), 27 (tjenestemænd) |

---

## Session 1 (cont.): Chapter IV

### What we built
- `kapitel-04.runa` — Chapter IV: Folketinget (§§ 28-34), 115 statements, 16 functions, 12 types

### Compiler fix
- **`Stmt::MonadicBind` match arms** — new variant added to `Stmt` enum since last compile; added handling in interpreter, `stmt_contains_try`, and `emit_stmt`

### Key findings

**The democratic pipeline — a dependency chain:**
```
§ 29 (valgret) ← § 30 (valgbarhed) ← § 32 (mandat) ← § 33 (validering)
```
Eligibility REQUIRES suffrage. A single gate controls both voting and candidacy. Every restriction on voting automatically restricts who can be elected.

**Constitutional locks — things that cannot be changed by ordinary law:**
- Voting age: requires referendum (§ 29 stk. 2)
- Proportional representation: constitutionally mandated (§ 31 stk. 2)
- Mandate continuity: mandates never expire before new election held (§ 32 stk. 4)
- Folketing inviolability: attacking it = high treason (§ 34)
- Faroe/Greenland 2+2 seats: constitutionally guaranteed (§ 28)

**§ 34 anticipated the Nuremberg principle:**
Three categories of high treason — the attacker, the commander, AND the one who obeys. "Just following orders" is explicitly NOT a defense. Written in 1849, reaffirmed 1953 — before the Nuremberg trials established this in international law.

**The oath symmetry:**
- § 8: The King swears to uphold the constitution
- § 32 stk. 7: Each new MP swears to uphold the constitution
- Monarch and parliamentarian make the SAME promise. The constitution binds all its actors to itself.

**§ 33 — Folketing as judge in its own case:**
Folketing alone judges the validity of its members' elections and eligibility. No court can overrule. This is an exception to separation of powers — and a necessary one, since any external body with power over Folketing's composition would be above the legislature.

**The dissolution paradox (§ 32 stk. 2):**
King can dissolve Folketing at any time. But King acts through ministers (§ 12). New PM must face Folketing first (§ 32 stk. 2). So: PM appointed → must face Folketing → THEN can dissolve. Creates a mandatory minimum of one parliamentary encounter.

**The Greenland/Faroe pattern (recurring):**
- § 1: "alle dele af riget" (open world)
- § 28: 2+2 seats (constitutionally fixed)
- § 31 stk. 5: Special rules for Greenland by law
- § 32 stk. 5: Special rules for mandate timing
- Pattern: constitutional anchor + legislative delegation

---

## Session 1 (cont.): Adversarial Analysis II

### What we built
- `analyse-2.runa` — Cross-cutting adversarial review of Chapters I-IV, 352 statements, 9 functions, 10 types

### Fidelity issues (8 found)

| # | Issue | Severity | Chapter |
|---|-------|----------|---------|
| F1 | Chapter I doesn't use `@ use` imports — types duplicated | Strukturel | I |
| F2 | Institution type redefined per chapter — incompatible | Strukturel | I-IV |
| F3 | Chapter III missing §§ 21, 25, 26 | Meningsaendrende | III |
| F4 | § 14 countersignature: single String, should be List | Strukturel | III |
| F5 | § 19 military: Forsvar/Angreb binary loses directionality | Strukturel | III |
| F6 | § 29 should use default logic (medmindre = exception) | Strukturel | IV |
| F7 | Voting age hardcoded as 18, text says "referendum result" | Kosmetisk | IV |
| F8 | § 27 transfer: missing pension choice alternative | Strukturel | III |

F2 is the most interesting — reveals a real need for extensible/open types in Futuruna. F6 is a missed opportunity: the text literally uses "medmindre" (unless), which IS Futuruna's default logic pattern.

### Constitutional insights (10 discovered)

**Three worth expanding into standalone analyses:**

1. **The Fredhellig Paradox** — If the King attacks Folketing, § 13 (sacrosanct, unprosecutable) and § 34 (attack = high treason) both apply simultaneously. No resolution mechanism. The person § 34 was designed to constrain is the same person § 13 makes untouchable. Comparable in depth to the Muslim King paradox.

2. **The Emergency Decree Perverse Incentive** — § 34 says preventing Folketing from assembling is treason. § 23 says if Folketing can't assemble, emergency decrees are legal. Treason CREATES the conditions for legal emergency powers. The decrees don't check how the emergency arose.

3. **The Threshold Hierarchy** — 6 levels of constitutional hardness discovered:
   - Level 0: No vote (self-defense § 19, pardons § 24)
   - Level 1: Simple majority (samtykke pattern, 5 instances)
   - Level 2: 5/6 supermajority (sovereignty delegation § 20)
   - Level 3: Referendum (voting age § 29, sovereignty fallback § 20)
   - Level 4: Amendment procedure (§ 88, not yet encoded)
   - Level 5: Impossible (§ 10.2 debt, § 34 inviolability, § 13 sacrosanctity)
   - §§ 10.2, 13, and 34 are HARDER than constitutional amendment — no exception mechanism exists.

**Other insights:**
4. Grand Paradox: King holds all power (§§ 3,12) but can exercise none (§ 14)
5. Accountability Inversion: power and responsibility inversely distributed (§§ 13,14)
6. Self-Sealing Exclusion: Folketing defines "uvaerdig", applies it, judges the outcome (§§ 30,33)
7. Non-Lutheran Parliament: no religious requirement for MPs or ministers — § 6 constrains only the figurehead
8. Dissolution Infinite Loop: no circuit breaker for repeated no-confidence + election cycles
9. Oath Web: King, civil servants, and MPs all swear to the document, not to each other
10. The constitution IS the social contract — the sole mutual commitment point

---

## Session 1 (cont.): Chapter V

### What we built
- `kapitel-05.runa` — Chapter V: Folketingets virksomhed (§§ 35-58), 213 statements, 27 functions, 15 types

### Key findings

**The complete legislative pipeline (§§ 21, 41, 42, 22):**
1. INITIATION: Government or any MP introduces bill (§ 21/41)
2. THREE READINGS: 1st (general) → 2nd (detail) → 3rd (final vote) (§ 41.2)
3. DELAY: 2/5 (72 MPs) can force 12-day pause between 2nd and 3rd (§ 41.3)
4. REFERENDUM WINDOW: 1/3 (60 MPs) can demand referendum within 3 days (§ 42.1)
5. ROYAL ASSENT: Within 30 days (§ 22)
6. BECOMES LAW

**§ 42 (referendum) is the most complex paragraph in the constitution:**
- 8 subsections, a complete state machine
- Double rejection threshold: majority of voters AND 30% of all eligible voters
- ENORMOUS exemption list: finance, tax, citizenship, expropriation, treaties, §§ 8-11 laws, § 19 decisions
- Most significant legislation is exempt — the referendum is a veto on ordinary legislation only

**The minority protection architecture:**

| Threshold | MPs needed | Power |
|-----------|-----------|-------|
| 1/3 (60) | Demand referendum (§ 42.1) |
| 2/5 (72) | Force 12-day delay (§ 41.3) |
| 2/5 (72) | Demand emergency session (§ 39) |
| 50%+ (90) | Quorum for any decision (§ 50) |

**Financial supremacy (§§ 43-47) — Folketing's absolute control:**
- No tax, conscription, or state loan without law (§ 43)
- No tax collection before finance law passes (§ 46)
- No expenditure without authorization (§ 46)
- State accounts + Folketing-elected auditors (§ 47)

**Tensions discovered:**
- § 31 (party-list PR) vs § 56 (free mandate) — parties get seats but members are constitutionally free of all instructions, including their party's
- § 42 referendum exemptions effectively gut the mechanism for important legislation
- § 42.5 double threshold makes rejection deliberately difficult — protection against small minorities
- § 55 ombudsmand: Folketing's watchdog who must NOT be an MP — controls without participating

**New patterns:**
- Clean slate rule (§ 41.4): all pending legislation dies at election/session end
- Emergency assembly (§ 39): 2/5 can force session — Speaker cannot block
- Voice without vote (§ 40): non-MP ministers can speak but not vote
- Fersk gerning (§ 57): caught in the act breaks parliamentary immunity

**Running totals after 5 chapters:**
- 58 of 89 paragraphs encoded (§§ 1-58)
- Files: grundlov.runa, kapitel-01 through 05, analyse, analyse-2, den-muslimske-konge
- Compiler fixes: `@ use` imports, `Stmt::For`, `Expr::Handle`, `Stmt::MonadicBind`

---

## Session 1 (cont.): Chapter VI

### What we built
- `kapitel-06.runa` — Chapter VI: Domstolene (§§ 59-65), 143 statements, 18 functions, 8 types

### Compiler fixes
- **`Stmt::Import` and `Stmt::Depend` match arms** — new variants added to `Stmt` enum; added handling in `emit_stmt`
- **`RustCodegen` new fields** — `cargo_deps`, `source_dir`, `imported` added to `new()` initializer

### Key findings

**The constitutional circle is now COMPLETE:**
```
§ 3:    Powers are separated
§§ 14-15: Folketing → Ministers → King
§ 63:   Courts control EVERYONE
§ 64:   Nobody controls the courts
= Courts are the constitutional apex
```

**§ 63 — Judicial review:** Courts can rule on ANY question about the limits of government authority. Combined with § 64 (judicial independence), this creates an institution that is separate from government (§ 62), can overrule government (§ 63), and cannot be punished by government (§ 64). The constitutional firewall.

**The § 33 vs § 63 tension:** § 33 says Folketing judges its own members' eligibility (no external review). § 63 says courts can review "ethvert spoergsmaal" (ANY question) about authority limits. Can courts review § 33 decisions? Still constitutionally unresolved.

**Rigsretten — the only hybrid institution:** Up to 15 Supreme Court judges + 15 Folketing-elected members. MPs explicitly excluded (separation). Balance actively maintained: if judges can't participate, equal number of elected members step down. The only institution bridging legislative and judicial branches.

**Income protection pattern (newly discovered):**
- § 10.2: State payment cannot carry debt
- § 27: Civil servants transferred without consent → no income loss
- § 64: Judges retired at 65 → no income loss ("uden tab af indtaegter")
- The constitution consistently protects income when limiting power.

**Three modes of power:**
- Active: Folketing (legislation) + Government (execution)
- Passive: Courts (review — wait for cases, then rule)
- Ceremonial: King (signature)

**Running totals after 6 chapters:**
- 65 of 89 paragraphs encoded (§§ 1-65)
- Samtykke count: 8 instances (§§ 5, 11, 19×2, 20, 24, 27, 60)
- Compiler fixes total: 6 (`@ use`, `For`, `Handle`, `MonadicBind`, `Import`/`Depend`, `RustCodegen` fields)

---

## Session 2: Chapter VII

### What we built
- `kapitel-07.runa` — Chapter VII: Folkekirken og religionsfriheden (§§ 66-70), 187 statements, 19 functions, 7 types, 5 rules

### Compiler fix
- **Rule syntax: typed parameters not supported** — `| rule(p: Person)` fails at the `:`. Rules use value arguments, not typed declarations. Fixed by using parameterless rules with helper functions.

### Key findings

**The religion pyramid — five layers discovered across the entire constitution:**

| Layer | Chapter | Content |
|-------|---------|---------|
| 1 (Identity) | I, § 4 | Folkekirken IS Evangelical Lutheran, state-supported |
| 2 (Constraint) | II, § 6 | King MUST be Evangelical Lutheran |
| 3 (Delegation) | VII, §§ 66, 69 | Church constitution + other denominations by ordinary law |
| 4 (Freedom) | VII, §§ 67, 68 | Freedom of religion + no contribution obligation |
| 5 (Equality) | VII, § 70 | No discrimination based on creed or descent |

The pyramid inverts power: the most powerful position (King) is the MOST religiously constrained; the ordinary citizen is the MOST free. Same accountability inversion as Chapter III (power ≠ freedom).

**§ 6 vs § 70 — the constitution contradicts itself:**
- § 6: King MUST be Lutheran (religious requirement for office)
- § 70: NOBODY shall be denied political rights based on creed
- § 70 says "ingen" (nobody) — no exception stated
- Resolution requires lex specialis (specific beats general) — a META-RULE not found anywhere in the constitutional text
- In a type-safe constitution, this would be a compile-time error
- Deepens the Muslim King paradox: the heir has FULL political rights (§ 70) but cannot be King (§ 6)

**§ 66 — the 177-year broken promise:**
- 1849: "Folkekirkens forfatning ordnes ved lov" — promised a church constitution
- 2026: never enacted. The longest unfulfilled constitutional mandate in Danish history
- The Folkekirke's entire governance is delegated to ordinary law (simple majority)
- Less constitutional protection than the voting age (which requires referendum per § 29)

**§ 67 — the constitution's ONLY morality clause:**
- "dog at intet læres eller foretages, som strider mod sædeligheden eller den offentlige orden"
- "Sædeligheden" (morality) appears NOWHERE else in §§ 1-89
- The constitution normally speaks in structures and procedures, never moral terms
- § 67 is the sole exception: morality enters the constitution through religion
- Uses default logic in the text itself: "dog at" = Futuruna's `exception`

**§ 68 vs § 4 — the church tax loophole:**
- § 4: State MUST support the Folkekirke
- § 68: Nobody obligated to contribute to others' worship ("personlige bidrag")
- Kirkeskat (church tax): only members pay → § 68 compliant
- Bloktilskud (general tax subsidy): ALL taxpayers fund → boundary undefined
- "Personlige bidrag" vs "state support" is a constitutional gray zone

**§ 66 vs § 69 — the asymmetric delegation:**
- § 66: Folkekirkens FORFATNING (internal governance structure)
- § 69: Andre trossamfunds FORHOLD (external relationship to state)
- The state church gets self-governance. Others get regulation.
- Both delegated to Folketing, but with different SCOPE.

**§ 70's double edge:**
- Clause 1: cannot be DENIED rights based on creed/descent
- Clause 2: cannot EVADE duties based on creed/descent
- Religion is neither shield nor sword

**New patterns:**
- ZERO new samtykke instances in Chapter VII — the ONLY chapter besides Chapter I with no consent-gating. Religious freedom is unconditional.
- The HARD/SOFT distinction: the Folkekirke's existence is constitutionally HARD (§ 4, requires § 88 amendment). Its governance is constitutionally SOFT (§ 66, changeable by simple majority).

**Running totals after 7 chapters:**
- 70 of 89 paragraphs encoded (§§ 1-70)
- Samtykke count: still 8 instances (no new ones in Chapter VII)
- Compiler fixes total: 7 (added: rule parameter syntax)

---

## Session 2 (cont.): Chapter VIII

### What we built
- `kapitel-08.runa` — Chapter VIII: Borgernes grundrettigheder (§§ 71-85), 353 statements, 35 functions, 13 types, 6 rules

### Key findings

**The rights hierarchy — four modal levels discovered:**

| Level | Modal | Paragraphs | Meaning |
|-------|-------|-----------|---------|
| 1 | "ukrænkelig" | §§ 71, 72, 73 | Inviolable — cannot be touched |
| 2 | "ret til" / "berettiget" | §§ 76, 77, 78, 79 | Positive right — you HAVE it |
| 3 | "skal" / "er afskaffet" | §§ 74, 83, 84 | Command — the state MUST |
| 4 | "bør tilstræbes" | § 75.1 | Aspiration — should be tried |

The constitution GRADUATES its promises. "Ukrænkelig" is absolute. "Bør tilstræbes" is a hope.

**§ 77 "ingensinde påny" vs § 88 — the deepest paradox:**
- § 77: Censorship can NEVER AGAIN be introduced ("ingensinde påny")
- § 88: Any constitutional provision can be amended
- Can you amend "never"? The unstoppable force (amendment power) meets the immovable object (eternal prohibition)
- This is hardness Level 5 — harder than constitutional amendment itself
- The constitution's only promise about the infinite future

**§ 73 — the MOST PROTECTED right (5 layers):**
1. Requires law (not executive decree)
2. Requires full compensation
3. Requires public good
4. 1/3 minority (60 MPs) can force new election before passage
5. Full judicial review of both legality AND compensation amount
No other right in the constitution has 5 layers of protection.

**§ 85 — the military hierarchy WITHIN rights:**
- CAN be limited for military: §§ 71 (liberty), 78 (association), 79 (assembly)
- CANNOT be limited: §§ 72 (home), 73 (property), 77 (speech)
- § 77 is absolute even in the military — soldiers can be locked up, banned from unions, restricted from gathering, but NEVER silenced
- Operational control: yes. Political control: no. A soldier remains a citizen.

**§ 71 extends § 70's discrimination grounds:**
- § 70: trosbekendelse (creed) + afstamning (descent)
- § 71: + POLITISK overbevisning (political conviction)
- Chapter VIII adds a protection ground that the equality clause itself missed

**§ 81 — the sole gendered obligation:**
- "Våbenfør mand" — every able-bodied MAN
- The only sex-based duty in an otherwise gender-neutral constitution (§ 2, § 70)
- Last survivor of 1849 gender norms in the 1953 text
- Is sex "afstamning" (descent) under § 70? Constitutionally unresolved

**The anti-feudalism programme (§§ 70, 83, 84):**
- § 70: Cannot lose rights by birth (no discrimination on descent)
- § 83: Cannot gain privileges by birth (noble privileges abolished)
- § 84: Cannot lock in wealth by birth (feudal estates banned)
- Symmetric: neither up nor down by birth

**§ 71 stk. 3 — the Greenland exception:**
- The ONLY fundamental right with a geographic exception
- 24-hour rule for judicial review can be deviated from in Greenland
- "efter de stedlige forhold" — because of local conditions
- Written in 1953 about distance; still in force 2026

**§ 72 — technological obsolescence in the text:**
- "post-, telegraf- og telefonhemmeligheden" — 1953 technologies
- Email, internet, metadata, GPS tracking: not named
- Interpretation must extend, but the text is bound to its era

**§ 78 — the constitutional asymmetry against authoritarianism:**
- FORMING an association: no permission needed
- DISSOLVING an association: requires a court
- Government can temporarily ban, must IMMEDIATELY sue
- Political associations go directly to Supreme Court
- Easier to exist than to be destroyed

**§ 80 — medieval riot pageantry, still in force:**
- Armed force against crowds only after 3 warnings
- Each warning "i kongens og lovens navn" (in the King's and law's name)
- Both sovereign AND law invoked simultaneously

**New patterns:**
- Default logic is the NATIVE pattern of the Bill of Rights — nearly every right uses default + exception
- ZERO new samtykke instances — rights simply ARE, not consent-dependent
- The HARD/SOFT split from Chapter VII continues: rights are HARD, their implementation is SOFT (delegated to law)

**Running totals after 8 chapters:**
- 85 of 89 paragraphs encoded (§§ 1-85)
- Samtykke count: still 8 (Chapters VII and VIII add zero)
- Compiler fixes total: 7 (no new fixes needed for Chapter VIII)

---

## Session 2 (cont.): Adversarial Analysis III

### What we built
- `analyse-3.runa` — Cross-cutting adversarial review of Chapters VII-VIII, 352 statements, 5 functions, 3 types

### Fidelity issues (10 found)

| # | Issue | Severity | Chapter |
|---|-------|----------|---------|
| F1 | § 67 "dyrke Gud" — monotheistic formulation excludes polytheism, Buddhism, atheism | Strukturel | VII |
| F2 | § 69 "afvigende" — normative framing (deviating, not different) | Strukturel | VII |
| F3 | § 71 stk. 1 "ingen DANSK BORGER" — citizen-specific, not universal | Meningsaendrende | VIII |
| F4 | § 71 stk. 6 explicitly exempts immigration detention from judicial review | Meningsaendrende | VIII |
| F5 | § 73 stk. 3 "domstole i dette øjemed" contradicts § 61 (no special courts) | Strukturel | VIII |
| F6 | § 77 "på tryk, i skrift og tale" — three modes, not all expression | Strukturel | VIII |
| F7 | § 78 stk. 2 "anderledes tænkende" — specifically protects dissenters | Strukturel | VIII |
| F8 | § 79 "befrygtes fare" — FEARED danger (subjective), not actual danger | Strukturel | VIII |
| F9 | § 83 abolishes privileges (forret), not titles (adel/titel/rang) | Kosmetisk | VIII |
| F10 | § 75 stk. 2 "sig eller sine" — includes dependents, not just self | Strukturel | VIII |

F3+F4 are the most serious: the Bill of Rights has a deliberate TWO-TIER liberty system (Danish citizens vs non-citizens) that our encoding completely missed.

### Constitutional insights (11 discovered)

**Three worth expanding into standalone analyses:**

1. **The Two-Tier Liberty System (Insight 2)** — The constitution uses precise subject words: "enhver" (everyone) for speech, "ingen dansk borger" (no Danish citizen) for liberty, "borgerne" (the citizens) for association/assembly. A foreigner can PUBLISH but can't ORGANIZE. Combined with the § 71.6 immigration detention carve-out, the Bill of Rights has a constitutional hole.

2. **The Conscientious Objection Gap (Insight 5)** — § 81 requires physical military service ("med sin person"). § 70 says you cannot evade duties based on creed. Together: NO constitutional basis for conscientious objection. Denmark allows it by ordinary law only.

3. **The Subject Grammar (Insight 10)** — 6 distinct categories of constitutional subject discovered: all persons (enhver), all citizens (borgerne), Danish citizens (dansk borger), specific populations (børn, mænd, kommuner), universal prohibition (ingen), and conditional (den, der...). Each right is precisely scoped.

**Other highlights:**
- **The Atheist's § 68 Paradox** — An atheist has no "own" worship, so ALL worship is "other." § 68 says don't pay for other worship. The constitution presumes theism.
- **The Morality Monopoly** — § 67's "sædelighed" morality standard is culturally set by 500 years of Folkekirke influence. The regulator is the competitor. McDonald's writes the health code for Burger King.
- **The King Protects You From Himself** — § 80 requires military force to be announced "in the King's name." The King's NAME restrains the King's POWER. The bouncer says "the boss says you can stay."
- **Nobles With No Nobility** — § 83 abolishes privileges, not titles. Danish counts still exist. The most Danish solution: don't abolish the aristocracy, just empty it.
- **Is Code Speech?** — § 77 enumerates three modes (print, writing, speech). If exhaustive: censoring a film is constitutionally fine, censoring a book is not.
- **The Pre-Crime Ban** — § 79 allows banning assemblies when danger is "befrygtes" (feared), not actual. Preventive detention: no. Preventive assembly ban: yes.
- **The Disability Trap** — § 75 only guarantees help if no family member can support you. A disabled person with wealthy relatives has no constitutional right to public assistance.
- **The Hæfte Ghost** — Three dead concepts in the text: hæfte (abolished 2001), telegraf (obsolete), tryk (narrowing). The text ages; the principles don't; but the text IS the law.

### Cumulative totals across all adversarial reviews
- analyse.runa: 10 issues + 4 funny consequences (Kap. I-II)
- analyse-2.runa: 8 fidelity + 10 insights (Kap. I-IV)
- analyse-3.runa: 10 fidelity + 11 insights (Kap. VII-VIII)
- **Total: 28 fidelity issues + 25 insights**

---

## Session 2 (cont.): Chapters IX-XI — Completion

### What we built
- `kapitel-09.runa` — Chapter IX: Forskellige bestemmelser (§§ 86-87), 93 statements, 7 functions, 2 types
- `kapitel-10.runa` — Chapter X: Grundlovsaendring (§ 88), 76 statements, 6 functions, 3 types
- `kapitel-11.runa` — Chapter XI: Overgangsbestemmelser (§ 89) + closing formula, 107 statements, 6 functions, 3 types

### Compiler fix
- **Actor system compilation:** Added `Value::Actor` display arm, `Stmt::Send` arms in `stmt_contains_try` and `emit_stmt`, `dispatch_actor_message` and `try_bind_pattern` methods. (Another session was simultaneously adding actor features to runa.rs — some fixes were needed to resolve the incomplete additions.)

### Key findings

**§ 86 — THE POINTER PATTERN (unique in the constitution):**
- Voting age for municipal councils AND parish councils = Folketing voting age
- "Til enhver tid gaeldende" — at any given time. Dynamic binding.
- This is a constitutional POINTER: § 86 doesn't say "18 years", it says "whatever § 29 says"
- Late binding in constitutional law. The first (and only) dynamic reference in the Grundlov
- **Faroe/Greenland exception:** They can set their OWN municipal voting age. The only deviation possible.
- **The Menighedsraad hook:** § 86 gives parish council elections constitutional status, partially grounding the Folkekirke's democratic governance — even though § 66's promise of a full church constitution was never fulfilled.

**§ 87 — THE GHOST PATTERN (constitutional fossil):**
- Icelandic citizens who enjoyed equal rights under the dissolution of the Danish-Icelandic Union (1944) keep Danish constitutional rights
- The ONLY paragraph that grants constitutional rights to non-Danish citizens by nationality
- Creates a constitutional MEMORY of the old realm: Iceland left, but the rights-relationship persists
- Rights follow the PERSON, not the TERRITORY — breaches § 1's territorial scope
- By 2026: the qualifying generation is gone. § 87 remains valid law with no practical subjects.
- A paragraph whose purpose was TEMPORAL — not broken (like § 66), but complete and expired.

**§ 86 + § 87 — THE JANUS CHAPTER:**
- § 86 points FORWARD (dynamic binding to future voting age changes)
- § 87 points BACKWARD (preserved rights from a former realm)
- The housekeeping chapter looks both ways in time.

**§ 88 — THE AMENDMENT PROCEDURE (the most important paragraph):**
- Five steps — the hardest procedure in the entire constitution:
  1. First Folketing passes proposal (simple majority)
  2. Government agrees to proceed (effective VETO — "regeringen vil fremme sagen")
  3. New election held (citizens choose knowing amendment is pending)
  4. Second Folketing passes UNCHANGED text ("i uaendret skikkelse")
  5. Referendum: majority of voters AND 40% of ALL eligible voters + King ratifies

- **The 40% rule:** Not just "more yes than no." 40% of ALL eligible must vote FOR.
  - If turnout is 70%: need 57% of actual voters
  - If turnout is 60%: need 67% of actual voters
  - If turnout is 50%: need 80% of actual voters
  - LOW TURNOUT KILLS AMENDMENTS. The sofa votes against.
  - Compare § 42 referendum: only needs 30% of all eligible

- **The government veto:** "Regeringen vil fremme sagen." The executive gates the constituent power. The body the constitution constrains gets a say in whether it's changed. The prisoner guards the key.

- **"I uaendret skikkelse":** Not one word can change between the two Folketings. Want to change a comma? Start over. Two separate democratic mandates for the exact same text.

- **The § 77 paradox crystallized:**
  - § 88: NO formal eternity clauses (unlike German Grundgesetz Art. 79(3))
  - § 77: "ingensinde paany" — never again. ACTS like an eternity clause.
  - § 88 is the mechanism that COULD amend § 77.
  - If § 88 amends § 77: "ingensinde" was wrong.
  - If § 77 blocks § 88: § 88 is incomplete.
  - The constitution INTENDS it to last forever, but KNOWS it can't guarantee that.

- **In practice:** 4 amendments since 1849 (→ 1866 → 1915 → 1920 → 1953). Each was effectively a new constitution. § 88 works.

**§ 89 — THE BOOTSTRAP (solving the transition problem):**
- "Traeder i kraft straks" — takes effect IMMEDIATELY. No waiting period.
- The old Rigsdag (elected under 1915/1920 Grundlov) continues until new election
- Old rules apply to old legislature until replaced
- Creates a brief DUAL REALITY: new constitution in force, old legislature governing
- The Landsting wasn't abolished by law — it was abolished by § 89 ITSELF
- The new constitution killed the old structure

- **Constitutional genealogy:**
  - § 89 names its predecessors: "Danmarks Riges Grundlov af 5. juni 1915 med aendringer af 10. september 1920"
  - Each Grundlov references its predecessor: 1953 → 1920 → 1915 → (1866 → 1849)
  - The constitution knows its own genealogy

**THE CLOSING FORMULA + SIGNATURE:**
- "Saa er da nu gaeldende ret, alle til ubroedelig efterlevelse"
- Signed by King Frederik IX + 13 ministers on Christiansborg Slot, 5 June 1953
- **Helga Pedersen** — the only woman among 14 signatories. She signed the constitution that (§ 2) opened succession to women.
- **The final § 14 demonstration:** The constitution is a royal act ("Under Vor Kongelige Haand"). Therefore it needs ministerial countersignature (§ 14). The constitution's own validity depends on the rule it contains. Self-referential to the very last line.

### THE COMPLETE THRESHOLD HIERARCHY (final, 8 levels):

| Level | Threshold | Examples |
|-------|-----------|----------|
| 0 | No vote | Self-defense § 19.2 |
| 1 | Simple majority | Samtykke pattern (8 instances) |
| 2 | 2/5 minority delay | §§ 39, 41 |
| 3 | 1/3 minority veto | §§ 42, 73 |
| 4 | 5/6 supermajority | § 20 sovereignty delegation |
| 5 | § 88 amendment | 2x Folketing + election + 40% referendum |
| 6 | "Ingensinde" | § 77 — claims to be harder than Level 5 |
| INF | True absolutes | § 10.2 (no debt), § 13 (sacrosanct), § 34 (high treason) |

### New patterns discovered in Chapters IX-XI

| Pattern | Count | Examples |
|---------|-------|---------|
| Dynamic reference (pointer) | 1 | § 86 → § 29 |
| Constitutional fossil | 1 | § 87 (Icelandic citizens) |
| Bootstrap/transition | 1 | § 89 (dual reality) |
| Government veto | 1 | § 88 (executive gates constituent power) |
| Self-referential closing | 1 | Signature requires § 14 |

### FINAL RUNNING TOTALS:
- **89 of 89 paragraphs encoded (§§ 1-89)**
- **11 chapters complete + closing formula**
- **Files:** grundlov.runa, kapitel-01 through 11, analyse, analyse-2, analyse-3, den-muslimske-konge
- **Samtykke count:** 8 (no new instances in Chapters IX-XI)
- **Compiler fixes total:** 8 (added: Actor/Send system for concurrent session)
- **Fidelity issues:** 28 across 3 adversarial reviews
- **Constitutional insights:** 25+ across 3 adversarial reviews

### THE CONCLUSION:
The entire Danish Constitution (Danmarks Riges Grundlov, 5 June 1953) is now encoded in the Futuruna programming language. 89 paragraphs, 11 chapters, one closing formula. Futuruna's mixed system of types (ADTs, named fields) and default logic (rules + exceptions) naturally expresses constitutional structure — defaults with exceptions, consent-gating, threshold hierarchies, and the selfreferential meta-rules of § 88 and § 89. The language and the law share the same deep structure.
