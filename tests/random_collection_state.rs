use futuruna::{Env, Interpreter, Value};

const INITIAL_STATE: u64 = 0x12345678_9abcdef0;

fn next_state(mut state: u64) -> u64 {
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    state
}

#[test]
fn random_collection_builtins_advance_one_shared_sequence() {
    let mut interpreter = Interpreter::new();
    let environment = Env::new();

    let first_state = next_state(INITIAL_STATE);
    let choice = interpreter.eval_builtin(
        "random_choice",
        vec![Value::List(vec![
            Value::Int(10),
            Value::Int(20),
            Value::Int(30),
            Value::Int(40),
            Value::Int(50),
        ])],
        &environment,
    );
    match choice {
        Value::Int(value) => {
            let expected = [10, 20, 30, 40, 50][(first_state as usize) % 5];
            assert_eq!(value, expected);
        }
        other => panic!("expected integer choice, got {other:?}"),
    }

    let second_state = next_state(first_state);
    let fraction = interpreter.eval_builtin("random_float", vec![], &environment);
    match fraction {
        Value::Float(value) => {
            let expected = second_state as f64 / u64::MAX as f64;
            assert_eq!(value.to_bits(), expected.to_bits());
        }
        other => panic!("expected floating-point value, got {other:?}"),
    }

    let mut expected_order = vec![1, 2, 3, 4];
    let mut state = second_state;
    for index in (1..expected_order.len()).rev() {
        state = next_state(state);
        let swap_index = (state as usize) % (index + 1);
        expected_order.swap(index, swap_index);
    }
    let shuffled = interpreter.eval_builtin(
        "shuffle",
        vec![Value::List(
            [1, 2, 3, 4].into_iter().map(Value::Int).collect(),
        )],
        &environment,
    );
    match shuffled {
        Value::List(values) => {
            let actual = values
                .into_iter()
                .map(|value| match value {
                    Value::Int(integer) => integer,
                    other => panic!("expected integer list element, got {other:?}"),
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected_order);
        }
        other => panic!("expected shuffled list, got {other:?}"),
    }

    let final_state = next_state(state);
    let final_fraction = interpreter.eval_builtin("random_float", vec![], &environment);
    match final_fraction {
        Value::Float(value) => {
            let expected = final_state as f64 / u64::MAX as f64;
            assert_eq!(value.to_bits(), expected.to_bits());
        }
        other => panic!("expected floating-point value, got {other:?}"),
    }
}
