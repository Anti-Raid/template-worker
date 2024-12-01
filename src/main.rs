mod cmds;
mod dispatch;
mod docgen;
mod event_handler;
mod expiry_tasks;
mod http;
mod on_startup;
mod props;
mod temporary_punishments;

/// The main function is just a command handling function
#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("templatedocs") => {
            cmds::docgen::docgen();
        }
        Some("start") => {
            cmds::bot::start().await;
        }
        _ => {
            println!(
                "No/unknown command specified.\n\nstart: [start the template worker itself]\ntemplatedocs: [generate template docs]"
            );
            std::process::exit(1);
        }
    };
}
