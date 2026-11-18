import { z } from "zod";
export declare const INPUT_SCHEMA = "{INPUT_SCHEMA}";
export declare const OUTPUT_SCHEMA = "{OUTPUT_SCHEMA}";
export declare const PARTIAL_OUTPUT_SCHEMA = "{PARTIAL_OUTPUT_SCHEMA}";
/** `Int`: a safe integer. Values beyond ±(2^53 − 1) are rejected, never rounded. */
export declare const IntSchema: z.ZodNumber;
/** `Float`: a finite number. */
export declare const FloatSchema: z.ZodNumber;
/** `Char`: a string of exactly one character (one Unicode code point). */
export declare const CharSchema: z.ZodString;
/** `Set(T)`: an array without duplicate values. */
export declare const SetSchema: <T>(item: z.ZodType<T>) => z.ZodType<T[]>;
declare const isoDateBrand: unique symbol;
/**
 * A Futuruna `Date`: an ISO 8601 calendar date `YYYY-MM-DD`, years 0000 to
 * 9999, that is on the calendar. On the wire it is the record
 * `{ "year": 2026, "month": 3, "day": 1 }`. Only {@link isoDate} and the
 * validators make one; a bare string is not one.
 */
export type IsoDate = string & {
    readonly [isoDateBrand]: "IsoDate";
};
/** `Date`: an ISO date `YYYY-MM-DD` that is on the calendar (`2026-02-30` is rejected). */
export declare const IsoDateSchema: z.ZodCustom<IsoDate, IsoDate>;
/** `text` as an {@link IsoDate}; throws a `RangeError` unless it is a date `YYYY-MM-DD` on the calendar. */
export declare function isoDate(text: string): IsoDate;
declare const unknownOf: unique symbol;
/**
 * A `T` in a partial case that is not known yet, written `{}`. `T` is a sum
 * type, whose variant the engine then needs at `path.$variant`, or, inside a
 * list or map, a value that is a record on the wire (an enum, a `Date`). It
 * has no `$variant`, so a `switch` on `$variant` must handle it.
 */
export type Unknown<T> = {
    readonly [key: string]: undefined;
    readonly $variant?: never;
    readonly [unknownOf]?: T;
};
/** Validates {@link Unknown}: exactly `{}`. */
export declare const UnknownSchema: z.ZodType<Unknown<never>>;
/**
 * "None": in a partial case, the answer that an optional value is absent,
 * `null` on the wire. A field left out (or `undefined`) is unknown instead,
 * and a bare `null` is not a partial value, so the two cannot be mixed up.
 */
export type None = {
    readonly $none: true;
};
/** The {@link None} answer. */
export declare const none: None;
/** Validates {@link None}: exactly `{ "$none": true }`. */
export declare const NoneSchema: z.ZodType<None>;
/** A codec, or a function returning one: the codecs of named types refer to each other lazily. */
export type CodecRef = Codec | (() => Codec);
/** The fields of a record or variant: each TypeScript name with the field's wire name and codec. */
export type CodecFields = {
    readonly [name: string]: readonly [wireName: string, codec: CodecRef];
};
/** The values of a positional variant, `$values` on both sides. */
export type PositionalCodecs = {
    readonly $values: readonly CodecRef[];
};
/**
 * How a TypeScript value is written in the canonical JSON `runa call` reads and
 * writes. `null` is the same on both sides; an optional value's codec writes
 * {@link none} as `null`.
 */
export type Codec = {
    readonly kind: "value";
} | {
    readonly kind: "date";
} | {
    readonly kind: "enum";
} | {
    readonly kind: "optional";
    readonly item: CodecRef;
} | {
    readonly kind: "list";
    readonly item: CodecRef;
} | {
    readonly kind: "map";
    readonly item: CodecRef;
} | {
    readonly kind: "record";
    readonly fields: CodecFields;
} | {
    readonly kind: "union";
    readonly variants: {
        readonly [variant: string]: CodecFields | PositionalCodecs;
    };
};
/** A value written as it is: a number, string, boolean or `null`, or an array or object of them. */
export declare const valueCodec: Codec;
/** A `Date`: `"2026-03-01"` is `{ "year": 2026, "month": 3, "day": 1 }` on the wire. */
export declare const isoDateCodec: Codec;
/** A sum type whose variants carry no fields: `"Deposit"` is `{ "$variant": "Deposit" }` on the wire. */
export declare const enumCodec: Codec;
/** An optional value whose present values use `item`: {@link none} and `null` are `null` on the wire. */
export declare const optionalCodec: (item: CodecRef) => Codec;
/** A list or set whose items use `item`. */
export declare const listCodec: (item: CodecRef) => Codec;
/** A `Map(String, T)` whose values use `item`; keys are the same on both sides. */
export declare const mapCodec: (item: CodecRef) => Codec;
/** A record: each TypeScript field name with its wire name and codec. */
export declare const recordCodec: (fields: CodecFields) => Codec;
/** A sum type tagged by `$variant`: the fields of each variant, or its positional `$values`. */
export declare const unionCodec: (variants: {
    readonly [variant: string]: CodecFields | PositionalCodecs;
}) => Codec;
/** A field a codec does not know, at its path in the value being converted. */
export type WireIssue = {
    readonly path: readonly (string | number)[];
    readonly message: string;
};
/** A value with a field its codec does not know: the contract has no such field there. */
export declare class WireError extends Error {
    readonly issues: readonly WireIssue[];
    constructor(issues: readonly WireIssue[]);
}
/**
 * Encode a TypeScript value, complete or partial, as `runa call` JSON. A field
 * the codec does not know throws {@link WireError}.
 */
export declare function toWire(codec: CodecRef, value: unknown): unknown;
/**
 * Decode `runa call` JSON into its TypeScript value. A field the codec does
 * not know throws {@link WireError}; any other mismatch (a string where a
 * number belongs, an unknown variant) stays as it is, for the validators to
 * reject.
 */
export declare function fromWire(codec: CodecRef, value: unknown): unknown;
/**
 * The TypeScript path of an input path the engine reports:
 * `events[0].amount_ore` → `events[0].amountOre`; an enum's tag is the enum
 * (`events[0].direction.$variant` → `events[0].direction`); a part of a `Date`
 * is the date (`events[0].date.year` → `events[0].date`). `wireCase` is the
 * encoded case, which names the variant of each sum type on the way. The part
 * of a path the codec does not describe stays as it is.
 */
export declare function fromWirePath(codec: CodecRef, wireCase: unknown, path: string): string;
/**
 * One step of a concrete input path: a field (record field, map key, `$variant`,
 * `$values`), a list index, or the entry of an open collection by its key
 * (`[day="2027-02-20"]`: the key field and its value).
 */
export type PathSegment = {
    readonly field: string;
} | {
    readonly index: number;
} | {
    readonly key: string;
    readonly value: string | number;
};
/** A path that does not parse, or that {@link setAt} cannot write. */
export declare class PathError extends Error {
    readonly path: string;
    constructor(path: string, message: string);
}
/** Split a concrete input path such as `accounts[2].closingCashOre` into its steps. */
export declare function parsePath(path: string): PathSegment[];
/**
 * Write `value` at a concrete input path, such as `accounts[2].closingCashOre`
 * or `taxpayer.outsideEndavu.$variant`, and return the new case. The case
 * passed in is not changed. A record on the way that was left out is created as
 * `{}`; a list element must already exist. Writing `$variant` keeps the object
 * when the variant is unchanged, and otherwise starts the new variant with its
 * fields unknown. The entry of an open collection (`balances[day="2027-02-20"]`)
 * is the whole entry: written into the entry with that key, or added to the
 * collection, its key set to the path's. The entry's `<entry>SetAt` is the same,
 * typed by a need.
 */
export declare function setAt<C>(partialCase: C, path: string, value: unknown): C;
/**
 * The pattern of a concrete path: list indices and entry keys become `[]`
 * (`accounts[2].kind` → `accounts[].kind`, `balances[day="2027-02-20"]` → `balances[]`).
 */
export declare function pathPattern(path: string): string;
/**
 * Whether a concrete path matches a pattern, where `[]` stands for any list
 * index or entry key and `*` for any map key.
 */
export declare function matchesPathPattern(path: string, pattern: string): boolean;
declare const inputPathOf: unique symbol;
/**
 * A concrete input path of the pattern `P`, as a need names it: list indices
 * filled in as non-negative integers (`events[3].isin` of `events[].isin`), map
 * keys as their keys. Only the validators make one: paths come from the engine.
 */
export type InputPath<P extends string = string> = string & {
    readonly [inputPathOf]: P;
};
/**
 * A validator for the concrete input paths matching one of `patterns`, typed by
 * them; `undefined` accepts any path (a recursive input type has paths of any
 * depth).
 */
export declare function inputPathSchema<P extends string>(patterns: readonly P[] | undefined): z.ZodType<InputPath<P>>;
/** A unit of measure: a number branded with its unit's name. */
export type UnitValue = number & {
    readonly __unit: string;
};
/** The validator of a unit of measure: a safe integer, typed `U` once parsed. */
export declare function unitSchema<U extends UnitValue>(unit: U["__unit"]): z.ZodType<U>;
/** `value` as a `U` of the unit `unit`; throws a `RangeError` unless it is a safe integer. */
export declare function unitValue<U extends UnitValue>(unit: U["__unit"], value: number): U;
/** A unit's declared scale: `per` of the unit make one `of` (`{ of: "DKK", per: 100 }` for øre). */
export type UnitScale = {
    readonly of: string;
    readonly per: number;
};
/**
 * `amount`, a decimal number of the unit's base as text (`"1740.25"` DKK), as
 * a `U`: exact, in integer arithmetic, never through a float. An amount finer
 * than one `U`, text that is not a decimal number, and a result beyond a safe
 * integer throw a {@link CalculationError} of kind `amount` naming the unit
 * and the amount. The SDK's converters (`øre.fromDecimal`) call it.
 */
export declare function unitFromDecimal<U extends UnitValue>(unit: U["__unit"], scale: UnitScale, amount: string): U;
/**
 * `value` as a decimal number of the unit's base, exact, with as many
 * decimals as one `U` has (`"1740.25"` for 174025 øre, `"1740.00"` for
 * 174000). Throws a `RangeError` unless `value` is a safe integer, or when the
 * scale is not built from 2s and 5s (a unit the SDK gives no converters).
 */
export declare function unitToDecimal<U extends UnitValue>(unit: U["__unit"], scale: UnitScale, value: U): string;
/** A named scalar type: a string or number branded with its type's name. */
export type ScalarValue = (string | number) & {
    readonly __scalar: string;
};
/**
 * The validator of a named scalar type: a string (or a safe integer), typed
 * `S` once parsed. The engine runs the type's checks.
 */
export declare function scalarSchema<S extends ScalarValue>(scalar: S["__scalar"], of: "String" | "Int"): z.ZodType<S>;
/** `value` as an `S`; throws a `RangeError` unless it is a string (or a safe integer). */
export declare function scalarValue<S extends ScalarValue>(scalar: S["__scalar"], of: "String" | "Int", value: unknown): S;
/** A source reference from the contract's field metadata. */
export type SourceReference = {
    label: string;
    role: string;
    binding: string;
    type?: string | undefined;
    value?: string | undefined;
    data?: unknown;
    text?: string | undefined;
    symbols?: string[] | undefined;
};
export declare const SourceReferenceSchema: z.ZodType<SourceReference>;
/** The act a paragraph belongs to, from the header of its source document. */
export type Act = {
    /** The act's title, such as `aktiesparekontoloven`. */
    title: string;
    /** Its identifier, such as `LBK nr. 281 af 2025`, where its document gives one. */
    identifier?: string | undefined;
    url?: string | undefined;
};
/**
 * The amending act's instruction behind a passage cited `as amended`: the act
 * (`LOV nr. 1691 af 30/12/2024`), its paragraph (`§ 4, nr. 1`) and the
 * instruction line (`1. § 8 a, stk. 1, 1. pkt., affattes således:`).
 */
export type Amendment = {
    act: string;
    paragraph: string;
    instruction: string;
};
/** A paragraph of an act, as a rule cites it: `§ 16, stk. 2`, the document quoting it, and the act. */
export type ParagraphReference = {
    paragraph: string;
    /** The quoted document, such as `sources/01-aktiesparekontoloven.txt`. */
    source?: string | undefined;
    act?: Act | undefined;
    /** For a passage cited `as amended`, the instruction that gives its wording. */
    asAmendedBy?: Amendment | undefined;
};
/** A rule or function that waits for a need's value, in its law section, with the paragraphs it cites. */
export type WaitingRule = {
    rule: string;
    /** The law section (anchor label) the rule is declared in. */
    section?: string | undefined;
    paragraphs?: ParagraphReference[] | undefined;
};
/**
 * The key of a list element a need lies in, where the element's type declares
 * a key field (`@ field(key = True)`): the element's path, the key field's
 * name and its value, such as `{ path: "openingPositions[0]", field: "isin",
 * value: "DK0060534915" }`.
 */
export type ElementKey<E extends string = string, F extends string = string, V = unknown> = {
    readonly path: InputPath<E>;
    readonly field: F;
    readonly value: V;
};
/** How a need's {@link ElementKey}s of one list are validated: the list's element pattern, key field and value. */
export type ElementKeySchema<K extends ElementKey> = {
    readonly element: string;
    readonly field: K["field"];
    readonly value: z.ZodType<K["value"]>;
};
/** The field metadata a need carries; each part is left out when the contract does not give it. */
type NeedMetadata<K> = {
    label?: string | undefined;
    question?: string | undefined;
    help?: string | undefined;
    unit?: string | undefined;
    sources?: SourceReference[] | undefined;
    /**
     * The rules and functions in a law section that wait for the value, the
     * ones citing paragraphs first; never empty.
     */
    waiting: [WaitingRule, ...WaitingRule[]];
    /** The keys of the list elements the path lies in, outermost first, where declared and known. */
    keys?: K[] | undefined;
};
/** A fact the client answers, at `path`, a path of `pattern`. */
export type FactNeed<P extends string = string, K = ElementKey> = NeedMetadata<K> & {
    kind: "fact";
    /** The input path pattern of `path`, which keys resolvers and types the answer. */
    pattern: P;
    /** The concrete input path to answer, with list indices. */
    path: InputPath<P>;
};
/** A value another entry provides: run `entry` for `subject` and write its result at `path`. */
export type EntryNeed<P extends string = string, K = ElementKey> = NeedMetadata<K> & {
    kind: "entry";
    /** The input path pattern of `path`. */
    pattern: P;
    /** The concrete input path the entry's result is written at. */
    path: InputPath<P>;
    /** The entry that provides the value. */
    entry: string;
    /** The input path of the subject the entry runs for. */
    subject: string;
};
/**
 * Something a partial case still needs: a fact, or another entry's result. For
 * a union of patterns it is a union of needs, one per pattern, so narrowing
 * `need.pattern` narrows the need and the type of its answer.
 */
export type Need<P extends string = string, K = ElementKey> = P extends string ? FactNeed<P, K> | EntryNeed<P, K> : never;
/**
 * One input path pattern of an entry at which a need can arise, as the law
 * gives it: the value's Futuruna type as written (`Int`, `Spouse?`,
 * `List(Child)`, a unit's name), the field's metadata, the patterns of the key
 * fields of the list elements it lies in (outermost first), and for a value
 * another entry provides, that entry and the path of its subject.
 */
export type GuideNeed = {
    readonly kind: "fact" | "entry";
    readonly type: string;
    readonly label?: string | undefined;
    readonly question?: string | undefined;
    readonly help?: string | undefined;
    readonly unit?: string | undefined;
    readonly keys: readonly string[];
    readonly sources: readonly SourceReference[];
    readonly entry?: string | undefined;
    readonly subject?: string | undefined;
};
/**
 * An entry's answer guide, generated from its law: every input path pattern
 * of `P` at which a need can arise, once. A pattern no need can name (a
 * record, the element of a list of records) is left out.
 */
export type AnswerGuide<P extends string = string> = {
    readonly [K in P]?: GuideNeed;
};
/** `P` when it is one pattern; `never` for a union of several. */
type OnePattern<P, All = P> = P extends unknown ? ([All] extends [P] ? P : never) : never;
/**
 * The value an answer to `N` writes: the type at its pattern in `Patterns`.
 * `never` until `N` is the need of one pattern, so a need of several patterns
 * must be narrowed (by `need.pattern`) before it is answered.
 */
export type AnswerTo<Patterns, N extends {
    readonly pattern: string;
}> = Patterns[OnePattern<N["pattern"]> & keyof Patterns];
/**
 * The validator of an entry's needs: the pattern is one of `patterns` (any
 * pattern, when `undefined`) and the path matches it, and each element key is
 * the declared key of a list the path lies in.
 */
export declare function needSchema<P extends string, K extends ElementKey>(patterns: readonly P[] | undefined, keys: readonly ElementKeySchema<K>[]): z.ZodType<Need<P, K>>;
/**
 * An assumption a result rests on: an `Assumption` written on a rule clause
 * that fired, on the section it sits in, or on the entry or its section.
 */
export type CalculationAssumption = {
    /** The law section it is written in; absent when it stands outside any section. */
    section?: string | undefined;
    /** The rule, member or function it is written on; absent when it is written on its section. */
    rule?: string | undefined;
    /** The binding the assumption is written as, when it is one. */
    binding?: string | undefined;
    /** Its value as the engine writes it: a record's fields as an object, at their names in the model. */
    value: unknown;
};
export declare const CalculationAssumptionSchema: z.ZodType<CalculationAssumption>;
/** A calculation's result and the assumptions it rests on, each once, in the order the model writes them. */
export type Calculated<R> = {
    readonly result: R;
    readonly assumptions: readonly CalculationAssumption[];
};
/** A partial case whose result no completion of its unknown fields can change. */
export type Complete<R, N extends Need = Need> = {
    $variant: "Complete";
    result: R;
    /** The assumptions the result rests on, each once, in the order the model writes them. */
    assumptions: CalculationAssumption[];
    /** The provided input paths evaluation never read, sorted. */
    unusedPaths: InputPath<N["pattern"]>[];
};
/** A partial case that cannot be decided yet. */
export type Needs<N extends Need = Need> = {
    $variant: "Needs";
    /** Every fact and entry that blocks the result as it stands, sorted by path, each path once. */
    needs: N[];
    /**
     * Facts read only behind a condition that is not known yet: they do not
     * block the result as it stands, and answering `needs` may make them needs.
     */
    later?: N[] | undefined;
};
/** The result of a partial call. */
export type QueryResult<R, N extends Need = Need> = Complete<R, N> | Needs<N>;
export declare function queryResultSchema<R, N extends Need>(result: z.ZodType<R>, need: z.ZodType<N>, path: z.ZodType<InputPath<N["pattern"]>>): z.ZodType<QueryResult<R, N>>;
/**
 * How the SDK reaches an engine. Each method receives the input document
 * `runa call --input` reads (one case, marked ready) and resolves to the output
 * document `runa call` writes, as an object or as its JSON text.
 */
export interface EngineTransport {
    /** Run `entry` on complete input, as `runa call`. */
    call(entry: string, input: unknown): Promise<unknown>;
    /** Run `entry` on a partial case, as `runa call --partial`. */
    callPartial(entry: string, input: unknown): Promise<unknown>;
}
/** A transport that also explains, as the `...Explained` calls need. */
export interface ExplainingTransport extends EngineTransport {
    /** Run `entry` and explain every leaf of the result, as `runa call --explain` (`--explain=full` when `full`). */
    callExplained(entry: string, input: unknown, full: boolean): Promise<unknown>;
    /** As `callExplained`, on a partial case: `runa call --partial --explain`. */
    callPartialExplained(entry: string, input: unknown, full: boolean): Promise<unknown>;
}
/**
 * One entry: its name, its contract hash, the wire codecs of its
 * input and result, and its input path patterns, which tag its needs.
 */
export type CalculationEntry = {
    readonly name: string;
    readonly schemaHash: string;
    readonly input: CodecRef;
    readonly output: CodecRef;
    readonly patterns: readonly string[];
};
/**
 * Why a call failed: the input did not validate, the output did not validate,
 * the envelope is for another contract or entry, or the engine reported
 * diagnostics for the case. `amount` is no call: a unit converter refused an
 * amount finer than its unit, or one that is not a decimal number.
 */
export type CalculationErrorKind = "input" | "output" | "envelope" | "diagnostics" | "amount";
/**
 * A diagnostic the engine reported for the case, at its path in the input
 * document. When an input value is not a value of its named scalar type, the
 * diagnostic names the type and the check that is false.
 */
export type CalculationDiagnostic = {
    caseId: string;
    path: string;
    message: string;
    scalar?: string | undefined;
    check?: string | undefined;
};
/**
 * An input value the engine refused because a check of its named scalar type
 * is false (`isin_check_digit`), at its TypeScript path (`events[3].isin`).
 */
export type ScalarIssue = {
    path: string;
    message: string;
    scalar: string;
    check: string;
};
export declare class CalculationError extends Error {
    readonly kind: CalculationErrorKind;
    /** The entry called; for `amount`, the unit converting the amount. */
    readonly entry: string;
    /** The validation issues, for `input` and `output`. */
    readonly issues: readonly unknown[];
    /** The engine's diagnostics, for `diagnostics`. */
    readonly diagnostics: readonly CalculationDiagnostic[];
    /** For `input` the engine found: the named scalar type of the first refused value. */
    readonly scalar: string | undefined;
    /** For `input` the engine found: the check of `scalar` that is false. */
    readonly check: string | undefined;
    constructor(kind: CalculationErrorKind, entry: string, message: string, details?: {
        issues?: readonly unknown[];
        diagnostics?: readonly CalculationDiagnostic[];
        scalar?: string | undefined;
        check?: string | undefined;
    });
}
/**
 * Calculate `entry` on a complete case: validate it, encode it, run it through
 * `transport.call`, check the envelope, and decode and validate the result;
 * returned with the assumptions it rests on.
 */
export declare function callEntry<I, O>(transport: EngineTransport, entry: CalculationEntry, inputSchema: z.ZodType<I>, outputSchema: z.ZodType<O>, input: I): Promise<Calculated<O>>;
/**
 * Query `entry` on a partial case through `transport.callPartial`: `Complete`
 * or `Needs`, with the result decoded, every path a TypeScript path and every
 * need tagged with its pattern.
 */
export declare function queryEntry<I, R, N extends Need>(transport: EngineTransport, entry: CalculationEntry, inputSchema: z.ZodType<I>, resultSchema: z.ZodType<QueryResult<R, N>>, input: I): Promise<QueryResult<R, N>>;
/** How much an explained call reports: `direct` contributors of each value, or also the `full` derivation. */
export type ExplainDepth = "direct" | "full";
/** A leaf of a result or a case: a number, string or boolean, an enum's variant, or a `Date` as an ISO date. */
export type ExplainedLeaf = string | number | boolean | null;
/** A paragraph a rule clause cites, with its quoted text. */
export type CitedParagraph = ParagraphReference & {
    text: string;
};
/** The rule clause that produced a value: its law section and cited paragraphs. */
export type ExplainedRule = {
    $variant: "Rule";
    rule: string;
    /** The RuleScope of a scoped rule. */
    scope?: string | undefined;
    clause: "default" | "exception" | "clause";
    /** The label of the exception that applied. */
    exception?: string | undefined;
    /** `file:line` of the clause. */
    location?: string | undefined;
    /** The law section around the clause; a key of {@link Explanation.sections}. */
    section?: string | undefined;
    cites?: CitedParagraph[] | undefined;
};
/** A rule a value was computed from, with the value it gave. */
export type RuleContributor = Omit<ExplainedRule, "cites"> & {
    paragraphs?: ParagraphReference[] | undefined;
    value: ExplainedLeaf;
    /** The node in {@link Explanation.derivation}, with depth `full`. */
    node?: number | undefined;
};
/** An input a value was computed from, at its TypeScript path in the case. */
export type InputContributor = {
    $variant: "Input";
    path: string;
    value: ExplainedLeaf;
};
export type Contributor = RuleContributor | InputContributor;
/**
 * What produced a value: a rule clause, the input at a TypeScript path of the
 * case, a computation outside any rule, or a constant of the model.
 */
export type ExplainedBy = ExplainedRule | {
    $variant: "Input";
    path: string;
} | {
    $variant: "Computed";
} | {
    $variant: "Constant";
};
/** One leaf of a result and where it came from. */
export type ExplainedValue = {
    value: ExplainedLeaf;
    by: ExplainedBy;
    from?: Contributor[] | undefined;
    /** Direct contributors beyond the first 32, with depth `direct`. */
    fromOmitted?: number | undefined;
    /** The inputs and rules the guards read that chose the clause. */
    because?: Contributor[] | undefined;
};
/** A law section: its anchor, quoted documents, attachments and quote. */
export type ExplainedSection = {
    location: string;
    sources: string[];
    /** Each document the section quotes: its binding, file and role. */
    documents: {
        binding: string;
        file: string | null;
        role: string | null;
    }[];
    attachments: {
        role: string;
        binding?: string | undefined;
        value: unknown;
    }[];
    quote: string;
    /** The quoted passages an instruction quoted in the same section amends. */
    superseded?: {
        paragraph: string;
        asAmendedBy: Amendment;
    }[] | undefined;
};
/**
 * Where every leaf of a result came from (`docs/rfcs/explanations.md`), at the
 * TypeScript paths of the result and the case: an enum is one leaf, and a
 * `Date` is one leaf, its ISO date, unless its parts came from different
 * places; then each part is explained at `<date>.year`, `.month` and `.day`.
 */
export type Explanation = {
    /** Every leaf of the result by its TypeScript path. */
    values: {
        [path: string]: ExplainedValue;
    };
    sections: {
        [label: string]: ExplainedSection;
    };
    /** With depth `full`: every rule node a value was computed from. */
    derivation?: {
        [node: string]: {
            value: ExplainedLeaf;
            by: ExplainedRule;
            from: Contributor[];
            because?: Contributor[] | undefined;
        };
    } | undefined;
};
/** A result with its assumptions and its explanation. */
export type Explained<R> = Calculated<R> & {
    readonly explanation: Explanation;
};
/** A partial call's result; a `Complete` result carries an explanation. */
export type ExplainedQuery<R> = {
    readonly result: R;
    readonly explanation?: Explanation | undefined;
};
/**
 * Calculate `entry` and explain it, through `transport.callExplained`
 * (`runa call --explain`): the result as {@link callEntry} returns it, and
 * where every leaf of it came from.
 */
export declare function explainEntry<I, O>(transport: ExplainingTransport, entry: CalculationEntry, depth: ExplainDepth, inputSchema: z.ZodType<I>, outputSchema: z.ZodType<O>, input: I): Promise<Explained<O>>;
/**
 * Query `entry` and explain a `Complete` result, through
 * `transport.callPartialExplained` (`runa call --partial --explain`).
 */
export declare function explainQueryEntry<I, R, N extends Need>(transport: ExplainingTransport, entry: CalculationEntry, depth: ExplainDepth, inputSchema: z.ZodType<I>, resultSchema: z.ZodType<QueryResult<R, N>>, input: I): Promise<ExplainedQuery<QueryResult<R, N>>>;
/** One entry running for one subject, on the stack of entries being answered. */
export type EntryFrame = {
    readonly entry: string;
    readonly subject: string;
};
/** The reply for a need `resolve` cannot fill: it stays open. */
export type Open = {
    readonly kind: "open";
};
/**
 * What `resolve` gives for a need at a path of `P`: `given(need, value)`, a
 * value to write at the need's path; `open`, when it cannot fill the need; or
 * the outcome of another entry ({@link ResolverContext.runEntry}). A reply
 * names the path it is for, so it is checked against the need it answers.
 */
export type ResolverReply<V = unknown, P extends string = string> = {
    readonly kind: "given";
    readonly path: InputPath<P>;
    readonly value: V;
} | Open | {
    readonly kind: "entry";
    readonly path: InputPath<P>;
    readonly answering: Answering<V>;
};
/** The reply for a need `resolve` cannot fill: it stays open. */
export declare const open: Open;
/** Any value: a type parameter of this constraint keeps a literal such as `"Married"`, a variant's name. */
type AnyValue = string | number | boolean | bigint | symbol | object | null | undefined;
/**
 * The reply that gives `value` for `need`. The value is checked against the
 * type at the need's pattern, so a need of several patterns is narrowed by
 * `need.pattern` first.
 */
export declare function given<P extends string, V extends AnyValue>(need: {
    readonly path: InputPath<P>;
}, value: V): ResolverReply<V, P>;
/** What `resolve` is given besides its needs. */
export interface ResolverContext<C> {
    /** The case sent in this round: the partial case and every answer written so far. */
    readonly partialCase: C;
    /**
     * Answer another entry for `need`, as the entry `need.entry` running for
     * `need.subject`: an entry need names them, a fact need is given them
     * (`{ ...need, entry: "share_income", subject: "taxpayer.spouse" }`). `run`
     * answers that entry, passing `options` on, as
     * `(options) => shareIncomeAnswer(transport, partialCase, resolve, options)`.
     * Its result is written at the need's path when it is `Answered`; when it is
     * `Unanswered` the need waits in `pending`. An entry already being answered
     * for the same subject is a cycle and throws {@link AnswerCycleError}.
     */
    runEntry<P extends string, R2>(need: {
        readonly path: InputPath<P>;
    } & EntryFrame, run: (options: AnswerOptions) => Promise<Answering<R2>>): Promise<ResolverReply<R2, P>>;
}
/** A partial call of one entry, such as `(c) => taxReportQuery(transport, c)`. */
export type Query<C, R, N extends Need = Need> = (partialCase: C) => Promise<QueryResult<R, N>>;
/** The value types of an entry's input patterns, when not known: any value at any pattern. */
export type AnyPatterns = {
    readonly [pattern: string]: unknown;
};
/** A reply to a need of the pattern `P`: its value of the type `Patterns` gives `P`, or `open`. */
export type Reply<Patterns, P extends string> = (P extends keyof Patterns ? ResolverReply<Patterns[P], P> : never) | Open;
/** Answers one need of a round, with a reply typed by the need's own pattern. */
export type AnswerEach<Patterns, N extends Need = Need> = (need: N) => Reply<Patterns, N["pattern"]> | Promise<Reply<Patterns, N["pattern"]>>;
/**
 * How an entry's needs are answered: called once per round with every need of
 * the round, so it can fetch what they ask for in one batch (by `need.keys`
 * or `need.path`), it returns the function that answers each need, which is
 * called once per need:
 *
 * ```ts
 * async (needs) => {
 *   const rows = await backend.events(needs.map(eventId));
 *   return (need) => {
 *     switch (need.pattern) {
 *       case "events[].amountOre":
 *         return given(need, rows.get(eventId(need)).amountOre);
 *       default:
 *         return open;
 *     }
 *   };
 * }
 * ```
 *
 * It returns a function rather than a list of answers so that each answer is
 * made from its own need: no need is left without a reply, and a reply names
 * the need's path, typed by its pattern. A reply for another path throws
 * {@link AnswerError}.
 */
export type Resolve<C, Patterns = AnyPatterns, N extends Need = Need> = (needs: readonly N[], context: ResolverContext<C>) => AnswerEach<Patterns, N> | Promise<AnswerEach<Patterns, N>>;
/** One round: the paths of its needs, and the paths it answered. */
export type Round<N extends Need = Need> = {
    readonly paths: readonly InputPath<N["pattern"]>[];
    readonly given: readonly InputPath<N["pattern"]>[];
};
/** An entry need waiting on what another entry left open. */
export type PendingEntry = {
    readonly need: Need;
    readonly answering: Answering<unknown>;
};
/**
 * Where answering an entry ended: `Answered`, or `Unanswered` when a round
 * answered nothing, with that round's needs left open. The partial case holds
 * every answer written.
 */
export type Answering<R, N extends Need = Need, C = unknown> = {
    readonly $variant: "Answered";
    readonly result: R;
    /** The assumptions the result rests on. */
    readonly assumptions: readonly CalculationAssumption[];
    readonly unusedPaths: readonly InputPath<N["pattern"]>[];
    readonly partialCase: C;
    readonly rounds: readonly Round<N>[];
} | {
    readonly $variant: "Unanswered";
    /** The needs of the last round `resolve` left open. */
    readonly open: readonly N[];
    /** Entry needs waiting on what another entry left open. */
    readonly pending: readonly PendingEntry[];
    /**
     * Facts read only behind an unknown condition in the last round: what the
     * engine may need once the open needs are answered, in the next round.
     */
    readonly later: readonly N[];
    readonly partialCase: C;
    readonly rounds: readonly Round<N>[];
};
/**
 * How an entry is answered from inside another's `resolve`: what
 * {@link ResolverContext.runEntry} passes to its `run`, to be passed on to
 * `<entry>Answer` unchanged.
 */
export type AnswerOptions = {
    /** The entries being answered above this one, outermost first. */
    readonly frames?: readonly EntryFrame[];
};
/** Answering cannot continue. */
export declare class AnswerError extends Error {
    constructor(message: string);
}
/** Two entries need each other for the same subjects: a model error; join them into one entry. */
export declare class AnswerCycleError extends AnswerError {
    readonly frames: readonly EntryFrame[];
    constructor(frames: readonly EntryFrame[]);
}
/**
 * Answer an entry round by round: query it on the partial case, call `resolve`
 * once per round with every need, write the given values with {@link setAt},
 * and query again. Ends `Answered`, or `Unanswered` when a round answers
 * nothing. A reply for another need's path, or a path still needed after it
 * was given, throws {@link AnswerError}; an entry that needs itself for the
 * same subject throws {@link AnswerCycleError}. The SDK's `<entry>Answer`
 * calls it.
 */
export declare function answerEntry<C, R, N extends Need, Patterns>(query: Query<C, R, N>, partialCase: C, resolve: Resolve<C, Patterns, N>, options?: AnswerOptions): Promise<Answering<R, N, C>>;
export {};
