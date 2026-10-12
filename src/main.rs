use clap::Parser;
use d4c::cli::{Cli, Commands};
use d4c::engine::Engine;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();

    let result = match cli.command {
        None => Engine::ui().await,
        Some(Commands::Ui) => Engine::ui().await,
        Some(Commands::Up {
            service_type,
            port,
            name,
            password,
            tag,
            cpus,
            memory,
            network,
            image,
        }) => Engine::up(
            service_type,
            port,
            name,
            password,
            tag,
            cpus,
            memory,
            network,
            image,
        ),
        Some(Commands::Ps { json }) => Engine::ps(json),
        Some(Commands::Start { service }) => Engine::start(&service),
        Some(Commands::Stop { service }) => Engine::stop(&service),
        Some(Commands::Rm { service }) => Engine::rm(&service),
        Some(Commands::Wipe { service, force }) => Engine::wipe(&service, force),
        Some(Commands::Logs {
            service,
            follow,
            tail,
        }) => Engine::logs(&service, follow, tail),
        Some(Commands::Update {
            service,
            cpus,
            memory,
        }) => Engine::update(&service, cpus, memory),
        Some(Commands::Tag { service, tag }) => Engine::tag(&service, tag),
        Some(Commands::Snap(cmd)) => Engine::snap(cmd),
        Some(Commands::Net(cmd)) => Engine::net(cmd).await,
        Some(Commands::Compose { output, mode }) => Engine::compose(output, &mode),
        Some(Commands::Search { query }) => Engine::search(&query),
    };

    if let Err(err) = result {
        let err_style = console::Style::new().red().bold();
        eprintln!("{} {:#}", err_style.apply_to("Error:"), err);
        std::process::exit(1);
    }
}
