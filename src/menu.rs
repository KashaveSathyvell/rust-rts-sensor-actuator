use std::io::{self, Write};

pub fn show_menu() {
    println!("\n===========================================");
    println!("Real-Time Sensor-Actuator System");
    println!("===========================================");
    println!("Select an option:");
    println!("1. Threaded Implementation Demo");
    println!("2. Async Implementation Demo");
    println!("3. Benchmark Mode (Async vs Threaded)");
    println!("4. Real-Time Dashboard");
    println!("5. Statistical Benchmark Mode (Criterion)");
    println!("6. Exit");
    println!("===========================================");
    print!("Choice (1-6): ");
    io::stdout().flush().unwrap();
}

pub fn get_user_choice() -> Result<u32, std::num::ParseIntError> {
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().parse::<u32>()
}

pub fn wait_for_enter() {
    println!("\nPress Enter to return to menu...");
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
}
