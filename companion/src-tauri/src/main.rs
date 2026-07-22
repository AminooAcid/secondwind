fn main() {
    // Headless job mode (Explorer context menu): --job <preset> <input>.
    let args: Vec<String> = std::env::args().collect();
    if let Some(index) = args.iter().position(|arg| arg == "--job") {
        std::process::exit(secondwind_companion::jobs_cli::run(&args[index + 1..]));
    }
    // Headless validation/support: --discover / --pair / --node-health.
    if args.iter().any(|arg| {
        arg == "--discover" || arg == "--pair" || arg == "--node-health"
    }) {
        std::process::exit(secondwind_companion::dev_cli::run(&args[1..]));
    }

    secondwind_companion::run();
}
