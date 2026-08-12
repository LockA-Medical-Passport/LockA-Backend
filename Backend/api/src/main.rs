fn main() {
    let settings = config::Settings::load().unwrap_or_else(|err| {
        eprintln!("configuration error: {err}");
        std::process::exit(1);
    });

    println!("Hello, world! api will bind to {}", settings.api_bind_addr);
}
