// Consumer-shaped fixture: invoice line totals

#[derive(Clone, Debug)]
struct LineItem {
    name: String,
    qty: i64,
    price_cents: i64,
}

fn line_item(name: &str, qty: i64, price_cents: i64) -> LineItem {
    LineItem {
        name: name.to_string(),
        qty,
        price_cents,
    }
}

fn add_item(items: Vec<LineItem>, name: &str, qty: i64, price_cents: i64) -> Vec<LineItem> {
    let mut out = items.clone();
    out.push(line_item(name, qty, price_cents));
    out
}

fn line_total(item: &LineItem) -> i64 {
    item.qty * item.price_cents
}

fn invoice_total(items: &Vec<LineItem>) -> i64 {
    let mut total = 0;
    for item in items {
        total += line_total(item);
    }
    total
}

fn main() {
    let mut items = Vec::new();
    items = add_item(items, "coffee", 2, 450);
    items = add_item(items, "tea", 1, 325);
    items = add_item(items, "cake", 3, 250);

    println!("items={}", items.len());
    println!("first={}", items[0].name.clone());
    println!("first_total={}", line_total(&items[0]));
    println!("invoice_total={}", invoice_total(&items));
}
