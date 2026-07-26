// Downstream-shaped fixture: deterministic collection/reporting workflow.

use std::collections::BTreeMap;

#[derive(Clone, Debug)]
struct StockItem {
    category: String,
    count: i64,
}

fn stock_item(category: &str, count: i64) -> StockItem {
    StockItem {
        category: category.to_string(),
        count,
    }
}

fn add_item(items: Vec<StockItem>, category: &str, count: i64) -> Vec<StockItem> {
    let mut out = items.clone();
    out.push(stock_item(category, count));
    out
}

fn totals_by_category(items: &Vec<StockItem>) -> BTreeMap<String, i64> {
    let mut totals = BTreeMap::new();
    for item in items {
        let current = totals.get(&item.category).unwrap_or(&0);
        totals.insert(item.category.clone(), *current + item.count);
    }
    totals
}

fn main() {
    let mut items = Vec::new();
    items = add_item(items, "hardware", 4);
    items = add_item(items, "books", 7);
    items = add_item(items, "hardware", 5);
    items = add_item(items, "tools", 2);
    items = add_item(items, "books", 3);

    let totals = totals_by_category(&items);
    println!("items={}", items.len());
    println!("books={}", totals.get("books").unwrap_or(&0));
    println!("hardware={}", totals.get("hardware").unwrap_or(&0));
    println!("tools={}", totals.get("tools").unwrap_or(&0));
}
