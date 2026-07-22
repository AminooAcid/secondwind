fn main() {
    // Headless job mode (Explorer context menu): --job <preset> <input>.
    let args: Vec<String> = std::env::args().collect();
    if let Some(index) = args.iter().position(|arg| arg == "--job") {
        std::process::exit(secondwind_companion::jobs_cli::run(&args[index + 1..]));
    }

    secondwind_companion::run();
}
