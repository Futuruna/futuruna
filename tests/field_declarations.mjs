// Reads `# Type(...)` record declarations, single-line or multi-line, with
// `@ field(...)` annotations on their fields. Strings and nesting are honoured.

function scanTo(text, start, stop) {
  let depth = 0;
  for (let i = start; i < text.length; i += 1) {
    const c = text[i];
    if (c === '"') {
      for (i += 1; text[i] !== '"'; i += text[i] === '\\' ? 2 : 1);
    } else if ('([{'.includes(c)) depth += 1;
    else if (')]}'.includes(c)) {
      if (depth === 0) return i;
      depth -= 1;
    } else if (depth === 0 && stop.includes(c)) return i;
  }
  return -1;
}

// The members of a record declaration: [{ name, type, field }], where `field`
// is the text of its `@ field(...)` annotation or null.
export function declaredFields(source, type) {
  const head = source.match(new RegExp(`^# ${type}\\(`, 'm'));
  if (!head) return null;
  const open = head.index + head[0].length;
  const close = scanTo(source, open, '');
  const members = [];
  for (let i = open; i < close;) {
    const end = scanTo(source, i, ',');
    let part = source.slice(i, end < 0 || end > close ? close : end).trim();
    let field = null;
    const annotation = part.match(/^@ field\(/);
    if (annotation) {
      const stop = scanTo(part, annotation[0].length, '');
      field = part.slice(0, stop + 1);
      part = part.slice(stop + 1).trim();
    }
    const colon = part.indexOf(':');
    members.push({ name: part.slice(0, colon).trim(), type: part.slice(colon + 1).trim(), field });
    i = end < 0 || end > close ? close : end + 1;
  }
  return members;
}

export function declaredField(source, type, name) {
  return declaredFields(source, type)?.find(member => member.name === name)?.field ?? null;
}
