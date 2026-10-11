// Simple program for exercising the LLDB MCP server:
// - named functions to set breakpoints on
// - local variables and a struct to inspect
// - a loop to step through
// - a recursive call for backtrace depth
// - multiple threads

use std::thread;

#[derive(Debug)]
#[allow(dead_code)] // fields inspected via LLDB, not used in Rust code
struct Point {
    x: i32,
    y: i32,
}

fn add(a: i32, b: i32) -> i32 {
    // Breakpoint target: inspect `a`, `b`
    a + b
}

fn factorial(n: u64) -> u64 {
    // Recursive: useful for backtrace testing
    if n <= 1 {
        return 1;
    }
    n * factorial(n - 1)
}

fn sum_loop(limit: i32) -> i32 {
    // Loop: useful for step_next / continue testing
    let mut total = 0;
    for i in 0..limit {
        total += i;
    }
    total
}

fn thread_worker(id: u32) {
    // Each thread does a small amount of work.
    let result = sum_loop(5);
    println!("thread {id}: sum = {result}");
}

fn main() {
    // --- simple arithmetic ---
    let x = 6;
    let y = 7;
    let sum = add(x, y);
    println!("add({x}, {y}) = {sum}");

    // --- struct ---
    let p = Point { x: 10, y: 20 };
    println!("point = {p:?}");

    // --- loop ---
    let s = sum_loop(10);
    println!("sum_loop(10) = {s}");

    // --- recursion ---
    let f = factorial(6);
    println!("factorial(6) = {f}");

    // --- threads ---
    let handles: Vec<_> = (0..3)
        .map(|i| thread::spawn(move || thread_worker(i)))
        .collect();
    for h in handles {
        h.join().expect("thread panicked");
    }

    println!("done");
}
