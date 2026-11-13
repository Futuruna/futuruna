import { z } from "zod";

export const INPUT_SCHEMA = "{INPUT_SCHEMA}";
export const OUTPUT_SCHEMA = "{OUTPUT_SCHEMA}";
export const PARTIAL_OUTPUT_SCHEMA = "{PARTIAL_OUTPUT_SCHEMA}";

// Values

/** `Int`: a safe integer. Values beyond ±(2^53 − 1) are rejected, never rounded. */
export const IntSchema = z.number().refine(Number.isSafeInteger, "expected a safe integer (Int within ±(2^53 − 1))");

/** `Float`: a finite number. */
export const FloatSchema = z.number().refine(Number.isFinite, "expected a finite number");

/** `Char`: a string of exactly one character (one Unicode code point). */
export const CharSchema = z.string().refine((text) => /^[\s\S]$/u.test(text), "expected exactly one character");

/** `Set(T)`: an array without duplicate values. */
export const SetSchema = <T>(item: z.ZodType<T>): z.ZodType<T[]> =>
  z.array(item).refine(
    (values) => new Set(values.map(canonicalJson)).size === values.length,
    "expected a set without duplicate values",
  );

function canonicalJson(value: unknown): string {
  if (value === undefined) return "undefined";
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (typeof value === "object" && value !== null) {
    const entries = Object.entries(value).sort(([left], [right]) => byText(left, right));
    return `{${entries.map(([key, item]) => `${JSON.stringify(key)}:${canonicalJson(item)}`).join(",")}}`;
  }
  return JSON.stringify(value);
}

/** Any value as text for a message: a string as it is, anything else as JSON. */
function shown(value: unknown): string {
  return typeof value === "string" ? value : canonicalJson(value);
}

function byText(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

declare const isoDateBrand: unique symbol;

/**
 * A Futuruna `Date`: an ISO 8601 calendar date `YYYY-MM-DD`, years 0000 to
 * 9999, that is on the calendar. On the wire it is the record
 * `{ "year": 2026, "month": 3, "day": 1 }`. Only {@link isoDate} and the
 * validators make one; a bare string is not one.
 */
export type IsoDate = string & { readonly [isoDateBrand]: "IsoDate" };

const ISO_DATE = /^(\d{4})-(\d{2})-(\d{2})$/;

function isoDateParts(text: string): { year: number; month: number; day: number } | undefined {
  const match = ISO_DATE.exec(text);
  if (match === null) return undefined;
  const [year, month, day] = [Number(match[1]), Number(match[2]), Number(match[3])];
  const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
  const days = month === 2 ? (leap ? 29 : 28) : [4, 6, 9, 11].includes(month) ? 30 : 31;
  return month >= 1 && month <= 12 && day >= 1 && day <= days ? { year, month, day } : undefined;
}

function isIsoDate(value: unknown): value is IsoDate {
  return typeof value === "string" && isoDateParts(value) !== undefined;
}

/** `Date`: an ISO date `YYYY-MM-DD` that is on the calendar (`2026-02-30` is rejected). */
export const IsoDateSchema = z.custom<IsoDate>(isIsoDate, "expected an ISO date YYYY-MM-DD on the calendar");

/** `text` as an {@link IsoDate}; throws a `RangeError` unless it is a date `YYYY-MM-DD` on the calendar. */
export function isoDate(text: string): IsoDate {
  if (!isIsoDate(text)) throw new RangeError(`expected an ISO date YYYY-MM-DD on the calendar, got ${JSON.stringify(text)}`);
  return text;
}

declare const unknownOf: unique symbol;

/**
 * A `T` in a partial case that is not known yet, written `{}`. `T` is a sum
 * type, whose variant the engine then needs at `path.$variant`, or, inside a
 * list or map, a value that is a record on the wire (an enum, a `Date`). It
 * has no `$variant`, so a `switch` on `$variant` must handle it.
 */
export type Unknown<T> = { readonly [key: string]: undefined; readonly $variant?: never; readonly [unknownOf]?: T };

/** Validates {@link Unknown}: exactly `{}`. */
export const UnknownSchema: z.ZodType<Unknown<never>> = z.object({}).strict();

/**
 * "None": in a partial case, the answer that an optional value is absent,
 * `null` on the wire. A field left out (or `undefined`) is unknown instead,
 * and a bare `null` is not a partial value, so the two cannot be mixed up.
 */
export type None = { readonly $none: true };

/** The {@link None} answer. */
export const none: None = Object.freeze({ $none: true });

/** Validates {@link None}: exactly `{ "$none": true }`. */
export const NoneSchema: z.ZodType<None> = z.object({ $none: z.literal(true) }).strict();

function isNone(value: unknown): value is None {
  return isObject(value) && value["$none"] === true && Object.keys(value).length === 1;
}

// The wire codec

/** A codec, or a function returning one: the codecs of named types refer to each other lazily. */
export type CodecRef = Codec | (() => Codec);

/** The fields of a record or variant: each TypeScript name with the field's wire name and codec. */
export type CodecFields = { readonly [name: string]: readonly [wireName: string, codec: CodecRef] };

/** The values of a positional variant, `$values` on both sides. */
export type PositionalCodecs = { readonly $values: readonly CodecRef[] };

/**
 * How a TypeScript value is written in the canonical JSON `runa call` reads and
 * writes. `null` is the same on both sides; an optional value's codec writes
 * {@link none} as `null`.
 */
export type Codec =
  | { readonly kind: "value" }
  | { readonly kind: "date" }
  | { readonly kind: "enum" }
  | { readonly kind: "optional"; readonly item: CodecRef }
  | { readonly kind: "list"; readonly item: CodecRef }
  | { readonly kind: "map"; readonly item: CodecRef }
  | { readonly kind: "record"; readonly fields: CodecFields }
  | { readonly kind: "union"; readonly variants: { readonly [variant: string]: CodecFields | PositionalCodecs } };

/** A value written as it is: a number, string, boolean or `null`, or an array or object of them. */
export const valueCodec: Codec = { kind: "value" };

/** A `Date`: `"2026-03-01"` is `{ "year": 2026, "month": 3, "day": 1 }` on the wire. */
export const isoDateCodec: Codec = { kind: "date" };

/** A sum type whose variants carry no fields: `"Deposit"` is `{ "$variant": "Deposit" }` on the wire. */
export const enumCodec: Codec = { kind: "enum" };

/** An optional value whose present values use `item`: {@link none} and `null` are `null` on the wire. */
export const optionalCodec = (item: CodecRef): Codec => ({ kind: "optional", item });

/** A list or set whose items use `item`. */
export const listCodec = (item: CodecRef): Codec => ({ kind: "list", item });

/** A `Map(String, T)` whose values use `item`; keys are the same on both sides. */
export const mapCodec = (item: CodecRef): Codec => ({ kind: "map", item });

/** A record: each TypeScript field name with its wire name and codec. */
export const recordCodec = (fields: CodecFields): Codec => ({ kind: "record", fields });

/** A sum type tagged by `$variant`: the fields of each variant, or its positional `$values`. */
export const unionCodec = (variants: { readonly [variant: string]: CodecFields | PositionalCodecs }): Codec => ({
  kind: "union",
  variants,
});

function codecOf(ref: CodecRef): Codec {
  return typeof ref === "function" ? ref() : ref;
}

/** The codec of the values a path walks through: an optional value's is its item's. */
function presentCodec(ref: CodecRef): Codec {
  const codec = codecOf(ref);
  return codec.kind === "optional" ? presentCodec(codec.item) : codec;
}

function isObject(value: unknown): value is { readonly [key: string]: unknown } {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasOwn(object: object, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(object, key);
}

function isPositional(spec: CodecFields | PositionalCodecs): spec is PositionalCodecs {
  return hasOwn(spec, "$values");
}

/** Set an own property, also for a key such as `__proto__`. */
function put(target: { [key: string]: unknown }, key: string, value: unknown): void {
  Object.defineProperty(target, key, { value, enumerable: true, writable: true, configurable: true });
}

const wireNames = new WeakMap<CodecFields, ReadonlyMap<string, readonly [string, CodecRef]>>();

/** The fields by wire name: wire name → TypeScript name and codec. */
function byWireName(fields: CodecFields): ReadonlyMap<string, readonly [string, CodecRef]> {
  let names = wireNames.get(fields);
  if (names === undefined) {
    names = new Map(Object.entries(fields).map(([name, [wireName, codec]]) => [wireName, [name, codec] as const]));
    wireNames.set(fields, names);
  }
  return names;
}

/** A field a codec does not know, at its path in the value being converted. */
export type WireIssue = { readonly path: readonly (string | number)[]; readonly message: string };

/** A value with a field its codec does not know: the contract has no such field there. */
export class WireError extends Error {
  readonly issues: readonly WireIssue[];

  constructor(issues: readonly WireIssue[]) {
    super(issueSummary(issues));
    this.name = "WireError";
    this.issues = issues;
  }
}

type Conversion = { readonly encode: boolean; readonly path: (string | number)[]; readonly issues: WireIssue[] };

/**
 * Encode a TypeScript value, complete or partial, as `runa call` JSON. A field
 * the codec does not know throws {@link WireError}.
 */
export function toWire(codec: CodecRef, value: unknown): unknown {
  return converted(codec, value, true);
}

/**
 * Decode `runa call` JSON into its TypeScript value. A field the codec does
 * not know throws {@link WireError}; any other mismatch (a string where a
 * number belongs, an unknown variant) stays as it is, for the validators to
 * reject.
 */
export function fromWire(codec: CodecRef, value: unknown): unknown {
  return converted(codec, value, false);
}

function converted(codec: CodecRef, value: unknown, encode: boolean): unknown {
  const conversion: Conversion = { encode, path: [], issues: [] };
  const out = convert(codec, value, conversion);
  if (conversion.issues.length > 0) throw new WireError(conversion.issues);
  return out;
}

function convert(ref: CodecRef, value: unknown, conversion: Conversion): unknown {
  if (value === undefined || value === null) return value;
  const codec = codecOf(ref);
  const { encode, path } = conversion;
  switch (codec.kind) {
    case "value":
      return value;
    case "date":
      return encode ? ((typeof value === "string" && isoDateParts(value)) || value) : dateFromWire(value);
    case "enum":
      if (encode) return typeof value === "string" ? { $variant: value } : value;
      return isObject(value) && typeof value["$variant"] === "string" && Object.keys(value).length === 1
        ? value["$variant"]
        : value;
    case "optional":
      return encode && isNone(value) ? null : convert(codec.item, value, conversion);
    case "list": {
      if (!Array.isArray(value)) return value;
      const out: unknown[] = [];
      for (const [index, item] of value.entries()) {
        path.push(index);
        out.push(convert(codec.item, item, conversion));
        path.pop();
      }
      return out;
    }
    case "map": {
      if (!isObject(value)) return value;
      const out: { [key: string]: unknown } = {};
      for (const [key, item] of Object.entries(value)) {
        path.push(key);
        put(out, key, convert(codec.item, item, conversion));
        path.pop();
      }
      return out;
    }
    case "record":
      return isObject(value) ? convertFields(codec.fields, value, false, conversion) : value;
    case "union": {
      if (!isObject(value)) return value;
      const variant = value["$variant"];
      const spec = typeof variant === "string" && hasOwn(codec.variants, variant) ? codec.variants[variant] : undefined;
      if (spec === undefined) return value;
      if (!isPositional(spec)) return convertFields(spec, value, true, conversion);
      const values = value["$values"];
      if (!Array.isArray(values)) return value;
      const out: { [key: string]: unknown } = { ...value };
      path.push("$values");
      out["$values"] = values.map((item: unknown, index) => {
        path.push(index);
        const next = convert(spec.$values[index] ?? valueCodec, item, conversion);
        path.pop();
        return next;
      });
      path.pop();
      return out;
    }
  }
}

/** `tagged`: the fields of a variant, next to its `$variant`. */
function convertFields(
  fields: CodecFields,
  value: { readonly [key: string]: unknown },
  tagged: boolean,
  conversion: Conversion,
): unknown {
  const { encode, path, issues } = conversion;
  const names = encode ? undefined : byWireName(fields);
  const out: { [key: string]: unknown } = {};
  for (const [key, item] of Object.entries(value)) {
    if (item === undefined) continue;
    if (tagged && key === "$variant") {
      out["$variant"] = item;
      continue;
    }
    const field = names === undefined ? (hasOwn(fields, key) ? fields[key] : undefined) : names.get(key);
    if (field === undefined) {
      issues.push({ path: [...path, key], message: `unknown field \`${key}\`` });
      continue;
    }
    const name = encode ? key : field[0];
    path.push(name);
    put(out, encode ? field[0] : name, convert(field[1], item, conversion));
    path.pop();
  }
  return out;
}

function dateFromWire(value: unknown): unknown {
  if (!isObject(value) || Object.keys(value).length !== 3) return value;
  const { year, month, day } = value;
  if (!(Number.isSafeInteger(year) && Number.isSafeInteger(month) && Number.isSafeInteger(day))) return value;
  const pad = (part: number, width: number) => String(part).padStart(width, "0");
  const [y, m, d] = [Number(year), Number(month), Number(day)];
  return `${y >= 0 ? pad(y, 4) : String(y)}-${pad(m, 2)}-${pad(d, 2)}`;
}

const DATE_PARTS: Record<string, true> = { year: true, month: true, day: true };

/**
 * The TypeScript path of an input path the engine reports:
 * `events[0].amount_ore` → `events[0].amountOre`; an enum's tag is the enum
 * (`events[0].direction.$variant` → `events[0].direction`); a part of a `Date`
 * is the date (`events[0].date.year` → `events[0].date`). `wireCase` is the
 * encoded case, which names the variant of each sum type on the way. The part
 * of a path the codec does not describe stays as it is.
 */
export function fromWirePath(codec: CodecRef, wireCase: unknown, path: string): string {
  return wirePath(codec, wireCase, path).path;
}

/**
 * A TypeScript path, the part of the `Date` there that the wire path named, and
 * the codec of the value at the path when the codec describes all of it.
 */
type LocatedPath = { readonly path: string; readonly part?: string | undefined; readonly codec?: Codec | undefined };

/** Where a path walk stands: the codec of the value there (when known) and that value in the wire case. */
type Position = { readonly node: Codec | PositionalCodecs | undefined; readonly value: unknown };

function child(value: unknown, key: string | number): unknown {
  if (typeof key === "number") return Array.isArray(value) ? (value as unknown[])[key] : undefined;
  return isObject(value) ? value[key] : undefined;
}

function wirePath(codec: CodecRef, wireCase: unknown, path: string): LocatedPath {
  let segments: PathSegment[];
  try {
    segments = parsePath(path);
  } catch {
    return { path };
  }
  const out: PathSegment[] = [];
  let at: Position = { node: presentCodec(codec), value: wireCase };
  for (const segment of segments) {
    const { node, value } = at;
    const key = "index" in segment ? segment.index : segment.field;
    const step = (next: CodecRef | undefined, name: PathSegment = segment): Position => {
      out.push(name);
      return { node: next === undefined ? undefined : presentCodec(next), value: child(value, key) };
    };
    if (node === undefined) {
      at = step(undefined);
    } else if ("$values" in node) {
      at = step("index" in segment ? node.$values[segment.index] : undefined);
    } else if ("index" in segment) {
      at = step(node.kind === "list" ? node.item : undefined);
    } else {
      const field = segment.field;
      switch (node.kind) {
        case "date":
          if (hasOwn(DATE_PARTS, field)) return { path: formatPath(out), part: field };
          at = step(undefined);
          break;
        case "enum":
          if (field === "$variant") return { path: formatPath(out) };
          at = step(undefined);
          break;
        case "map":
          at = step(node.item);
          break;
        case "record": {
          const named = byWireName(node.fields).get(field);
          at = named === undefined ? step(undefined) : step(named[1], { field: named[0] });
          break;
        }
        case "union": {
          const variant = isObject(value) ? value["$variant"] : undefined;
          const spec = typeof variant === "string" && hasOwn(node.variants, variant) ? node.variants[variant] : undefined;
          if (field === "$values" && spec !== undefined && isPositional(spec)) {
            out.push(segment);
            at = { node: spec, value: child(value, "$values") };
            break;
          }
          const named = (spec !== undefined ? [spec] : Object.values(node.variants))
            .filter((candidate): candidate is CodecFields => !isPositional(candidate))
            .map((candidate) => byWireName(candidate).get(field))
            .find((found) => found !== undefined);
          at = named === undefined ? step(undefined) : step(named[1], { field: named[0] });
          break;
        }
        default:
          at = step(undefined);
      }
    }
  }
  const end = at.node;
  return { path: formatPath(out), codec: end === undefined || "$values" in end ? undefined : end };
}

function formatPath(segments: readonly PathSegment[]): string {
  return segments
    .map((segment, at) =>
      "index" in segment ? `[${String(segment.index)}]` : at === 0 ? segment.field : `.${segment.field}`,
    )
    .join("");
}

// Paths and setAt

/** One step of a concrete input path: a field (record field, map key, `$variant`, `$values`) or a list index. */
export type PathSegment = { readonly field: string } | { readonly index: number };

/** A path that does not parse, or that {@link setAt} cannot write. */
export class PathError extends Error {
  readonly path: string;

  constructor(path: string, message: string) {
    super(`${path}: ${message}`);
    this.name = "PathError";
    this.path = path;
  }
}

const FIRST_FIELD = /^[^.[\]]+/;
const NEXT_FIELD = /^\.([^.[\]]+)/;
const LIST_INDEX = /^\[(\d+)\]/;

/** Split a concrete input path such as `accounts[2].closingCashOre` into its steps. */
export function parsePath(path: string): PathSegment[] {
  const first = FIRST_FIELD.exec(path);
  if (first === null) throw new PathError(path, "a path starts with a field name");
  const segments: PathSegment[] = [{ field: first[0] }];
  let rest = path.slice(first[0].length);
  while (rest.length > 0) {
    const index = LIST_INDEX.exec(rest);
    if (index !== null) {
      segments.push({ index: Number(index[1]) });
      rest = rest.slice(index[0].length);
      continue;
    }
    const field = NEXT_FIELD.exec(rest);
    if (field?.[1] === undefined) throw new PathError(path, `unexpected \`${rest}\``);
    segments.push({ field: field[1] });
    rest = rest.slice(field[0].length);
  }
  return segments;
}

/**
 * Write `value` at a concrete input path, such as `accounts[2].closingCashOre`
 * or `taxpayer.outsideEndavu.$variant`, and return the new case. The case
 * passed in is not changed. A record on the way that was left out is created as
 * `{}`; a list element must already exist. Writing `$variant` keeps the object
 * when the variant is unchanged, and otherwise starts the new variant with its
 * fields unknown. The entry's `<entry>SetAt` is the same, typed by a need.
 */
export function setAt<C>(partialCase: C, path: string, value: unknown): C {
  return writeAt(partialCase, parsePath(path), 0, value, path) as C;
}

function writeAt(node: unknown, segments: readonly PathSegment[], at: number, value: unknown, path: string): unknown {
  const segment = segments[at];
  if (segment === undefined) return value;
  if ("index" in segment) {
    if (!Array.isArray(node) || segment.index >= node.length) {
      throw new PathError(path, `there is no list element [${String(segment.index)}] to write into`);
    }
    const copy: unknown[] = node.slice();
    copy[segment.index] = writeAt(copy[segment.index], segments, at + 1, value, path);
    return copy;
  }
  if (node === null || isNone(node)) {
    throw new PathError(path, `\`${segment.field}\` is inside a value given as none`);
  }
  if (node !== undefined && !isObject(node)) {
    throw new PathError(path, `\`${segment.field}\` is inside a value that is not an object`);
  }
  const record = node ?? {};
  if (segment.field === "$variant" && at === segments.length - 1) {
    return record["$variant"] === value ? record : { $variant: value };
  }
  const out: { [key: string]: unknown } = { ...record };
  put(out, segment.field, writeAt(record[segment.field], segments, at + 1, value, path));
  return out;
}

/** The pattern of a concrete path: list indices become `[]` (`accounts[2].kind` → `accounts[].kind`). */
export function pathPattern(path: string): string {
  return path.replace(/\[\d+\]/g, "[]");
}

const patternExpressions = new Map<string, RegExp>();

/** Whether a concrete path matches a pattern, where `[]` stands for any list index and `*` for any map key. */
export function matchesPathPattern(path: string, pattern: string): boolean {
  let expression = patternExpressions.get(pattern);
  if (expression === undefined) {
    const source = pattern
      .split("[]")
      .map((part) => part.split("*").map(escapeRegExp).join("[^.[\\]]+"))
      .join("\\[\\d+\\]");
    expression = new RegExp(`^${source}$`);
    patternExpressions.set(pattern, expression);
  }
  return expression.test(path);
}

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

declare const inputPathOf: unique symbol;

/**
 * A concrete input path of the pattern `P`, as a need names it: list indices
 * filled in as non-negative integers (`events[3].isin` of `events[].isin`), map
 * keys as their keys. Only the validators make one: paths come from the engine.
 */
export type InputPath<P extends string = string> = string & { readonly [inputPathOf]: P };

/**
 * A validator for the concrete input paths matching one of `patterns`, typed by
 * them; `undefined` accepts any path (a recursive input type has paths of any
 * depth).
 */
export function inputPathSchema<P extends string>(patterns: readonly P[] | undefined): z.ZodType<InputPath<P>> {
  return z.custom<InputPath<P>>(
    (value) =>
      typeof value === "string" &&
      (patterns === undefined ? parsesAsPath(value) : patterns.some((pattern) => matchesPathPattern(value, pattern))),
    "not an input path of this calculation",
  );
}

function parsesAsPath(path: string): boolean {
  try {
    parsePath(path);
    return true;
  } catch {
    return false;
  }
}

// Units of measure

/** A unit of measure: a number branded with its unit's name. */
export type UnitValue = number & { readonly __unit: string };

/** The validator of a unit of measure: a safe integer, typed `U` once parsed. */
export function unitSchema<U extends UnitValue>(unit: U["__unit"]): z.ZodType<U> {
  return z.custom<U>((value) => Number.isSafeInteger(value), `expected a safe integer (${unit})`);
}

/** `value` as a `U` of the unit `unit`; throws a `RangeError` unless it is a safe integer. */
export function unitValue<U extends UnitValue>(unit: U["__unit"], value: number): U {
  if (!Number.isSafeInteger(value)) throw new RangeError(`${unit}: expected a safe integer, got ${String(value)}`);
  return value as U;
}

// Needs and the result of a partial call

/** A source reference from the contract's field metadata. */
export type SourceReference = {
  label: string;
  role: string;
  binding: string;
  attachmentPath?: string | undefined;
  type?: string | undefined;
  value?: string | undefined;
  data?: unknown;
  text?: string | undefined;
  symbols?: string[] | undefined;
};

export const SourceReferenceSchema: z.ZodType<SourceReference> = z
  .object({
    label: z.string(),
    role: z.string(),
    binding: z.string(),
    attachmentPath: z.string().optional(),
    type: z.string().optional(),
    value: z.string().optional(),
    data: z.unknown().optional(),
    text: z.string().optional(),
    symbols: z.array(z.string()).optional(),
  })
  .strict();

/** The act a paragraph belongs to, from the header of its source document. */
export type Act = {
  /** The act's title, such as `aktiesparekontoloven`. */
  title: string;
  /** Its identifier, such as `LBK nr. 281 af 2025`, where its document gives one. */
  identifier?: string | undefined;
  url?: string | undefined;
};

/** A paragraph of an act, as a rule cites it: `§ 16, stk. 2`, the document quoting it, and the act. */
export type ParagraphReference = {
  paragraph: string;
  /** The quoted document, such as `sources/01-aktiesparekontoloven.txt`. */
  source?: string | undefined;
  act?: Act | undefined;
};

/** A rule or function that waits for a need's value, in its law section, with the paragraphs it cites. */
export type WaitingRule = {
  rule: string;
  /** The law section (anchor label) the rule is declared in. */
  section?: string | undefined;
  paragraphs?: ParagraphReference[] | undefined;
};

const ActSchema: z.ZodType<Act> = z
  .object({ title: z.string(), identifier: z.string().optional(), url: z.string().optional() })
  .strict();

const ParagraphReferenceSchema: z.ZodType<ParagraphReference> = z
  .object({ paragraph: z.string(), source: z.string().optional(), act: ActSchema.optional() })
  .strict();

const WaitingRuleSchema: z.ZodType<WaitingRule> = z
  .object({ rule: z.string(), section: z.string().optional(), paragraphs: z.array(ParagraphReferenceSchema).optional() })
  .strict();

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

/** A value another calculation entry provides: run `entry` for `subject` and write its result at `path`. */
export type EntryNeed<P extends string = string, K = ElementKey> = NeedMetadata<K> & {
  kind: "entry";
  /** The input path pattern of `path`. */
  pattern: P;
  /** The concrete input path the entry's result is written at. */
  path: InputPath<P>;
  /** The calculation entry that provides the value. */
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

/** `P` when it is one pattern; `never` for a union of several. */
type OnePattern<P, All = P> = P extends unknown ? ([All] extends [P] ? P : never) : never;

/**
 * The value an answer to `N` writes: the type at its pattern in `Patterns`.
 * `never` until `N` is the need of one pattern, so a need of several patterns
 * must be narrowed (by `need.pattern`) before it is answered.
 */
export type AnswerTo<Patterns, N extends { readonly pattern: string }> = Patterns[OnePattern<N["pattern"]> &
  keyof Patterns];

/**
 * The validator of an entry's needs: the pattern is one of `patterns` (any
 * pattern, when `undefined`) and the path matches it, and each element key is
 * the declared key of a list the path lies in.
 */
export function needSchema<P extends string, K extends ElementKey>(
  patterns: readonly P[] | undefined,
  keys: readonly ElementKeySchema<K>[],
): z.ZodType<Need<P, K>> {
  const metadata = {
    pattern: z.string(),
    path: z.string(),
    label: z.string().optional(),
    question: z.string().optional(),
    help: z.string().optional(),
    unit: z.string().optional(),
    sources: z.array(SourceReferenceSchema).optional(),
    waiting: z.tuple([WaitingRuleSchema], WaitingRuleSchema),
    keys: z.array(z.object({ path: z.string(), field: z.string(), value: z.unknown() }).strict()).optional(),
  };
  const schema = z
    .discriminatedUnion("kind", [
      z.object({ kind: z.literal("fact"), ...metadata }).strict(),
      z.object({ kind: z.literal("entry"), entry: z.string(), subject: z.string(), ...metadata }).strict(),
    ])
    .superRefine((need, context) => {
      const known = patterns === undefined || patterns.some((pattern) => pattern === need.pattern);
      if (!known || !matchesPathPattern(need.path, need.pattern)) {
        context.addIssue({ code: "custom", path: ["path"], message: "not an input path of this calculation" });
      }
      for (const [at, key] of (need.keys ?? []).entries()) {
        const declared = keys.find(
          (candidate) =>
            need.pattern.startsWith(candidate.element) &&
            matchesPathPattern(key.path, candidate.element) &&
            candidate.field === key.field &&
            candidate.value.safeParse(key.value).success,
        );
        if (declared === undefined) {
          context.addIssue({ code: "custom", path: ["keys", at], message: "not the declared key of a list the path lies in" });
        }
      }
    });
  // The refinement checks what the type says: `pattern` is one of `P` and `path` one of its paths.
  return schema as unknown as z.ZodType<Need<P, K>>;
}

/** A partial case whose result no completion of its unknown fields can change. */
export type Complete<R, N extends Need = Need> = {
  $variant: "Complete";
  result: R;
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

export function queryResultSchema<R, N extends Need>(
  result: z.ZodType<R>,
  need: z.ZodType<N>,
  path: z.ZodType<InputPath<N["pattern"]>>,
): z.ZodType<QueryResult<R, N>> {
  return z.discriminatedUnion("$variant", [
    z.object({ $variant: z.literal("Complete"), result, unusedPaths: z.array(path) }).strict(),
    z.object({ $variant: z.literal("Needs"), needs: z.array(need), later: z.array(need).optional() }).strict(),
  ]);
}

/** Wire names of the result's and sources' fields that differ in TypeScript. */
const SOURCE_NAMES: { readonly [wireName: string]: string } = { attachment_path: "attachmentPath" };
const RESULT_NAMES: { readonly [wireName: string]: string } = { unused_paths: "unusedPaths" };

function renamed(
  value: { readonly [key: string]: unknown },
  names: { readonly [wireName: string]: string },
): { [key: string]: unknown } {
  const out: { [key: string]: unknown } = {};
  for (const [key, item] of Object.entries(value)) put(out, names[key] ?? key, item);
  return out;
}

/** The TypeScript form of a partial call's result, its paths at TypeScript paths of the case. */
function queryResultFromWire(entry: CalculationEntry, wireCase: unknown, result: unknown): unknown {
  if (!isObject(result)) return result;
  const out = renamed(result, RESULT_NAMES);
  if (result["$variant"] === "Complete") {
    if (hasOwn(out, "result")) out["result"] = fromWire(entry.output, out["result"]);
    const unused = out["unusedPaths"];
    if (Array.isArray(unused)) {
      const paths = unused.map((item: unknown) => (typeof item === "string" ? fromWirePath(entry.input, wireCase, item) : item));
      // The parts of a date are one path in TypeScript.
      out["unusedPaths"] = [...new Set(paths)].sort((a, b) => byText(shown(a), shown(b)));
    }
  } else if (result["$variant"] === "Needs") {
    for (const list of ["needs", "later"]) {
      const needs = out[list];
      if (Array.isArray(needs)) out[list] = needs.map((need: unknown) => needFromWire(entry, wireCase, need));
    }
  }
  return out;
}

/** A need at TypeScript paths of the case, tagged with its pattern. */
function needFromWire(entry: CalculationEntry, wireCase: unknown, need: unknown): unknown {
  if (!isObject(need)) return need;
  const translated = { ...need };
  const at = need["path"];
  if (typeof at === "string") {
    const path = fromWirePath(entry.input, wireCase, at);
    translated["path"] = path;
    // The pattern the path matches; a path of a recursive type has its list indices as `[]`.
    translated["pattern"] = entry.patterns.find((pattern) => matchesPathPattern(path, pattern)) ?? pathPattern(path);
  }
  const subject = need["subject"];
  if (typeof subject === "string") translated["subject"] = fromWirePath(entry.input, wireCase, subject);
  const sources = need["sources"];
  if (Array.isArray(sources)) {
    translated["sources"] = sources.map((source: unknown) => (isObject(source) ? renamed(source, SOURCE_NAMES) : source));
  }
  const keys = need["keys"];
  if (Array.isArray(keys)) translated["keys"] = keys.map((key: unknown) => elementKeyFromWire(entry, wireCase, key));
  return translated;
}

/** An element key at the element's TypeScript path, with its field's TypeScript name and value. */
function elementKeyFromWire(entry: CalculationEntry, wireCase: unknown, key: unknown): unknown {
  if (!isObject(key) || typeof key["path"] !== "string" || typeof key["field"] !== "string") return key;
  const located = wirePath(entry.input, wireCase, key["path"]);
  const field = located.codec?.kind === "record" ? byWireName(located.codec.fields).get(key["field"]) : undefined;
  if (field === undefined) return { ...key, path: located.path };
  return { path: located.path, field: field[0], value: fromWire(field[1], key["value"]) };
}

// Calls through an engine transport

const CASE_ID = "case-1";

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
 * One calculation entry: its name, its contract hash, the wire codecs of its
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
 * diagnostics for the case.
 */
export type CalculationErrorKind = "input" | "output" | "envelope" | "diagnostics";

/** A diagnostic the engine reported for the case, at its path in the input document. */
export type CalculationDiagnostic = { caseId: string; path: string; message: string };

export class CalculationError extends Error {
  readonly kind: CalculationErrorKind;
  readonly entry: string;
  /** The validation issues, for `input` and `output`. */
  readonly issues: readonly unknown[];
  /** The engine's diagnostics, for `diagnostics`. */
  readonly diagnostics: readonly CalculationDiagnostic[];

  constructor(
    kind: CalculationErrorKind,
    entry: string,
    message: string,
    details: { issues?: readonly unknown[]; diagnostics?: readonly CalculationDiagnostic[] } = {},
  ) {
    super(`${entry}: ${message}`);
    this.name = "CalculationError";
    this.kind = kind;
    this.entry = entry;
    this.issues = details.issues ?? [];
    this.diagnostics = details.diagnostics ?? [];
  }
}

function issueSummary(issues: readonly { path: readonly PropertyKey[]; message: string }[]): string {
  const first = issues[0];
  if (first === undefined) return "invalid";
  const at = first.path.length === 0 ? "$" : first.path.map(String).join(".");
  return `${String(issues.length)} issue${issues.length === 1 ? "" : "s"}; first at ${at}: ${first.message}`;
}

/** The one case of an output document (`result`, and `explanation` when explained), its envelope checked. */
function caseOutput(
  entry: CalculationEntry,
  outputSchema: string,
  document: unknown,
): { readonly [key: string]: unknown } {
  const parsed: unknown = typeof document === "string" ? JSON.parse(document) : document;
  if (!isObject(parsed) || !("$futuruna" in parsed)) return { result: parsed };
  const envelope = parsed["$futuruna"];
  const name = entry.name;
  if (!isObject(envelope)) throw new CalculationError("envelope", name, "`$futuruna` is not an object");
  if (envelope["schema_hash"] !== undefined && envelope["schema_hash"] !== entry.schemaHash) {
    throw new CalculationError(
      "envelope",
      name,
      `the engine runs contract ${shown(envelope["schema_hash"])}, but this SDK was generated for ${entry.schemaHash}`,
    );
  }
  if (envelope["entry"] !== undefined && envelope["entry"] !== name) {
    throw new CalculationError("envelope", name, `the output is for entry ${shown(envelope["entry"])}`);
  }
  if (envelope["schema"] !== undefined && envelope["schema"] !== outputSchema) {
    throw new CalculationError("envelope", name, `expected a ${outputSchema} document, got ${shown(envelope["schema"])}`);
  }
  const diagnostics = parsed["diagnostics"];
  if (Array.isArray(diagnostics) && diagnostics.length > 0) {
    const list = diagnostics.filter(isObject).map((diagnostic) => ({
      caseId: shown(diagnostic["case_id"]),
      path: shown(diagnostic["path"]),
      message: shown(diagnostic["message"]),
    }));
    const first = list[0];
    const summary = first === undefined ? "" : `; first at ${first.path}: ${first.message}`;
    throw new CalculationError("diagnostics", name, `${String(diagnostics.length)} diagnostic(s)${summary}`, {
      diagnostics: list,
    });
  }
  const results = parsed["results"];
  const only: unknown = Array.isArray(results) && results.length === 1 ? (results as unknown[])[0] : undefined;
  if (!isObject(only) || !("result" in only)) {
    throw new CalculationError("envelope", name, "the output document does not hold exactly one case result");
  }
  return only;
}

/** Validate a case and encode it as the input document `runa call` reads. */
function inputDocument<I>(entry: CalculationEntry, inputSchema: z.ZodType<I>, input: I) {
  const checked = inputSchema.safeParse(input);
  if (!checked.success) {
    const issues = checked.error.issues;
    throw new CalculationError("input", entry.name, `invalid input: ${issueSummary(issues)}`, { issues });
  }
  const wireCase = toWire(entry.input, checked.data);
  const document = {
    $futuruna: { schema: INPUT_SCHEMA, schema_hash: entry.schemaHash, entry: entry.name },
    cases: [{ case_id: CASE_ID, input_status: "ready", input: wireCase }],
  };
  return { wireCase, document };
}

/** Decode a result with `decode` and validate it; either failing is an `output` error. */
function validated<O>(entry: CalculationEntry, schema: z.ZodType<O>, decode: () => unknown): O {
  let value: unknown;
  try {
    value = decode();
  } catch (error) {
    if (!(error instanceof WireError)) throw error;
    throw new CalculationError("output", entry.name, `invalid result: ${error.message}`, { issues: error.issues });
  }
  const checked = schema.safeParse(value);
  if (!checked.success) {
    const issues = checked.error.issues;
    throw new CalculationError("output", entry.name, `invalid result: ${issueSummary(issues)}`, { issues });
  }
  return checked.data;
}

/**
 * Calculate `entry` on a complete case: validate it, encode it, run it through
 * `transport.call`, check the envelope, and decode and validate the result.
 */
export async function callEntry<I, O>(
  transport: EngineTransport,
  entry: CalculationEntry,
  inputSchema: z.ZodType<I>,
  outputSchema: z.ZodType<O>,
  input: I,
): Promise<O> {
  const { document } = inputDocument(entry, inputSchema, input);
  const output = await transport.call(entry.name, document);
  const result = caseOutput(entry, OUTPUT_SCHEMA, output)["result"];
  return validated(entry, outputSchema, () => fromWire(entry.output, result));
}

/**
 * Query `entry` on a partial case through `transport.callPartial`: `Complete`
 * or `Needs`, with the result decoded, every path a TypeScript path and every
 * need tagged with its pattern.
 */
export async function queryEntry<I, R, N extends Need>(
  transport: EngineTransport,
  entry: CalculationEntry,
  inputSchema: z.ZodType<I>,
  resultSchema: z.ZodType<QueryResult<R, N>>,
  input: I,
): Promise<QueryResult<R, N>> {
  const { wireCase, document } = inputDocument(entry, inputSchema, input);
  const output = await transport.callPartial(entry.name, document);
  const result = caseOutput(entry, PARTIAL_OUTPUT_SCHEMA, output)["result"];
  return validated(entry, resultSchema, () => queryResultFromWire(entry, wireCase, result));
}

// Explanations

/** How much an explained call reports: `direct` contributors of each value, or also the `full` derivation. */
export type ExplainDepth = "direct" | "full";

/** A leaf of a result or a case: a number, string or boolean, an enum's variant, or a `Date` as an ISO date. */
export type ExplainedLeaf = string | number | boolean | null;

/** A paragraph a rule clause cites, with its quoted text. */
export type CitedParagraph = ParagraphReference & { text: string };

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
export type InputContributor = { $variant: "Input"; path: string; value: ExplainedLeaf };

export type Contributor = RuleContributor | InputContributor;

/**
 * What produced a value: a rule clause, the input at a TypeScript path of the
 * case, a computation outside any rule, or a constant of the model.
 */
export type ExplainedBy =
  | ExplainedRule
  | { $variant: "Input"; path: string }
  | { $variant: "Computed" }
  | { $variant: "Constant" };

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
  attachments: { role: string; binding?: string | undefined; value: unknown }[];
  quote: string;
};

/**
 * Where every leaf of a result came from (`docs/rfcs/explanations.md`), at the
 * TypeScript paths of the result and the case: an enum is one leaf, and a
 * `Date` is one leaf, its ISO date, unless its parts came from different
 * places; then each part is explained at `<date>.year`, `.month` and `.day`.
 */
export type Explanation = {
  /** Every leaf of the result by its TypeScript path. */
  values: { [path: string]: ExplainedValue };
  sections: { [label: string]: ExplainedSection };
  /** With depth `full`: every rule node a value was computed from. */
  derivation?:
    | {
        [node: string]: {
          value: ExplainedLeaf;
          by: ExplainedRule;
          from: Contributor[];
          because?: Contributor[] | undefined;
        };
      }
    | undefined;
  /** The assumptions of the law sections whose clauses gave the result, and of the entry. */
  assumptions?: { section: string; binding?: string | undefined; value: unknown }[] | undefined;
};

/** A result with its explanation. */
export type Explained<R> = { readonly result: R; readonly explanation: Explanation };

/** A partial call's result; a `Complete` result carries an explanation. */
export type ExplainedQuery<R> = { readonly result: R; readonly explanation?: Explanation | undefined };

type JsonObject = { [key: string]: unknown };

/** The ISO date of a `Date`'s parts, when they are one. */
function isoDateOf(parts: { readonly [part: string]: unknown }): string | undefined {
  const date = dateFromWire({ year: parts["year"], month: parts["month"], day: parts["day"] });
  return isIsoDate(date) ? date : undefined;
}

/** `items` without repeats, by their canonical JSON. */
function distinct<T>(items: readonly T[]): T[] {
  const seen = new Set<string>();
  return items.filter((item) => {
    const key = canonicalJson(item);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

/** Input contributors at TypeScript paths; the three parts of one input `Date` become the date. */
function contributorsFromWire(input: (path: string) => LocatedPath, from: readonly unknown[]): unknown[] {
  const out: unknown[][] = [];
  const dates = new Map<string, { at: number; parts: Map<string, JsonObject> }>();
  for (const item of from) {
    if (!isObject(item) || item["$variant"] !== "Input" || typeof item["path"] !== "string") {
      out.push([item]);
      continue;
    }
    const located = input(item["path"]);
    const translated: JsonObject = { ...item, path: located.path };
    if (located.part === undefined) {
      out.push([translated]);
      continue;
    }
    let date = dates.get(located.path);
    if (date === undefined) {
      date = { at: out.length, parts: new Map() };
      dates.set(located.path, date);
      out.push([]);
    }
    date.parts.set(located.part, translated);
  }
  for (const [path, { at, parts }] of dates) {
    const value = parts.size === 3 ? isoDateOf(Object.fromEntries([...parts].map(([part, item]) => [part, item["value"]]))) : undefined;
    out[at] =
      value !== undefined
        ? [{ $variant: "Input", path, value }]
        : [...parts].map(([part, item]) => ({ ...item, path: `${path}.${part}` }));
  }
  return distinct(out.flat(1));
}

function explainedFromWire(input: (path: string) => LocatedPath, value: unknown): JsonObject {
  const out = isObject(value) ? renamed(value, EXPLAINED_NAMES) : {};
  const by = out["by"];
  if (isObject(by) && by["$variant"] === "Input" && typeof by["path"] === "string") {
    out["by"] = { ...by, path: input(by["path"]).path };
  }
  for (const list of ["from", "because"]) {
    const items = out[list];
    if (Array.isArray(items)) out[list] = contributorsFromWire(input, items);
  }
  return out;
}

const EXPLAINED_NAMES: { readonly [wireName: string]: string } = { from_omitted: "fromOmitted" };

/** The explained parts of one `Date` of a result as one value, when they share what produced them. */
function explainedDate(parts: ReadonlyMap<string, JsonObject>): JsonObject | undefined {
  const explained = [...parts.values()];
  const first = explained[0];
  if (parts.size !== 3 || first === undefined) return undefined;
  if (!explained.every((part) => canonicalJson(part["by"]) === canonicalJson(first["by"]))) return undefined;
  const value = isoDateOf(Object.fromEntries([...parts].map(([part, item]) => [part, item["value"]])));
  if (value === undefined) return undefined;
  const out: JsonObject = { value, by: first["by"] };
  for (const list of ["from", "because"]) {
    const items: unknown[] = explained.flatMap((part) => {
      const listed = part[list];
      return Array.isArray(listed) ? (listed as unknown[]) : [];
    });
    if (explained.some((part) => part[list] !== undefined)) out[list] = distinct(items);
  }
  const omitted = explained.map((part) => part["fromOmitted"]).filter((count): count is number => typeof count === "number");
  if (omitted.length > 0) out["fromOmitted"] = Math.max(...omitted);
  return out;
}

/**
 * The TypeScript form of an explanation: result and input paths are the
 * TypeScript paths of `wireResult` and `wireCase` (an enum at its own path, a
 * `Date` as a whole) and `from_omitted` is `fromOmitted`. Leaf values are
 * already TypeScript values: numbers, strings, booleans and variant names.
 */
function explanationFromWire(entry: CalculationEntry, wireCase: unknown, wireResult: unknown, explanation: unknown): Explanation {
  if (!isObject(explanation) || !isObject(explanation["values"]) || !isObject(explanation["sections"])) {
    throw new CalculationError("output", entry.name, "the explanation is not an object with values and sections");
  }
  const input = (path: string) => wirePath(entry.input, wireCase, path);
  const groups = new Map<string, Map<string, JsonObject>>();
  const values: [string, unknown][] = [];
  for (const [path, value] of Object.entries(explanation["values"])) {
    const located = wirePath(entry.output, wireResult, path);
    const explained = explainedFromWire(input, value);
    if (located.part === undefined) {
      values.push([located.path, explained]);
      continue;
    }
    let parts = groups.get(located.path);
    if (parts === undefined) {
      parts = new Map();
      groups.set(located.path, parts);
    }
    parts.set(located.part, explained);
  }
  for (const [path, parts] of groups) {
    const date = explainedDate(parts);
    if (date !== undefined) values.push([path, date]);
    else for (const [part, explained] of parts) values.push([`${path}.${part}`, explained]);
  }
  const translated: JsonObject = {};
  for (const [path, value] of values.sort(([left], [right]) => byText(left, right))) put(translated, path, value);
  const out: JsonObject = { ...explanation, values: translated };
  const derivation = explanation["derivation"];
  if (isObject(derivation)) {
    const nodes: JsonObject = {};
    for (const [node, value] of Object.entries(derivation)) put(nodes, node, explainedFromWire(input, value));
    out["derivation"] = nodes;
  }
  // The engine's explanation document, its paths translated; `runa call --explain` defines its shape.
  return out as Explanation;
}

/**
 * Calculate `entry` and explain it, through `transport.callExplained`
 * (`runa call --explain`): the result as {@link callEntry} returns it, and
 * where every leaf of it came from.
 */
export async function explainEntry<I, O>(
  transport: ExplainingTransport,
  entry: CalculationEntry,
  depth: ExplainDepth,
  inputSchema: z.ZodType<I>,
  outputSchema: z.ZodType<O>,
  input: I,
): Promise<Explained<O>> {
  const { wireCase, document } = inputDocument(entry, inputSchema, input);
  const output = caseOutput(entry, OUTPUT_SCHEMA, await transport.callExplained(entry.name, document, depth === "full"));
  const wireResult = output["result"];
  const result = validated(entry, outputSchema, () => fromWire(entry.output, wireResult));
  if (output["explanation"] === undefined) {
    throw new CalculationError("output", entry.name, "the engine returned no explanation");
  }
  return { result, explanation: explanationFromWire(entry, wireCase, wireResult, output["explanation"]) };
}

/**
 * Query `entry` and explain a `Complete` result, through
 * `transport.callPartialExplained` (`runa call --partial --explain`).
 */
export async function explainQueryEntry<I, R, N extends Need>(
  transport: ExplainingTransport,
  entry: CalculationEntry,
  depth: ExplainDepth,
  inputSchema: z.ZodType<I>,
  resultSchema: z.ZodType<QueryResult<R, N>>,
  input: I,
): Promise<ExplainedQuery<QueryResult<R, N>>> {
  const { wireCase, document } = inputDocument(entry, inputSchema, input);
  const reply = await transport.callPartialExplained(entry.name, document, depth === "full");
  const output = caseOutput(entry, PARTIAL_OUTPUT_SCHEMA, reply);
  const wire = output["result"];
  const result = validated(entry, resultSchema, () => queryResultFromWire(entry, wireCase, wire));
  if (output["explanation"] === undefined) return { result };
  const wireResult = isObject(wire) ? wire["result"] : undefined;
  return { result, explanation: explanationFromWire(entry, wireCase, wireResult, output["explanation"]) };
}

// The resolver loop

/** One calculation entry running for one subject, on the resolver stack. */
export type EntryFrame = { readonly entry: string; readonly subject: string };

/**
 * What a resolver returns: an answer to write at the need's path, `ask` when
 * only the customer knows, or the resolution of another entry
 * ({@link ResolverContext.runEntry}).
 */
export type ResolverAnswer<V = unknown> =
  | { readonly kind: "answer"; readonly value: V }
  | { readonly kind: "ask" }
  | { readonly kind: "entry"; readonly resolution: Resolution<unknown, V> };

/** The resolver reply for a need only the customer can answer. */
export const ask: { readonly kind: "ask" } = Object.freeze({ kind: "ask" });

/** The resolver reply that answers a need with `value`. */
export function answer<V>(value: V): ResolverAnswer<V> {
  return { kind: "answer", value };
}

export interface ResolverContext<C> {
  /** The case sent in this round. */
  readonly partialCase: C;
  /** The entries running above this loop, outermost first. */
  readonly frames: readonly EntryFrame[];
  /**
   * Resolve another entry for this need, through the same loop. The frame is
   * the need's `entry` and `subject` unless given; a frame already on the
   * stack is a cycle and throws {@link ResolveCycleError}.
   */
  runEntry<C2, R2, N2 extends Need>(
    query: Query<C2, R2, N2>,
    initialCase: C2,
    resolvers: Resolvers<C2, N2>,
    frame?: EntryFrame,
  ): Promise<ResolverAnswer<R2>>;
}

/** A partial call of one entry, such as `(c) => taxReportQuery(transport, c)`. */
export type Query<C, R, N extends Need = Need> = (partialCase: C) => Promise<QueryResult<R, N>>;

/** Answers a need `N` with a value `V`, asks, or resolves another entry. */
export type Resolver<C, V, N extends Need = Need> = (
  need: N,
  context: ResolverContext<C>,
) => ResolverAnswer<V> | Promise<ResolverAnswer<V>>;

/**
 * Resolvers keyed by a need's pattern, with `[]` for any list index
 * (`accounts[].closingCashOre`) and `*` for any map key; each receives the
 * needs of its pattern. A need no resolver matches is a question.
 */
export type Resolvers<C, N extends Need = Need> = {
  readonly [P in N["pattern"]]?: Resolver<C, unknown, Extract<N, { pattern: P }>>;
};

/** An entry need whose own loop stopped at questions about its subject. */
export type PendingEntry = { readonly need: Need; readonly resolution: Resolution<unknown, unknown> };

/**
 * Where a resolver loop stopped: `Complete`, or `Questions` when a round
 * produced no answer. The partial case holds every answer written so far.
 */
export type Resolution<C, R, N extends Need = Need> =
  | {
      readonly $variant: "Complete";
      readonly result: R;
      readonly unusedPaths: readonly InputPath<N["pattern"]>[];
      readonly partialCase: C;
      readonly rounds: number;
    }
  | {
      readonly $variant: "Questions";
      /** The needs to ask the customer. */
      readonly questions: readonly N[];
      /** Entry needs waiting on questions in another entry's case. */
      readonly pending: readonly PendingEntry[];
      readonly partialCase: C;
      readonly rounds: number;
    };

export type ResolveOptions = {
  /** The entries already running above this loop; nested loops pass their stack. */
  readonly frames?: readonly EntryFrame[];
};

/** The resolver loop cannot continue. */
export class ResolveError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ResolveError";
  }
}

/** Two entries need each other for the same subjects: a model error; join them into one entry. */
export class ResolveCycleError extends ResolveError {
  readonly frames: readonly EntryFrame[];

  constructor(frames: readonly EntryFrame[]) {
    super(
      `calculation entries need each other: ${frames.map((frame) => `${frame.entry}(${frame.subject})`).join(" → ")}; model them as one joint entry`,
    );
    this.name = "ResolveCycleError";
    this.frames = frames;
  }
}

/**
 * Resolve a partial case: query it, let the resolver of each need's pattern
 * answer it, write the answers into the case with {@link setAt}, and query
 * again. Stops with `Complete`, or with `Questions` when a round adds no
 * answer. A path that is still needed after it was answered is a
 * {@link ResolveError}; an entry that needs itself for the same subject is a
 * {@link ResolveCycleError}.
 */
export async function resolve<C, R, N extends Need>(
  query: Query<C, R, N>,
  initialCase: C,
  resolvers: Resolvers<C, N>,
  options: ResolveOptions = {},
): Promise<Resolution<C, R, N>> {
  const frames = options.frames ?? [];
  // Each resolver is keyed by the pattern of the needs it receives.
  const byPattern = resolvers as { readonly [pattern: string]: Resolver<C, unknown, N> | undefined };
  const answered = new Set<string>();
  let partialCase = initialCase;
  for (let rounds = 1; ; rounds += 1) {
    const result = await query(partialCase);
    if (result.$variant === "Complete") {
      return { $variant: "Complete", result: result.result, unusedPaths: result.unusedPaths, partialCase, rounds };
    }
    const questions: N[] = [];
    const pending: PendingEntry[] = [];
    let next = partialCase;
    for (const need of result.needs) {
      if (answered.has(need.path)) {
        throw new ResolveError(`the engine still needs \`${need.path}\` after it was answered`);
      }
      const resolver = hasOwn(byPattern, need.pattern) ? byPattern[need.pattern] : undefined;
      if (resolver === undefined) {
        questions.push(need);
        continue;
      }
      const reply = await resolver(need, {
        partialCase,
        frames,
        async runEntry(nestedQuery, nestedCase, nestedResolvers, frame) {
          const target = frame ?? (need.kind === "entry" ? { entry: need.entry, subject: need.subject } : undefined);
          if (target === undefined) {
            throw new ResolveError(`\`${need.path}\` is a fact need; give runEntry the frame of the entry it runs`);
          }
          const stack = [...frames, target];
          if (frames.some((active) => active.entry === target.entry && active.subject === target.subject)) {
            throw new ResolveCycleError(stack);
          }
          const resolution = await resolve(nestedQuery, nestedCase, nestedResolvers, { frames: stack });
          return { kind: "entry", resolution };
        },
      });
      if (reply.kind === "ask") {
        questions.push(need);
      } else if (reply.kind === "answer") {
        next = setAt(next, need.path, reply.value);
        answered.add(need.path);
      } else if (reply.resolution.$variant === "Complete") {
        next = setAt(next, need.path, reply.resolution.result);
        answered.add(need.path);
      } else {
        pending.push({ need, resolution: reply.resolution });
      }
    }
    if (next === partialCase) return { $variant: "Questions", questions, pending, partialCase, rounds };
    partialCase = next;
  }
}
