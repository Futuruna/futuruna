const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { before, test } = require('node:test');
const { Registry, parseRawGrammar, INITIAL } = require('vscode-textmate');
const { loadWASM, OnigScanner, OnigString } = require('vscode-oniguruma');

let grammar;
before(async () => {
  const wasm = fs.readFileSync(require.resolve('vscode-oniguruma/release/onig.wasm'));
  await loadWASM(wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength));
  const grammarPath = path.join(__dirname, '../syntaxes/runa.tmLanguage.json');
  const registry = new Registry({
    onigLib: Promise.resolve({
      createOnigScanner: (patterns) => new OnigScanner(patterns),
      createOnigString: (text) => new OnigString(text),
    }),
    loadGrammar: async (scope) => scope === 'source.runa'
      ? parseRawGrammar(fs.readFileSync(grammarPath, 'utf8'), grammarPath)
      : null,
  });
  grammar = await registry.loadGrammar('source.runa');
});

function tokenize(source) {
  let state = INITIAL;
  return source.split('\n').map((line) => {
    const result = grammar.tokenizeLine(line, state);
    state = result.ruleStack;
    return { line, tokens: result.tokens };
  });
}

function scopes(rows, row, fragment) {
  const { line, tokens } = rows[row];
  const offset = line.indexOf(fragment);
  assert.notEqual(offset, -1, `missing fixture text ${fragment}`);
  const token = tokens.find((token) => token.startIndex <= offset && token.endIndex > offset);
  assert.ok(token, `no token at ${row}:${offset}`);
  return token.scopes;
}

function has(rows, row, fragment, scope) {
  const actual = scopes(rows, row, fragment);
  assert.ok(actual.includes(scope), `${fragment}: expected ${scope}; got ${actual.join(', ')}`);
}

test('quoted law blocks stay comments until the closing fence', () => {
  const rows = tokenize('----\nLaw says "tax" > 0 and True\n----\n= after = 1');
  has(rows, 1, 'Law', 'comment.block.runa');
  has(rows, 1, 'True', 'comment.block.runa');
  has(rows, 3, 'after', 'variable.other.binding.runa');
  assert.ok(!scopes(rows, 3, 'after').some((scope) => scope.startsWith('comment.')));
  const inline = tokenize('---- quoted ---- = after = 1');
  has(inline, 0, 'quoted', 'comment.block.runa');
  has(inline, 0, 'after', 'variable.other.binding.runa');
  const banner = tokenize('-----\n-- Tax law\n---------\n= after = 1');
  has(banner, 1, 'Tax', 'comment.block.runa');
  has(banner, 3, 'after', 'variable.other.binding.runa');
  assert.ok(!scopes(banner, 3, 'after').some((scope) => scope.startsWith('comment.')));
});

test('character literals cannot start a runaway double-quoted string', () => {
  const rows = tokenize("= quote = '\"'\n= newline = '\\n'\n= letter = 'ø'\n= after = True");
  for (const [line, text] of [[0, "'\"'"], [1, "'\\n'"], [2, "'ø'"]]) {
    has(rows, line, text, 'constant.character.runa');
  }
  has(rows, 3, 'after', 'variable.other.binding.runa');
});

test('triple strings retain multiline prose and tokenize interpolation', () => {
  const rows = tokenize('= text = """Quote "tax"\nResult {{ if True { 4 } else { 2 } }}\nend"""\n= after = 1');
  has(rows, 0, 'tax', 'string.quoted.triple.runa');
  has(rows, 1, 'Result', 'string.quoted.triple.runa');
  has(rows, 1, 'if', 'keyword.control.runa');
  has(rows, 1, 'True', 'constant.language.boolean.runa');
  has(rows, 1, 'else', 'meta.embedded.inline.runa');
  has(rows, 2, 'end', 'string.quoted.triple.runa');
  has(rows, 3, 'after', 'variable.other.binding.runa');
  const quoted = tokenize('"""{{ "}}" }} prose"""');
  has(quoted, 0, 'prose', 'string.quoted.triple.runa');
  assert.ok(!scopes(quoted, 0, 'prose').includes('meta.embedded.inline.runa'));
});

test('rule syntax and verification runes have their own scopes', () => {
  const rows = tokenize('| fee(p) -> 10 under p.income > 0\n| exception fee(p) -> 0\n? lawful\n= result = value?');
  has(rows, 0, '|', 'keyword.operator.rule.runa');
  has(rows, 0, 'fee', 'entity.name.function.rule.runa');
  has(rows, 0, 'under', 'keyword.control.runa');
  has(rows, 1, 'exception', 'keyword.control.runa');
  has(rows, 2, '?', 'keyword.operator.verification.runa');
  has(rows, 3, '?', 'keyword.operator.query.runa');
});

test('law-model keywords and both accepted Boolean spellings are recognized', () => {
  const rows = tokenize('under exception scope not and\nTrue False true false');
  for (const word of ['under', 'exception', 'scope', 'not', 'and']) {
    has(rows, 0, word, 'keyword.control.runa');
  }
  for (const word of ['True', 'False', 'true', 'false']) {
    has(rows, 1, word, 'constant.language.boolean.runa');
  }
});

test('comparisons remain operators and declarations keep their rune', () => {
  const rows = tokenize('x < y\nx > f(y)\nx <= y\nx >= y\n> f(x: Int) -> Int { x }');
  for (const [line, text] of [[0, '<'], [1, '>'], [2, '<='], [3, '>=']]) {
    has(rows, line, text, 'keyword.operator.comparison.runa');
  }
  has(rows, 4, '>', 'keyword.operator.definition.runa');
});

test('editor block-comment commands use the language delimiter', () => {
  const config = JSON.parse(fs.readFileSync(path.join(__dirname, '../language-configuration.json'), 'utf8'));
  assert.deepEqual(config.comments.blockComment, ['----', '----']);
});

test('existing match arms, pipes and line comments retain their scopes', () => {
  const rows = tokenize('| _ -> 0\n| Some(x) -> x\nitems |> map(f)\n-- comment "text"\n| løn(p) -> 1');
  has(rows, 0, '_', 'variable.language.wildcard.runa');
  has(rows, 1, 'Some', 'entity.name.tag.runa');
  has(rows, 2, '|>', 'keyword.operator.pipe.runa');
  has(rows, 3, 'comment', 'comment.line.double-dash.runa');
  has(rows, 4, 'løn', 'entity.name.function.rule.runa');
});
