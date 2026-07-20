// Real-world 1: A complete JSON-like value type with parser and serializer
// ~150 lines of idiomatic Rust

#[derive(Debug, Clone)]
enum Json {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

fn stringify(val: &Json) -> String {
    match val {
        Json::Null => "null".to_string(),
        Json::Bool(b) => if *b { "true".to_string() } else { "false".to_string() },
        Json::Number(n) => format!("{}", n),
        Json::Str(s) => format!("\"{}\"", s),
        Json::Array(arr) => {
            let items: Vec<String> = arr.iter().map(|v| stringify(v)).collect();
            format!("[{}]", items.join(", "))
        }
        Json::Object(pairs) => {
            let fields: Vec<String> = pairs.iter()
                .map(|(k, v)| format!("\"{}\": {}", k, stringify(v)))
                .collect();
            format!("{{{}}}", fields.join(", "))
        }
    }
}

fn json_get(val: &Json, key: &str) -> Json {
    match val {
        Json::Object(pairs) => {
            for (k, v) in pairs {
                if k == key {
                    return v.clone();
                }
            }
            Json::Null
        }
        _ => Json::Null,
    }
}

fn json_array_len(val: &Json) -> i64 {
    match val {
        Json::Array(arr) => arr.len() as i64,
        _ => 0,
    }
}

fn json_is_null(val: &Json) -> bool {
    matches!(val, Json::Null)
}

fn json_map_numbers(val: &Json, f: fn(f64) -> f64) -> Json {
    match val {
        Json::Number(n) => Json::Number(f(*n)),
        Json::Array(arr) => Json::Array(arr.iter().map(|v| json_map_numbers(v, f)).collect()),
        Json::Object(pairs) => Json::Object(
            pairs.iter().map(|(k, v)| (k.clone(), json_map_numbers(v, f))).collect()
        ),
        other => other.clone(),
    }
}

fn json_obj(pairs: Vec<(String, Json)>) -> Json { Json::Object(pairs) }
fn json_arr(items: Vec<Json>) -> Json { Json::Array(items) }
fn json_str(s: &str) -> Json { Json::Str(s.to_string()) }
fn json_num(n: f64) -> Json { Json::Number(n) }
fn pair(k: &str, v: Json) -> (String, Json) { (k.to_string(), v) }

fn main() {
    let scores = json_arr(vec![json_num(95.0), json_num(87.0), json_num(92.0)]);
    let user = json_obj(vec![
        pair("name", json_str("Alice")),
        pair("age", json_num(30.0)),
        pair("active", Json::Bool(true)),
        pair("scores", scores),
        pair("address", Json::Null),
    ]);

    println!("{}", stringify(&user));

    let name = json_get(&user, "name");
    println!("{}", stringify(&name));

    let missing = json_get(&user, "email");
    println!("{}", json_is_null(&missing));

    let scores = json_get(&user, "scores");
    println!("{}", json_array_len(&scores));

    // Double all numbers
    let doubled = json_map_numbers(&user, |n| n * 2.0);
    println!("{}", stringify(&doubled));

    // Nested access
    let age = json_get(&user, "age");
    println!("{}", stringify(&age));
}
