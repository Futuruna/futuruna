// Adversarial 1: Complex ownership patterns
// The kind of Rust that makes beginners cry
// runa-from-rust: expect-unsupported recursive ownership with borrowed return references

use std::collections::HashMap;

struct Node {
    value: i64,
    children: Vec<Box<Node>>,
}

impl Node {
    fn new(value: i64) -> Self {
        Node { value, children: Vec::new() }
    }

    fn add_child(&mut self, child: Node) {
        self.children.push(Box::new(child));
    }

    fn sum(&self) -> i64 {
        self.value + self.children.iter().map(|c| c.sum()).sum::<i64>()
    }

    fn depth(&self) -> i64 {
        if self.children.is_empty() {
            1
        } else {
            1 + self.children.iter().map(|c| c.depth()).max().unwrap_or(0)
        }
    }

    fn find(&self, target: i64) -> Option<&Node> {
        if self.value == target {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find(target) {
                return Some(found);
            }
        }
        None
    }
}

fn build_tree() -> Node {
    let mut root = Node::new(1);
    let mut left = Node::new(2);
    left.add_child(Node::new(4));
    left.add_child(Node::new(5));
    root.add_child(left);
    root.add_child(Node::new(3));
    root
}

fn main() {
    let tree = build_tree();
    println!("Sum: {}", tree.sum());
    println!("Depth: {}", tree.depth());

    match tree.find(4) {
        Some(node) => println!("Found: {}", node.value),
        None => println!("Not found"),
    }
}
