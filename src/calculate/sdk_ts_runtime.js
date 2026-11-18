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
export const SetSchema = (item) => z.array(item).refine((values) => new Set(values.map(canonicalJson)).size === values.length, "expected a set without duplicate values");
function canonicalJson(value) {
    if (value === undefined)
        return "undefined";
    if (Array.isArray(value))
        return `[${value.map(canonicalJson).join(",")}]`;
    if (typeof value === "object" && value !== null) {
        const entries = Object.entries(value).sort(([left], [right]) => byText(left, right));
        return `{${entries.map(([key, item]) => `${JSON.stringify(key)}:${canonicalJson(item)}`).join(",")}}`;
    }
    return JSON.stringify(value);
}
/** Any value as text for a message: a string as it is, anything else as JSON. */
function shown(value) {
    return typeof value === "string" ? value : canonicalJson(value);
}
function byText(left, right) {
    return left < right ? -1 : left > right ? 1 : 0;
}
const ISO_DATE = /^(\d{4})-(\d{2})-(\d{2})$/;
function isoDateParts(text) {
    const match = ISO_DATE.exec(text);
    if (match === null)
        return undefined;
    const [year, month, day] = [Number(match[1]), Number(match[2]), Number(match[3])];
    const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
    const days = month === 2 ? (leap ? 29 : 28) : [4, 6, 9, 11].includes(month) ? 30 : 31;
    return month >= 1 && month <= 12 && day >= 1 && day <= days ? { year, month, day } : undefined;
}
function isIsoDate(value) {
    return typeof value === "string" && isoDateParts(value) !== undefined;
}
/** `Date`: an ISO date `YYYY-MM-DD` that is on the calendar (`2026-02-30` is rejected). */
export const IsoDateSchema = z.custom(isIsoDate, "expected an ISO date YYYY-MM-DD on the calendar");
/** `text` as an {@link IsoDate}; throws a `RangeError` unless it is a date `YYYY-MM-DD` on the calendar. */
export function isoDate(text) {
    if (!isIsoDate(text))
        throw new RangeError(`expected an ISO date YYYY-MM-DD on the calendar, got ${JSON.stringify(text)}`);
    return text;
}
/** Validates {@link Unknown}: exactly `{}`. */
export const UnknownSchema = z.object({}).strict();
/** The {@link None} answer. */
export const none = Object.freeze({ $none: true });
/** Validates {@link None}: exactly `{ "$none": true }`. */
export const NoneSchema = z.object({ $none: z.literal(true) }).strict();
function isNone(value) {
    return isObject(value) && value["$none"] === true && Object.keys(value).length === 1;
}
/** A value written as it is: a number, string, boolean or `null`, or an array or object of them. */
export const valueCodec = { kind: "value" };
/** A `Date`: `"2026-03-01"` is `{ "year": 2026, "month": 3, "day": 1 }` on the wire. */
export const isoDateCodec = { kind: "date" };
/** A sum type whose variants carry no fields: `"Deposit"` is `{ "$variant": "Deposit" }` on the wire. */
export const enumCodec = { kind: "enum" };
/** An optional value whose present values use `item`: {@link none} and `null` are `null` on the wire. */
export const optionalCodec = (item) => ({ kind: "optional", item });
/** A list or set whose items use `item`. */
export const listCodec = (item) => ({ kind: "list", item });
/** A `Map(String, T)` whose values use `item`; keys are the same on both sides. */
export const mapCodec = (item) => ({ kind: "map", item });
/** A record: each TypeScript field name with its wire name and codec. */
export const recordCodec = (fields) => ({ kind: "record", fields });
/** A sum type tagged by `$variant`: the fields of each variant, or its positional `$values`. */
export const unionCodec = (variants) => ({
    kind: "union",
    variants,
});
function codecOf(ref) {
    return typeof ref === "function" ? ref() : ref;
}
/** The codec of the values a path walks through: an optional value's is its item's. */
function presentCodec(ref) {
    const codec = codecOf(ref);
    return codec.kind === "optional" ? presentCodec(codec.item) : codec;
}
function isObject(value) {
    return typeof value === "object" && value !== null && !Array.isArray(value);
}
function hasOwn(object, key) {
    return Object.prototype.hasOwnProperty.call(object, key);
}
function isPositional(spec) {
    return hasOwn(spec, "$values");
}
/** Set an own property, also for a key such as `__proto__`. */
function put(target, key, value) {
    Object.defineProperty(target, key, { value, enumerable: true, writable: true, configurable: true });
}
const wireNames = new WeakMap();
/** The fields by wire name: wire name → TypeScript name and codec. */
function byWireName(fields) {
    let names = wireNames.get(fields);
    if (names === undefined) {
        names = new Map(Object.entries(fields).map(([name, [wireName, codec]]) => [wireName, [name, codec]]));
        wireNames.set(fields, names);
    }
    return names;
}
/** A value with a field its codec does not know: the contract has no such field there. */
export class WireError extends Error {
    issues;
    constructor(issues) {
        super(issueSummary(issues));
        this.name = "WireError";
        this.issues = issues;
    }
}
/**
 * Encode a TypeScript value, complete or partial, as `runa call` JSON. A field
 * the codec does not know throws {@link WireError}.
 */
export function toWire(codec, value) {
    return converted(codec, value, true);
}
/**
 * Decode `runa call` JSON into its TypeScript value. A field the codec does
 * not know throws {@link WireError}; any other mismatch (a string where a
 * number belongs, an unknown variant) stays as it is, for the validators to
 * reject.
 */
export function fromWire(codec, value) {
    return converted(codec, value, false);
}
function converted(codec, value, encode) {
    const conversion = { encode, path: [], issues: [] };
    const out = convert(codec, value, conversion);
    if (conversion.issues.length > 0)
        throw new WireError(conversion.issues);
    return out;
}
function convert(ref, value, conversion) {
    if (value === undefined || value === null)
        return value;
    const codec = codecOf(ref);
    const { encode, path } = conversion;
    switch (codec.kind) {
        case "value":
            return value;
        case "date":
            return encode ? ((typeof value === "string" && isoDateParts(value)) || value) : dateFromWire(value);
        case "enum":
            if (encode)
                return typeof value === "string" ? { $variant: value } : value;
            return isObject(value) && typeof value["$variant"] === "string" && Object.keys(value).length === 1
                ? value["$variant"]
                : value;
        case "optional":
            return encode && isNone(value) ? null : convert(codec.item, value, conversion);
        case "list": {
            if (!Array.isArray(value))
                return value;
            const out = [];
            for (const [index, item] of value.entries()) {
                path.push(index);
                out.push(convert(codec.item, item, conversion));
                path.pop();
            }
            return out;
        }
        case "map": {
            if (!isObject(value))
                return value;
            const out = {};
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
            if (!isObject(value))
                return value;
            const variant = value["$variant"];
            const spec = typeof variant === "string" && hasOwn(codec.variants, variant) ? codec.variants[variant] : undefined;
            if (spec === undefined)
                return value;
            if (!isPositional(spec))
                return convertFields(spec, value, true, conversion);
            const values = value["$values"];
            if (!Array.isArray(values))
                return value;
            const out = { ...value };
            path.push("$values");
            out["$values"] = values.map((item, index) => {
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
function convertFields(fields, value, tagged, conversion) {
    const { encode, path, issues } = conversion;
    const names = encode ? undefined : byWireName(fields);
    const out = {};
    for (const [key, item] of Object.entries(value)) {
        if (item === undefined)
            continue;
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
function dateFromWire(value) {
    if (!isObject(value) || Object.keys(value).length !== 3)
        return value;
    const { year, month, day } = value;
    if (!(Number.isSafeInteger(year) && Number.isSafeInteger(month) && Number.isSafeInteger(day)))
        return value;
    const pad = (part, width) => String(part).padStart(width, "0");
    const [y, m, d] = [Number(year), Number(month), Number(day)];
    return `${y >= 0 ? pad(y, 4) : String(y)}-${pad(m, 2)}-${pad(d, 2)}`;
}
const DATE_PARTS = { year: true, month: true, day: true };
/**
 * The TypeScript path of an input path the engine reports:
 * `events[0].amount_ore` → `events[0].amountOre`; an enum's tag is the enum
 * (`events[0].direction.$variant` → `events[0].direction`); a part of a `Date`
 * is the date (`events[0].date.year` → `events[0].date`). `wireCase` is the
 * encoded case, which names the variant of each sum type on the way. The part
 * of a path the codec does not describe stays as it is.
 */
export function fromWirePath(codec, wireCase, path) {
    return wirePath(codec, wireCase, path).path;
}
function child(value, key) {
    if (typeof key === "number")
        return Array.isArray(value) ? value[key] : undefined;
    return isObject(value) ? value[key] : undefined;
}
function wirePath(codec, wireCase, path) {
    let segments;
    try {
        segments = parsePath(path);
    }
    catch {
        return { path };
    }
    const out = [];
    let at = { node: presentCodec(codec), value: wireCase };
    for (const segment of segments) {
        const { node, value } = at;
        if ("key" in segment) {
            // An entry of an open collection by its key: the key field at its TypeScript name.
            const item = node !== undefined && !("$values" in node) && node.kind === "list" ? presentCodec(node.item) : undefined;
            const named = item?.kind === "record" ? byWireName(item.fields).get(segment.key) : undefined;
            out.push({ key: named?.[0] ?? segment.key, value: segment.value });
            const entry = Array.isArray(value)
                ? value.find((candidate) => isObject(candidate) && candidate[segment.key] === segment.value)
                : undefined;
            at = { node: item, value: entry };
            continue;
        }
        const key = "index" in segment ? segment.index : segment.field;
        const step = (next, name = segment) => {
            out.push(name);
            return { node: next === undefined ? undefined : presentCodec(next), value: child(value, key) };
        };
        if (node === undefined) {
            at = step(undefined);
        }
        else if ("$values" in node) {
            at = step("index" in segment ? node.$values[segment.index] : undefined);
        }
        else if ("index" in segment) {
            at = step(node.kind === "list" ? node.item : undefined);
        }
        else {
            const field = segment.field;
            switch (node.kind) {
                case "date":
                    if (hasOwn(DATE_PARTS, field))
                        return { path: formatPath(out), part: field };
                    at = step(undefined);
                    break;
                case "enum":
                    if (field === "$variant")
                        return { path: formatPath(out) };
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
                        .filter((candidate) => !isPositional(candidate))
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
function formatPath(segments) {
    return segments
        .map((segment, at) => "key" in segment
        ? `[${segment.key}=${JSON.stringify(segment.value)}]`
        : "index" in segment
            ? `[${String(segment.index)}]`
            : at === 0
                ? segment.field
                : `.${segment.field}`)
        .join("");
}
/** A path that does not parse, or that {@link setAt} cannot write. */
export class PathError extends Error {
    path;
    constructor(path, message) {
        super(`${path}: ${message}`);
        this.name = "PathError";
        this.path = path;
    }
}
const FIRST_FIELD = /^[^.[\]]+/;
const NEXT_FIELD = /^\.([^.[\]]+)/;
const LIST_INDEX = /^\[(\d+)\]/;
const KEY_VALUE = String.raw `"(?:[^"\\]|\\.)*"|-?\d+`;
const ENTRY_KEY = new RegExp(String.raw `^\[([^.[\]=]+)=(${KEY_VALUE})\]`);
/** A list index or an entry's key selector, as `[]` in a pattern stands for either. */
const ELEMENT = String.raw `\[(?:\d+|[^.[\]=]+=(?:${KEY_VALUE}))\]`;
/** Split a concrete input path such as `accounts[2].closingCashOre` into its steps. */
export function parsePath(path) {
    const first = FIRST_FIELD.exec(path);
    if (first === null)
        throw new PathError(path, "a path starts with a field name");
    const segments = [{ field: first[0] }];
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
            segments.push({ key: entry[1], value: JSON.parse(entry[2]) });
            rest = rest.slice(entry[0].length);
            continue;
        }
        const field = NEXT_FIELD.exec(rest);
        if (field?.[1] === undefined)
            throw new PathError(path, `unexpected \`${rest}\``);
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
export function setAt(partialCase, path, value) {
    return writeAt(partialCase, parsePath(path), 0, value, path);
}
function writeAt(node, segments, at, value, path) {
    const segment = segments[at];
    if (segment === undefined)
        return value;
    if ("key" in segment) {
        const list = node ?? [];
        if (!Array.isArray(list))
            throw new PathError(path, `the entry \`${segment.key}\` = ${JSON.stringify(segment.value)} is inside a value that is not a list`);
        const copy = list.slice();
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
        const copy = node.slice();
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
    const out = { ...record };
    put(out, segment.field, writeAt(record[segment.field], segments, at + 1, value, path));
    return out;
}
/** A new entry of an open collection, with the key its path names. */
function keyedValue(value, segment, path) {
    if (!isObject(value))
        throw new PathError(path, "an entry of an open collection is a record");
    const given = value[segment.key];
    if (given === undefined)
        return { ...value, [segment.key]: segment.value };
    if (given !== segment.value) {
        throw new PathError(path, `the entry's \`${segment.key}\` is ${JSON.stringify(given)}, and the need asks for ${JSON.stringify(segment.value)}`);
    }
    return value;
}
/**
 * The pattern of a concrete path: list indices and entry keys become `[]`
 * (`accounts[2].kind` → `accounts[].kind`, `balances[day="2027-02-20"]` → `balances[]`).
 */
export function pathPattern(path) {
    return path.replace(new RegExp(ELEMENT, "g"), "[]");
}
const patternExpressions = new Map();
/**
 * Whether a concrete path matches a pattern, where `[]` stands for any list
 * index or entry key and `*` for any map key.
 */
export function matchesPathPattern(path, pattern) {
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
function escapeRegExp(text) {
    return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
/**
 * A validator for the concrete input paths matching one of `patterns`, typed by
 * them; `undefined` accepts any path (a recursive input type has paths of any
 * depth).
 */
export function inputPathSchema(patterns) {
    return z.custom((value) => typeof value === "string" &&
        (patterns === undefined ? parsesAsPath(value) : patterns.some((pattern) => matchesPathPattern(value, pattern))), "not an input path of this calculation");
}
function parsesAsPath(path) {
    try {
        parsePath(path);
        return true;
    }
    catch {
        return false;
    }
}
/** The validator of a unit of measure: a safe integer, typed `U` once parsed. */
export function unitSchema(unit) {
    return z.custom((value) => Number.isSafeInteger(value), `expected a safe integer (${unit})`);
}
/** `value` as a `U` of the unit `unit`; throws a `RangeError` unless it is a safe integer. */
export function unitValue(unit, value) {
    if (!Number.isSafeInteger(value))
        throw new RangeError(`${unit}: expected a safe integer, got ${String(value)}`);
    return value;
}
/** The decimal places of `1 / per`, when `per` is built from 2s and 5s only; otherwise `undefined`. */
function scaleDigits(per) {
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
function amountOf(scale, amount) {
    return scale.of === "1" ? amount : `${amount} ${scale.of}`;
}
/**
 * `amount`, a decimal number of the unit's base as text (`"1740.25"` DKK), as
 * a `U`: exact, in integer arithmetic, never through a float. An amount finer
 * than one `U`, text that is not a decimal number, and a result beyond a safe
 * integer throw a {@link CalculationError} of kind `amount` naming the unit
 * and the amount. The SDK's converters (`øre.fromDecimal`) call it.
 */
export function unitFromDecimal(unit, scale, amount) {
    const match = /^([+-]?)(\d+)(?:\.(\d+))?$/.exec(amount);
    if (match === null) {
        throw new CalculationError("amount", unit, `\`${amount}\` is not a decimal number${scale.of === "1" ? "" : ` of ${scale.of}`}`);
    }
    const [, sign = "", whole = "", fraction = ""] = match;
    const scaled = BigInt(whole + fraction) * BigInt(scale.per);
    const divisor = 10n ** BigInt(fraction.length);
    if (scaled % divisor !== 0n) {
        throw new CalculationError("amount", unit, `${amountOf(scale, amount)} is finer than one ${unit} (1/${String(scale.per)}${scale.of === "1" ? "" : ` ${scale.of}`})`);
    }
    const magnitude = scaled / divisor;
    if (magnitude > BigInt(Number.MAX_SAFE_INTEGER)) {
        throw new CalculationError("amount", unit, `${amountOf(scale, amount)} is beyond a safe integer of ${unit}`);
    }
    const value = Number(magnitude);
    return (sign === "-" && value !== 0 ? -value : value);
}
/**
 * `value` as a decimal number of the unit's base, exact, with as many
 * decimals as one `U` has (`"1740.25"` for 174025 øre, `"1740.00"` for
 * 174000). Throws a `RangeError` unless `value` is a safe integer, or when the
 * scale is not built from 2s and 5s (a unit the SDK gives no converters).
 */
export function unitToDecimal(unit, scale, value) {
    if (!Number.isSafeInteger(value))
        throw new RangeError(`${unit}: expected a safe integer, got ${String(value)}`);
    const digits = scaleDigits(scale.per);
    if (digits === undefined)
        throw new RangeError(`${unit}: 1/${String(scale.per)} has no finite decimal form`);
    const scaled = BigInt(value) * (10n ** BigInt(digits) / BigInt(scale.per));
    const negative = scaled < 0n;
    const text = (negative ? -scaled : scaled).toString().padStart(digits + 1, "0");
    const whole = text.slice(0, text.length - digits);
    const fraction = digits > 0 ? `.${text.slice(text.length - digits)}` : "";
    return `${negative ? "-" : ""}${whole}${fraction}`;
}
function isScalarPrimitive(of, value) {
    return of === "Int" ? Number.isSafeInteger(value) : typeof value === "string";
}
/**
 * The validator of a named scalar type: a string (or a safe integer), typed
 * `S` once parsed. The engine runs the type's checks.
 */
export function scalarSchema(scalar, of) {
    return z.custom((value) => isScalarPrimitive(of, value), `expected ${of === "Int" ? "a safe integer" : "a string"} (${scalar})`);
}
/** `value` as an `S`; throws a `RangeError` unless it is a string (or a safe integer). */
export function scalarValue(scalar, of, value) {
    if (!isScalarPrimitive(of, value)) {
        throw new RangeError(`${scalar}: expected ${of === "Int" ? "a safe integer" : "a string"}, got ${String(value)}`);
    }
    return value;
}
export const SourceReferenceSchema = z
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
const ActSchema = z
    .object({ title: z.string(), identifier: z.string().optional(), url: z.string().optional() })
    .strict();
const AmendmentSchema = z
    .object({ act: z.string(), paragraph: z.string(), instruction: z.string() })
    .strict();
const ParagraphReferenceSchema = z
    .object({ paragraph: z.string(), source: z.string().optional(), act: ActSchema.optional(), asAmendedBy: AmendmentSchema.optional() })
    .strict();
const PARAGRAPH_NAMES = { as_amended_by: "asAmendedBy" };
/** The list at `key` of `object`, each cited paragraph with TypeScript names (`as_amended_by` is `asAmendedBy`). */
function withParagraphsFromWire(object, key) {
    if (!isObject(object))
        return object;
    const list = object[key];
    if (!Array.isArray(list))
        return object;
    return { ...object, [key]: list.map((paragraph) => (isObject(paragraph) ? renamed(paragraph, PARAGRAPH_NAMES) : paragraph)) };
}
const WaitingRuleSchema = z
    .object({ rule: z.string(), section: z.string().optional(), paragraphs: z.array(ParagraphReferenceSchema).optional() })
    .strict();
/**
 * The validator of an entry's needs: the pattern is one of `patterns` (any
 * pattern, when `undefined`) and the path matches it, and each element key is
 * the declared key of a list the path lies in.
 */
export function needSchema(patterns, keys) {
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
            const declared = keys.find((candidate) => need.pattern.startsWith(candidate.element) &&
                matchesPathPattern(key.path, candidate.element) &&
                candidate.field === key.field &&
                candidate.value.safeParse(key.value).success);
            if (declared === undefined) {
                context.addIssue({ code: "custom", path: ["keys", at], message: "not the declared key of a list the path lies in" });
            }
        }
    });
    // The refinement checks what the type says: `pattern` is one of `P` and `path` one of its paths.
    return schema;
}
export const CalculationAssumptionSchema = z
    .object({
    section: z.string().optional(),
    rule: z.string().optional(),
    binding: z.string().optional(),
    value: z.unknown(),
})
    .strict();
export function queryResultSchema(result, need, path) {
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
const RESULT_NAMES = { unused_paths: "unusedPaths" };
function renamed(value, names) {
    const out = {};
    for (const [key, item] of Object.entries(value))
        put(out, names[key] ?? key, item);
    return out;
}
/** The TypeScript form of a partial call's result, its paths at TypeScript paths of the case. */
function queryResultFromWire(entry, wireCase, result) {
    if (!isObject(result))
        return result;
    const out = renamed(result, RESULT_NAMES);
    if (result["$variant"] === "Complete") {
        if (hasOwn(out, "result"))
            out["result"] = fromWire(entry.output, out["result"]);
        // The engine leaves out an empty list.
        if (out["assumptions"] === undefined)
            out["assumptions"] = [];
        const unused = out["unusedPaths"];
        if (Array.isArray(unused)) {
            const paths = unused.map((item) => (typeof item === "string" ? fromWirePath(entry.input, wireCase, item) : item));
            // The parts of a date are one path in TypeScript.
            out["unusedPaths"] = [...new Set(paths)].sort((a, b) => byText(shown(a), shown(b)));
        }
    }
    else if (result["$variant"] === "Needs") {
        for (const list of ["needs", "later"]) {
            const needs = out[list];
            if (Array.isArray(needs))
                out[list] = needs.map((need) => needFromWire(entry, wireCase, need));
        }
    }
    return out;
}
/** A need at TypeScript paths of the case, tagged with its pattern. */
function needFromWire(entry, wireCase, need) {
    if (!isObject(need))
        return need;
    const translated = { ...need };
    const at = need["path"];
    if (typeof at === "string") {
        const path = fromWirePath(entry.input, wireCase, at);
        translated["path"] = path;
        // The pattern the path matches; a path of a recursive type has its list indices as `[]`.
        translated["pattern"] = entry.patterns.find((pattern) => matchesPathPattern(path, pattern)) ?? pathPattern(path);
    }
    const subject = need["subject"];
    if (typeof subject === "string")
        translated["subject"] = fromWirePath(entry.input, wireCase, subject);
    const keys = need["keys"];
    if (Array.isArray(keys))
        translated["keys"] = keys.map((key) => elementKeyFromWire(entry, wireCase, key));
    const waiting = need["waiting"];
    if (Array.isArray(waiting))
        translated["waiting"] = waiting.map((rule) => withParagraphsFromWire(rule, "paragraphs"));
    return translated;
}
/** An element key at the element's TypeScript path, with its field's TypeScript name and value. */
function elementKeyFromWire(entry, wireCase, key) {
    if (!isObject(key) || typeof key["path"] !== "string" || typeof key["field"] !== "string")
        return key;
    const located = wirePath(entry.input, wireCase, key["path"]);
    const field = located.codec?.kind === "record" ? byWireName(located.codec.fields).get(key["field"]) : undefined;
    if (field === undefined)
        return { ...key, path: located.path };
    return { path: located.path, field: field[0], value: fromWire(field[1], key["value"]) };
}
// Calls through an engine transport
const CASE_ID = "case-1";
export class CalculationError extends Error {
    kind;
    /** The entry called; for `amount`, the unit converting the amount. */
    entry;
    /** The validation issues, for `input` and `output`. */
    issues;
    /** The engine's diagnostics, for `diagnostics`. */
    diagnostics;
    /** For `input` the engine found: the named scalar type of the first refused value. */
    scalar;
    /** For `input` the engine found: the check of `scalar` that is false. */
    check;
    constructor(kind, entry, message, details = {}) {
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
function issueSummary(issues) {
    const first = issues[0];
    if (first === undefined)
        return "invalid";
    const at = first.path.length === 0 ? "$" : first.path.map(String).join(".");
    return `${String(issues.length)} issue${issues.length === 1 ? "" : "s"}; first at ${at}: ${first.message}`;
}
/** The one case of an output document, its envelope checked. */
function caseOutput(entry, outputSchema, document, wireCase) {
    const parsed = typeof document === "string" ? JSON.parse(document) : document;
    if (!isObject(parsed) || !("$futuruna" in parsed))
        return { result: parsed };
    const envelope = parsed["$futuruna"];
    const name = entry.name;
    if (!isObject(envelope))
        throw new CalculationError("envelope", name, "`$futuruna` is not an object");
    if (envelope["schema_hash"] !== undefined && envelope["schema_hash"] !== entry.schemaHash) {
        throw new CalculationError("envelope", name, `the engine runs contract ${shown(envelope["schema_hash"])}, but this SDK was generated for ${entry.schemaHash}`);
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
            scalar: typeof diagnostic["scalar"] === "string" ? diagnostic["scalar"] : undefined,
            check: typeof diagnostic["check"] === "string" ? diagnostic["check"] : undefined,
        }));
        // A value that fails a check of its named scalar type is an input error,
        // at the TypeScript path, as a validation failure the SDK finds itself.
        const refused = list.filter((diagnostic) => diagnostic.scalar !== undefined && diagnostic.check !== undefined);
        const firstRefused = refused[0];
        if (firstRefused !== undefined && refused.length === list.length) {
            const issues = refused.map((diagnostic) => ({
                path: fromWirePath(entry.input, wireCase, diagnostic.path.replace(/^\$\.?/, "")),
                message: diagnostic.message,
                scalar: diagnostic.scalar ?? "",
                check: diagnostic.check ?? "",
            }));
            const at = issues[0]?.path ?? "";
            throw new CalculationError("input", name, `invalid input: ${String(issues.length)} issue${issues.length === 1 ? "" : "s"}; first at ${at === "" ? "$" : at}: ${firstRefused.message}`, { issues, diagnostics: list, scalar: firstRefused.scalar, check: firstRefused.check });
        }
        const first = list[0];
        const summary = first === undefined ? "" : `; first at ${first.path}: ${first.message}`;
        throw new CalculationError("diagnostics", name, `${String(diagnostics.length)} diagnostic(s)${summary}`, {
            diagnostics: list,
        });
    }
    const results = parsed["results"];
    const only = Array.isArray(results) && results.length === 1 ? results[0] : undefined;
    if (!isObject(only) || !("result" in only)) {
        throw new CalculationError("envelope", name, "the output document does not hold exactly one case result");
    }
    return only;
}
/** Validate a case and encode it as the input document `runa call` reads. */
function inputDocument(entry, inputSchema, input) {
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
function validated(entry, schema, decode) {
    let value;
    try {
        value = decode();
    }
    catch (error) {
        if (!(error instanceof WireError))
            throw error;
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
export async function callEntry(transport, entry, inputSchema, outputSchema, input) {
    const { wireCase, document } = inputDocument(entry, inputSchema, input);
    const output = caseOutput(entry, OUTPUT_SCHEMA, await transport.call(entry.name, document), wireCase);
    return calculated(entry, outputSchema, output);
}
/** The result of a case and its assumptions (the engine leaves out an empty list), decoded and validated. */
function calculated(entry, outputSchema, output) {
    const result = validated(entry, outputSchema, () => fromWire(entry.output, output["result"]));
    const assumptions = validated(entry, z.array(CalculationAssumptionSchema), () => output["assumptions"] ?? []);
    return { result, assumptions };
}
/**
 * Query `entry` on a partial case through `transport.callPartial`: `Complete`
 * or `Needs`, with the result decoded, every path a TypeScript path and every
 * need tagged with its pattern.
 */
export async function queryEntry(transport, entry, inputSchema, resultSchema, input) {
    const { wireCase, document } = inputDocument(entry, inputSchema, input);
    const output = await transport.callPartial(entry.name, document);
    const result = caseOutput(entry, PARTIAL_OUTPUT_SCHEMA, output, wireCase)["result"];
    return validated(entry, resultSchema, () => queryResultFromWire(entry, wireCase, result));
}
/** The reply for a need `resolve` cannot fill: it stays open. */
export const open = Object.freeze({ kind: "open" });
/**
 * The reply that gives `value` for `need`. The value is checked against the
 * type at the need's pattern, so a need of several patterns is narrowed by
 * `need.pattern` first.
 */
export function given(need, value) {
    return { kind: "given", path: need.path, value };
}
/** Answering cannot continue. */
export class AnswerError extends Error {
    constructor(message) {
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
export async function answerEntry(query, partialCase, resolve) {
    const resolveLoose = resolve;
    const answered = new Set();
    const rounds = [];
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
        const context = { partialCase: current };
        const answerEach = await resolveLoose(result.needs, context);
        const replies = await Promise.all(result.needs.map(async (need) => {
            const reply = await answerEach(need);
            if (reply.kind !== "open" && reply.path !== need.path) {
                throw new AnswerError(`\`resolve\` replied to \`${need.path}\` with a reply for \`${reply.path}\``);
            }
            return reply;
        }));
        const left = [];
        const givenPaths = [];
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
