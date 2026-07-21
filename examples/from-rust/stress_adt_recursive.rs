// Stress: Recursive ADTs, deep pattern matching
#[derive(Debug, Clone)]
enum Tree {
    Leaf(i64),
    Branch(Box<Tree>, Box<Tree>),
}

fn tree_sum(t: &Tree) -> i64 {
    match t {
        Tree::Leaf(n) => *n,
        Tree::Branch(l, r) => tree_sum(l) + tree_sum(r),
    }
}

fn tree_depth(t: &Tree) -> i64 {
    match t {
        Tree::Leaf(_) => 0,
        Tree::Branch(l, r) => {
            let ld = tree_depth(l);
            let rd = tree_depth(r);
            1 + if ld > rd { ld } else { rd }
        }
    }
}

fn tree_count(t: &Tree) -> i64 {
    match t {
        Tree::Leaf(_) => 1,
        Tree::Branch(l, r) => tree_count(l) + tree_count(r),
    }
}

fn main() {
    let t = Tree::Branch(
        Box::new(Tree::Branch(
            Box::new(Tree::Leaf(1)),
            Box::new(Tree::Leaf(2)),
        )),
        Box::new(Tree::Branch(
            Box::new(Tree::Leaf(3)),
            Box::new(Tree::Leaf(4)),
        )),
    );
    println!("{}", tree_sum(&t));
    println!("{}", tree_depth(&t));
    println!("{}", tree_count(&t));
}
