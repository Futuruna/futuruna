// T09: Nested matching, guards, complex patterns
enum Tree {
    Leaf(i64),
    Node(Box<Tree>, Box<Tree>),
}

fn tree_sum(t: &Tree) -> i64 {
    match t {
        Tree::Leaf(n) => *n,
        Tree::Node(l, r) => tree_sum(l) + tree_sum(r),
    }
}

fn tree_depth(t: &Tree) -> i64 {
    match t {
        Tree::Leaf(_) => 1,
        Tree::Node(l, r) => {
            let ld = tree_depth(l);
            let rd = tree_depth(r);
            1 + if ld > rd { ld } else { rd }
        }
    }
}

fn tree_count(t: &Tree) -> i64 {
    match t {
        Tree::Leaf(_) => 1,
        Tree::Node(l, r) => tree_count(l) + tree_count(r),
    }
}

fn main() {
    let tree = Tree::Node(
        Box::new(Tree::Node(
            Box::new(Tree::Leaf(1)),
            Box::new(Tree::Leaf(2)),
        )),
        Box::new(Tree::Leaf(3)),
    );
    println!("{}", tree_sum(&tree));
    println!("{}", tree_depth(&tree));
    println!("{}", tree_count(&tree));
}
