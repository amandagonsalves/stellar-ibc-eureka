use std::process::ExitCode;

fn print_help() {
    println!(
        "stellar-osmosis — manage the local Osmosis appchain (osmosis service in docker-compose.yml)\n\n\
         USAGE:\n    stellar-osmosis <COMMAND> [--stateful]\n\n\
         COMMANDS:\n    \
         start      Start the local Osmosis appchain and wait until it produces blocks\n    \
         stop       Stop the local Osmosis appchain\n    \
         health     Report whether the local Osmosis appchain is producing blocks\n    \
         help       Show this message\n\n\
         FLAGS:\n    \
         --stateful   Keep existing chain state instead of resetting it (start only)"
    );
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(String::as_str).unwrap_or("help");
    let stateful = args.iter().any(|arg| arg == "--stateful");

    let result = match command {
        "start" => stellar_osmosis::start(stateful).await,
        "stop" => stellar_osmosis::stop(),
        "health" => stellar_osmosis::report().await,
        "help" | "-h" | "--help" => {
            print_help();
            return ExitCode::SUCCESS;
        }
        other => {
            eprintln!("unknown command '{other}'\n");
            print_help();
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
