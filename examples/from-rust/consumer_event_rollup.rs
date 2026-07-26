// Consumer-shaped fixture: event rollup by category

use std::collections::BTreeMap;

#[derive(Clone, Debug)]
struct Event {
    kind: String,
    amount: i64,
}

fn event(kind: &str, amount: i64) -> Event {
    Event {
        kind: kind.to_string(),
        amount,
    }
}

fn add_event(events: Vec<Event>, kind: &str, amount: i64) -> Vec<Event> {
    let mut out = events.clone();
    out.push(event(kind, amount));
    out
}

fn totals_by_kind(events: &Vec<Event>) -> BTreeMap<String, i64> {
    let mut totals = BTreeMap::new();
    for event in events {
        let current = totals.get(&event.kind).unwrap_or(&0);
        totals.insert(event.kind.clone(), *current + event.amount);
    }
    totals
}

fn main() {
    let mut events = Vec::new();
    events = add_event(events, "login", 1);
    events = add_event(events, "purchase", 7);
    events = add_event(events, "login", 3);
    events = add_event(events, "refund", -2);

    let totals = totals_by_kind(&events);
    println!("events={}", events.len());
    println!("login={}", totals.get("login").unwrap_or(&0));
    println!("purchase={}", totals.get("purchase").unwrap_or(&0));
    println!("refund={}", totals.get("refund").unwrap_or(&0));
}
