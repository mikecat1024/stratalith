use std::hint::black_box;
use std::time::{Duration, Instant};
use stratalith_markdown::{parse, serialize};

fn median_per_call(iterations: u32, mut run: impl FnMut()) -> Duration {
    let mut samples = [Duration::ZERO; 7];
    for sample in &mut samples {
        let start = Instant::now();
        for _ in 0..iterations {
            run();
        }
        *sample = start.elapsed() / iterations;
    }
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn main() {
    let prose = "A paragraph with several words and a café. ".repeat(2400);
    let mixed = "# Heading\n\n- **bold** with [link](a)\n- `code` and *emphasis*\n\n".repeat(300);
    let unmatched: String = (1..=540).map(|n| format!("x{}", "`".repeat(n))).collect();

    for (name, source, iterations) in [
        ("prose", &prose, 20),
        ("mixed", &mixed, 15),
        ("unmatched backticks", &unmatched, 3),
    ] {
        let duration = median_per_call(iterations, || {
            black_box(parse(black_box(source)).unwrap());
        });
        println!("parse {name} ({} bytes)  {duration:?}", source.len());
    }

    let document = parse(&mixed).unwrap();
    let duration = median_per_call(15, || {
        black_box(serialize(black_box(&document)).unwrap());
    });
    println!("serialize mixed  {duration:?}");
}
