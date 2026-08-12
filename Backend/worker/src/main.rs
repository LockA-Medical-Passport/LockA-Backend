fn main() {
    let settings = config::Settings::load().unwrap_or_else(|err| {
        eprintln!("configuration error: {err}");
        std::process::exit(1);
    });

    println!("Hello, world! worker tick interval: {}s", settings.worker_tick_interval_secs);
}
