// Downstream-shaped fixture: nested customer/order/line data.

#[derive(Clone, Debug)]
struct LineItem {
    sku: String,
    qty: i64,
    price_cents: i64,
}

#[derive(Clone, Debug)]
struct Order {
    id: String,
    lines: Vec<LineItem>,
}

#[derive(Clone, Debug)]
struct Customer {
    name: String,
    orders: Vec<Order>,
}

fn line_item(sku: &str, qty: i64, price_cents: i64) -> LineItem {
    LineItem {
        sku: sku.to_string(),
        qty,
        price_cents,
    }
}

fn add_line(lines: Vec<LineItem>, sku: &str, qty: i64, price_cents: i64) -> Vec<LineItem> {
    let mut out = lines.clone();
    out.push(line_item(sku, qty, price_cents));
    out
}

fn order(id: &str, lines: Vec<LineItem>) -> Order {
    Order {
        id: id.to_string(),
        lines,
    }
}

fn add_order(orders: Vec<Order>, order: Order) -> Vec<Order> {
    let mut out = orders.clone();
    out.push(order);
    out
}

fn line_total(item: &LineItem) -> i64 {
    item.qty * item.price_cents
}

fn order_total(order: &Order) -> i64 {
    line_total(&order.lines[0]) + line_total(&order.lines[1])
}

fn customer_total(customer: &Customer) -> i64 {
    order_total(&customer.orders[0]) + order_total(&customer.orders[1])
}

fn main() {
    let mut first_lines = Vec::new();
    first_lines = add_line(first_lines, "coffee", 2, 450);
    first_lines = add_line(first_lines, "cake", 1, 700);

    let mut second_lines = Vec::new();
    second_lines = add_line(second_lines, "tea", 3, 325);
    second_lines = add_line(second_lines, "mug", 1, 1200);

    let mut orders = Vec::new();
    orders = add_order(orders, order("A-100", first_lines));
    orders = add_order(orders, order("A-101", second_lines));

    let customer = Customer {
        name: "Ada".to_string(),
        orders,
    };

    println!("customer={}", customer.name.clone());
    println!("orders={}", customer.orders.len());
    println!("first_order={}", customer.orders[0].id.clone());
    println!("first_sku={}", customer.orders[0].lines[0].sku.clone());
    println!("total={}", customer_total(&customer));
}
