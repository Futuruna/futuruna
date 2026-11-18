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
    if ("key" in segment) {
      // An entry of an open collection by its key: the key field at its TypeScript name.
      const item = node !== undefined && !("$values" in node) && node.kind === "list" ? presentCodec(node.item) : undefined;
      const named = item?.kind === "record" ? byWireName(item.fields).get(segment.key) : undefined;
      out.push({ key: named?.[0] ?? segment.key, value: segment.value });
      const entry = Array.isArray(value)
        ? (value as unknown[]).find((candidate) => isObject(candidate) && candidate[segment.key] === segment.value)
        : undefined;
      at = { node: item, value: entry };
      continue;
    }
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
      "key" in segment
        ? `[${segment.key}=${JSON.stringify(segment.value)}]`
        : "index" in segment
          ? `[${String(segment.index)}]`
          : at === 0
            ? segment.field
            : `.${segment.field}`,
    )
    .join("");
}

// Paths and setAt

/**
 * One step of a concrete input path: a field (record field, map key, `$variant`,
 * `$values`), a list index, or the entry of an open collection by its key
 * (`[day="2027-02-20"]`: the key field and its value).
 */
export type PathSegment =
  | { readonly field: string }
  | { readonly index: number }
  | { readonly key: string; readonly value: string | number };

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
const KEY_VALUE = String.raw`"(?:[^"\\]|\\.)*"|-?\d+`;
const ENTRY_KEY = new RegExp(String.raw`^\[([^.[\]=]+)=(${KEY_VALUE})\]`);
/** A list index or an entry's key selector, as `[]` in a pattern stands for either. */
const ELEMENT = String.raw`\[(?:\d+|[^.[\]=]+=(?:${KEY_VALUE}))\]`;

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
    const entry = ENTRY_KEY.exec(rest);
    if (entry?.[1] !== undefined && entry[2] !== undefined) {
      segments.push({ key: entry[1], value: JSON.parse(entry[2]) as string | number });
      rest = rest.slice(entry[0].length);
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
 * fields unknown. The entry of an open collection (`balances[day="2027-02-20"]`)
 * is the whole entry: written into the entry with that key, or added to the
 * collection, its key set to the path's. The entry's `<entry>SetAt` is the same,
 * typed by a need.
 */
export function setAt<C>(partialCase: C, path: string, value: unknown): C {
  return writeAt(partialCase, parsePath(path), 0, value, path) as C;
}

function writeAt(node: unknown, segments: readonly PathSegment[], at: number, value: unknown, path: string): unknown {
  const segment = segments[at];
  if (segment === undefined) return value;
  if ("key" in segment) {
    const list: unknown = node ?? [];
    if (!Array.isArray(list)) throw new PathError(path, `the entry \`${segment.key}\` = ${JSON.stringify(segment.value)} is inside a value that is not a list`);
    const copy: unknown[] = (list as unknown[]).slice();
    const found = copy.findIndex((entry) => isObject(entry) && entry[segment.key] === segment.value);
    if (found >= 0) {
      copy[found] = writeAt(copy[found], segments, at + 1, value, path);
      return copy;
    }
    if (at !== segments.length - 1) {
      throw new PathError(path, `there is no entry whose \`${segment.key}\` is ${JSON.stringify(segment.value)} to write into`);
    }
    copy.push(keyedValue(value, segment, path));
    return copy;
  }
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

/** A new entry of an open collection, with the key its path names. */
function keyedValue(value: unknown, segment: { readonly key: string; readonly value: string | number }, path: string): unknown {
  if (!isObject(value)) throw new PathError(path, "an entry of an open collection is a record");
  const given = value[segment.key];
  if (given === undefined) return { ...value, [segment.key]: segment.value };
  if (given !== segment.value) {
    throw new PathError(path, `the entry's \`${segment.key}\` is ${JSON.stringify(given)}, and the need asks for ${JSON.stringify(segment.value)}`);
  }
  return value;
}

/**
 * The pattern of a concrete path: list indices and entry keys become `[]`
 * (`accounts[2].kind` → `accounts[].kind`, `balances[day="2027-02-20"]` → `balances[]`).
 */
export function pathPattern(path: string): string {
  return path.replace(new RegExp(ELEMENT, "g"), "[]");
}

const patternExpressions = new Map<string, RegExp>();

/**
 * Whether a concrete path matches a pattern, where `[]` stands for any list
 * index or entry key and `*` for any map key.
 */
export function matchesPathPattern(path: string, pattern: string): boolean {
  let expression = patternExpressions.get(pattern);
  if (expression === undefined) {
    const source = pattern
      .split("[]")
      .map((part) => part.split("*").map(escapeRegExp).join("[^.[\\]]+"))
      .join(ELEMENT);
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

/** A unit's declared scale: `per` of the unit make one `of` (`{ of: "DKK", per: 100 }` for øre). */
export type UnitScale = { readonly of: string; readonly per: number };

/** The decimal places of `1 / per`, when `per` is built from 2s and 5s only; otherwise `undefined`. */
function scaleDigits(per: number): number | undefined {
  let rest = per;
  let twos = 0;
  let fives = 0;
  while (rest % 2 === 0) {
    rest /= 2;
    twos += 1;
  }
  while (rest % 5 === 0) {
    rest /= 5;
    fives += 1;
  }
  return rest === 1 ? Math.max(twos, fives) : undefined;
}

function amountOf(scale: UnitScale, amount: string): string {
  return scale.of === "1" ? amount : `${amount} ${scale.of}`;
}

/**
 * `amount`, a decimal number of the unit's base as text (`"1740.25"` DKK), as
 * a `U`: exact, in integer arithmetic, never through a float. An amount finer
 * than one `U`, text that is not a decimal number, and a result beyond a safe
 * integer throw a {@link CalculationError} of kind `amount` naming the unit
 * and the amount. The SDK's converters (`øre.fromDecimal`) call it.
 */
export function unitFromDecimal<U extends UnitValue>(unit: U["__unit"], scale: UnitScale, amount: string): U {
  const match = /^([+-]?)(\d+)(?:\.(\d+))?$/.exec(amount);
  if (match === null) {
    throw new CalculationError("amount", unit, `\`${amount}\` is not a decimal number${scale.of === "1" ? "" : ` of ${scale.of}`}`);
  }
  const [, sign = "", whole = "", fraction = ""] = match;
  const scaled = BigInt(whole + fraction) * BigInt(scale.per);
  const divisor = 10n ** BigInt(fraction.length);
  if (scaled % divisor !== 0n) {
    throw new CalculationError(
      "amount",
      unit,
      `${amountOf(scale, amount)} is finer than one ${unit} (1/${String(scale.per)}${scale.of === "1" ? "" : ` ${scale.of}`})`,
    );
  }
  const magnitude = scaled / divisor;
  if (magnitude > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new CalculationError("amount", unit, `${amountOf(scale, amount)} is beyond a safe integer of ${unit}`);
  }
  const value = Number(magnitude);
  return (sign === "-" && value !== 0 ? -value : value) as U;
}

/**
 * `value` as a decimal number of the unit's base, exact, with as many
 * decimals as one `U` has (`"1740.25"` for 174025 øre, `"1740.00"` for
 * 174000). Throws a `RangeError` unless `value` is a safe integer, or when the
 * scale is not built from 2s and 5s (a unit the SDK gives no converters).
 */
export function unitToDecimal<U extends UnitValue>(unit: U["__unit"], scale: UnitScale, value: U): string {
  if (!Number.isSafeInteger(value)) throw new RangeError(`${unit}: expected a safe integer, got ${String(value)}`);
  const digits = scaleDigits(scale.per);
  if (digits === undefined) throw new RangeError(`${unit}: 1/${String(scale.per)} has no finite decimal form`);
  const scaled = BigInt(value) * (10n ** BigInt(digits) / BigInt(scale.per));
  const negative = scaled < 0n;
  const text = (negative ? -scaled : scaled).toString().padStart(digits + 1, "0");
  const whole = text.slice(0, text.length - digits);
  const fraction = digits > 0 ? `.${text.slice(text.length - digits)}` : "";
  return `${negative ? "-" : ""}${whole}${fraction}`;
}

// Named scalar types

/** A named scalar type: a string or number branded with its type's name. */
export type ScalarValue = (string | number) & { readonly __scalar: string };

function isScalarPrimitive(of: "String" | "Int", value: unknown): boolean {
  return of === "Int" ? Number.isSafeInteger(value) : typeof value === "string";
}

/**
 * The validator of a named scalar type: a string (or a safe integer), typed
 * `S` once parsed. The engine runs the type's checks.
 */
export function scalarSchema<S extends ScalarValue>(scalar: S["__scalar"], of: "String" | "Int"): z.ZodType<S> {
  return z.custom<S>(
    (value) => isScalarPrimitive(of, value),
    `expected ${of === "Int" ? "a safe integer" : "a string"} (${scalar})`,
  );
}

/** `value` as an `S`; throws a `RangeError` unless it is a string (or a safe integer). */
export function scalarValue<S extends ScalarValue>(scalar: S["__scalar"], of: "String" | "Int", value: unknown): S {
  if (!isScalarPrimitive(of, value)) {
    throw new RangeError(`${scalar}: expected ${of === "Int" ? "a safe integer" : "a string"}, got ${String(value)}`);
  }
  return value as S;
}

// Needs and the result of a partial call

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

export const SourceReferenceSchema: z.ZodType<SourceReference> = z
  .object({
    label: z.string(),
    role: z.string(),
    binding: z.string(),
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

/**
 * The amending act's instruction behind a passage cited `as amended`: the act
 * (`LOV nr. 1691 af 30/12/2024`), its paragraph (`§ 4, nr. 1`) and the
 * instruction line (`1. § 8 a, stk. 1, 1. pkt., affattes således:`).
 */
export type Amendment = { act: string; paragraph: string; instruction: string };

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

const ActSchema: z.ZodType<Act> = z
  .object({ title: z.string(), identifier: z.string().optional(), url: z.string().optional() })
  .strict();

const AmendmentSchema: z.ZodType<Amendment> = z
  .object({ act: z.string(), paragraph: z.string(), instruction: z.string() })
  .strict();

const ParagraphReferenceSchema: z.ZodType<ParagraphReference> = z
  .object({ paragraph: z.string(), source: z.string().optional(), act: ActSchema.optional(), asAmendedBy: AmendmentSchema.optional() })
  .strict();

const PARAGRAPH_NAMES: { readonly [wireName: string]: string } = { as_amended_by: "asAmendedBy" };

/** The list at `key` of `object`, each cited paragraph with TypeScript names (`as_amended_by` is `asAmendedBy`). */
function withParagraphsFromWire(object: unknown, key: string): unknown {
  if (!isObject(object)) return object;
  const list = object[key];
  if (!Array.isArray(list)) return object;
  return { ...object, [key]: list.map((paragraph: unknown) => (isObject(paragraph) ? renamed(paragraph, PARAGRAPH_NAMES) : paragraph)) };
}

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

export const CalculationAssumptionSchema: z.ZodType<CalculationAssumption> = z
  .object({
    section: z.string().optional(),
    rule: z.string().optional(),
    binding: z.string().optional(),
    value: z.unknown(),
  })
  .strict();

/** A calculation's result and the assumptions it rests on, each once, in the order the model writes them. */
export type Calculated<R> = { readonly result: R; readonly assumptions: readonly CalculationAssumption[] };

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

export function queryResultSchema<R, N extends Need>(
  result: z.ZodType<R>,
  need: z.ZodType<N>,
  path: z.ZodType<InputPath<N["pattern"]>>,
): z.ZodType<QueryResult<R, N>> {
  return z.discriminatedUnion("$variant", [
    z
      .object({
        $variant: z.literal("Complete"),
        result,
        assumptions: z.array(CalculationAssumptionSchema),
        unusedPaths: z.array(path),
      })
      .strict(),
    z.object({ $variant: z.literal("Needs"), needs: z.array(need), later: z.array(need).optional() }).strict(),
  ]);
}

/** Wire names of the result's fields that differ in TypeScript. */
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
    // The engine leaves out an empty list.
    if (out["assumptions"] === undefined) out["assumptions"] = [];
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
  const keys = need["keys"];
  if (Array.isArray(keys)) translated["keys"] = keys.map((key: unknown) => elementKeyFromWire(entry, wireCase, key));
  const waiting = need["waiting"];
  if (Array.isArray(waiting)) translated["waiting"] = waiting.map((rule: unknown) => withParagraphsFromWire(rule, "paragraphs"));
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
export type ScalarIssue = { path: string; message: string; scalar: string; check: string };

export class CalculationError extends Error {
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

  constructor(
    kind: CalculationErrorKind,
    entry: string,
    message: string,
    details: {
      issues?: readonly unknown[];
      diagnostics?: readonly CalculationDiagnostic[];
      scalar?: string | undefined;
      check?: string | undefined;
    } = {},
  ) {
    super(`${entry}: ${message}`);
    this.name = "CalculationError";
    this.kind = kind;
    this.entry = entry;
    this.issues = details.issues ?? [];
    this.diagnostics = details.diagnostics ?? [];
    this.scalar = details.scalar;
    this.check = details.check;
  }
}

function issueSummary(issues: readonly { path: readonly PropertyKey[]; message: string }[]): string {
  const first = issues[0];
  if (first === undefined) return "invalid";
  const at = first.path.length === 0 ? "$" : first.path.map(String).join(".");
  return `${String(issues.length)} issue${issues.length === 1 ? "" : "s"}; first at ${at}: ${first.message}`;
}

/** The one case of an output document, its envelope checked. */
function caseOutput(
  entry: CalculationEntry,
  outputSchema: string,
  document: unknown,
  wireCase: unknown,
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
    const list: CalculationDiagnostic[] = diagnostics.filter(isObject).map((diagnostic) => ({
      caseId: shown(diagnostic["case_id"]),
      path: shown(diagnostic["path"]),
      message: shown(diagnostic["message"]),
      scalar: typeof diagnostic["scalar"] === "string" ? diagnostic["scalar"] : undefined,
      check: typeof diagnostic["check"] === "string" ? diagnostic["check"] : undefined,
    }));
    // A value that fails a check of its named scalar type is an input error,
    // at the TypeScript path, as a validation failure the SDK finds itself.
    const refused = list.filter((diagnostic) => diagnostic.scalar !== undefined && diagnostic.check !== undefined);
    const firstRefused = refused[0];
    if (firstRefused !== undefined && refused.length === list.length) {
      const issues: ScalarIssue[] = refused.map((diagnostic) => ({
        path: fromWirePath(entry.input, wireCase, diagnostic.path.replace(/^\$\.?/, "")),
        message: diagnostic.message,
        scalar: diagnostic.scalar ?? "",
        check: diagnostic.check ?? "",
      }));
      const at = issues[0]?.path ?? "";
      throw new CalculationError(
        "input",
        name,
        `invalid input: ${String(issues.length)} issue${issues.length === 1 ? "" : "s"}; first at ${at === "" ? "$" : at}: ${firstRefused.message}`,
        { issues, diagnostics: list, scalar: firstRefused.scalar, check: firstRefused.check },
      );
    }
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

/**
 * The descriptive text a call returns besides what the caller acts on. By
 * default a need carries its kind, path, pattern, keys, waiting rules and, for
 * an entry need, its entry and subject, and an assumption its section, rule,
 * binding and value without `statement` and `resolve_with`. Name the members
 * to add, as `runa call --describe` does.
 */
export type Describe = {
  readonly needs?: readonly ("label" | "question" | "help" | "unit" | "sources")[] | undefined;
  readonly assumptions?: readonly ("statement" | "resolveWith")[] | undefined;
};

/** How a call, query or answer runs. */
export type CallOptions = {
  /** The descriptive text to return; none by default. */
  readonly describe?: Describe | undefined;
};

/** `describe` as the input document's `describe` list. */
function describeWire(describe: Describe | undefined): string[] {
  return [
    ...(describe?.needs ?? []),
    ...(describe?.assumptions ?? []).map((member) => (member === "resolveWith" ? "resolve_with" : member)),
  ];
}

/** Validate a case and encode it as the input document `runa call` reads. */
function inputDocument<I>(entry: CalculationEntry, inputSchema: z.ZodType<I>, input: I, options: CallOptions = {}) {
  const checked = inputSchema.safeParse(input);
  if (!checked.success) {
    const issues = checked.error.issues;
    throw new CalculationError("input", entry.name, `invalid input: ${issueSummary(issues)}`, { issues });
  }
  const wireCase = toWire(entry.input, checked.data);
  const describe = describeWire(options.describe);
  const document = {
    $futuruna: { schema: INPUT_SCHEMA, schema_hash: entry.schemaHash, entry: entry.name },
    ...(describe.length > 0 ? { describe } : {}),
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
 * `transport.call`, check the envelope, and decode and validate the result;
 * returned with the assumptions it rests on.
 */
export async function callEntry<I, O>(
  transport: EngineTransport,
  entry: CalculationEntry,
  inputSchema: z.ZodType<I>,
  outputSchema: z.ZodType<O>,
  input: I,
  options: CallOptions = {},
): Promise<Calculated<O>> {
  const { wireCase, document } = inputDocument(entry, inputSchema, input, options);
  const output = caseOutput(entry, OUTPUT_SCHEMA, await transport.call(entry.name, document), wireCase);
  return calculated(entry, outputSchema, output);
}

/** The result of a case and its assumptions (the engine leaves out an empty list), decoded and validated. */
function calculated<O>(entry: CalculationEntry, outputSchema: z.ZodType<O>, output: { readonly [key: string]: unknown }): Calculated<O> {
  const result = validated(entry, outputSchema, () => fromWire(entry.output, output["result"]));
  const assumptions = validated(entry, z.array(CalculationAssumptionSchema), () => output["assumptions"] ?? []);
  return { result, assumptions };
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
  options: CallOptions = {},
): Promise<QueryResult<R, N>> {
  const { wireCase, document } = inputDocument(entry, inputSchema, input, options);
  const output = await transport.callPartial(entry.name, document);
  const result = caseOutput(entry, PARTIAL_OUTPUT_SCHEMA, output, wireCase)["result"];
  return validated(entry, resultSchema, () => queryResultFromWire(entry, wireCase, result));
}

// Answering an entry

/** The reply for a need `resolve` cannot fill: it stays open. */
export type Open = { readonly kind: "open" };

/**
 * What `resolve` gives for a need at a path of `P`: `given(need, value)`, a
 * value to write at the need's path, or `open`, when it cannot fill the need.
 * A reply names the path it is for, so it is checked against the need it
 * answers.
 */
export type ResolverReply<V = unknown, P extends string = string> =
  | { readonly kind: "given"; readonly path: InputPath<P>; readonly value: V }
  | Open;

/** The reply for a need `resolve` cannot fill: it stays open. */
export const open: Open = Object.freeze({ kind: "open" });

/** Any value: a type parameter of this constraint keeps a literal such as `"Married"`, a variant's name. */
type AnyValue = string | number | boolean | bigint | symbol | object | null | undefined;

/**
 * The reply that gives `value` for `need`. The value is checked against the
 * type at the need's pattern, so a need of several patterns is narrowed by
 * `need.pattern` first.
 */
export function given<P extends string, V extends AnyValue>(need: { readonly path: InputPath<P> }, value: V): ResolverReply<V, P> {
  return { kind: "given", path: need.path, value };
}

/** What `resolve` is given besides its needs. */
export interface ResolverContext<C> {
  /** The case sent in this round: the partial case and every answer written so far. */
  readonly partialCase: C;
}

/** A partial call of one entry, such as `(c) => taxReportQuery(transport, c)`. */
export type Query<C, R, N extends Need = Need> = (partialCase: C) => Promise<QueryResult<R, N>>;

/** The value types of an entry's input patterns, when not known: any value at any pattern. */
export type AnyPatterns = { readonly [pattern: string]: unknown };

/** A reply to a need of the pattern `P`: its value of the type `Patterns` gives `P`, or `open`. */
export type Reply<Patterns, P extends string> = (P extends keyof Patterns ? ResolverReply<Patterns[P], P> : never) | Open;

/** Answers one need of a round, with a reply typed by the need's own pattern. */
export type AnswerEach<Patterns, N extends Need = Need> = (
  need: N,
) => Reply<Patterns, N["pattern"]> | Promise<Reply<Patterns, N["pattern"]>>;

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
export type Resolve<C, Patterns = AnyPatterns, N extends Need = Need> = (
  needs: readonly N[],
  context: ResolverContext<C>,
) => AnswerEach<Patterns, N> | Promise<AnswerEach<Patterns, N>>;

/** `resolve` as the round loop holds it, whatever the needs and their patterns. */
type LooseResolve<C, N> = (
  needs: readonly N[],
  context: ResolverContext<C>,
) => ((need: N) => ResolverReply | Promise<ResolverReply>) | Promise<(need: N) => ResolverReply | Promise<ResolverReply>>;

/** One round: the paths of its needs, and the paths it answered. */
export type Round<N extends Need = Need> = {
  readonly paths: readonly InputPath<N["pattern"]>[];
  readonly given: readonly InputPath<N["pattern"]>[];
};

/**
 * Where answering an entry ended: `Answered`, or `Unanswered` when a round
 * answered nothing, with that round's needs left open. The partial case holds
 * every answer written.
 */
export type Answering<R, N extends Need = Need, C = unknown> =
  | {
      readonly $variant: "Answered";
      readonly result: R;
      /** The assumptions the result rests on. */
      readonly assumptions: readonly CalculationAssumption[];
      readonly unusedPaths: readonly InputPath<N["pattern"]>[];
      readonly partialCase: C;
      readonly rounds: readonly Round<N>[];
    }
  | {
      readonly $variant: "Unanswered";
      /** The needs of the last round `resolve` left open. */
      readonly open: readonly N[];
      /**
       * Facts read only behind an unknown condition in the last round: what the
       * engine may need once the open needs are answered, in the next round.
       */
      readonly later: readonly N[];
      readonly partialCase: C;
      readonly rounds: readonly Round<N>[];
    };

/** Answering cannot continue. */
export class AnswerError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AnswerError";
  }
}

/**
 * Answer an entry round by round: query it on the partial case, call `resolve`
 * once per round with every need, write the given values with {@link setAt},
 * and query again. Ends `Answered`, or `Unanswered` when a round answers
 * nothing. A reply for another need's path, or a path still needed after it
 * was given, throws {@link AnswerError}. An entry need is answered like any
 * other need. The SDK's `<entry>Answer` calls it.
 */
export async function answerEntry<C, R, N extends Need, Patterns>(
  query: Query<C, R, N>,
  partialCase: C,
  resolve: Resolve<C, Patterns, N>,
): Promise<Answering<R, N, C>> {
  const resolveLoose = resolve as unknown as LooseResolve<C, N>;
  const answered = new Set<string>();
  const rounds: Round<N>[] = [];
  let current = partialCase;
  for (;;) {
    const result = await query(current);
    if (result.$variant === "Complete") {
      return {
        $variant: "Answered",
        result: result.result,
        assumptions: result.assumptions,
        unusedPaths: result.unusedPaths,
        partialCase: current,
        rounds,
      };
    }
    for (const need of result.needs) {
      if (answered.has(need.path)) {
        throw new AnswerError(`the engine still needs \`${need.path}\` after it was given`);
      }
    }
    const context: ResolverContext<C> = { partialCase: current };
    const answerEach = await resolveLoose(result.needs, context);
    const replies = await Promise.all(
      result.needs.map(async (need) => {
        const reply = await answerEach(need);
        if (reply.kind !== "open" && reply.path !== need.path) {
          throw new AnswerError(`\`resolve\` replied to \`${need.path}\` with a reply for \`${reply.path}\``);
        }
        return reply;
      }),
    );
    const left: N[] = [];
    const givenPaths: InputPath<N["pattern"]>[] = [];
    let next = current;
    result.needs.forEach((need, at) => {
      const reply = replies[at];
      if (reply === undefined || reply.kind === "open") {
        left.push(need);
        return;
      }
      next = setAt(next, need.path, reply.value);
      answered.add(need.path);
      givenPaths.push(need.path);
    });
    rounds.push({ paths: result.needs.map((need) => need.path), given: givenPaths });
    if (givenPaths.length === 0) {
      return { $variant: "Unanswered", open: left, later: result.later ?? [], partialCase: current, rounds };
    }
    current = next;
  }
}
