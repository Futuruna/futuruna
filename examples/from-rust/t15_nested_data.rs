// T15: Recursive data structures (linked list)
#[derive(Debug, Clone)]
enum IntList {
    Nil,
    Cons(i64, Box<IntList>),
}

fn list_sum(xs: &IntList) -> i64 {
    match xs {
        IntList::Nil => 0,
        IntList::Cons(h, t) => h + list_sum(t),
    }
}

fn list_len(xs: &IntList) -> i64 {
    match xs {
        IntList::Nil => 0,
        IntList::Cons(_, t) => 1 + list_len(t),
    }
}

fn list_map(xs: &IntList, f: fn(i64) -> i64) -> IntList {
    match xs {
        IntList::Nil => IntList::Nil,
        IntList::Cons(h, t) => IntList::Cons(f(*h), Box::new(list_map(t, f))),
    }
}

fn list_to_string(xs: &IntList) -> String {
    match xs {
        IntList::Nil => "nil".to_string(),
        IntList::Cons(h, t) => format!("{}:{}", h, list_to_string(t)),
    }
}

fn main() {
    let xs = IntList::Cons(1, Box::new(IntList::Cons(2, Box::new(IntList::Cons(3,
        Box::new(IntList::Nil))))));
    println!("{}", list_to_string(&xs));
    println!("{}", list_sum(&xs));
    println!("{}", list_len(&xs));
    let doubled = list_map(&xs, |x| x * 2);
    println!("{}", list_to_string(&doubled));
}
