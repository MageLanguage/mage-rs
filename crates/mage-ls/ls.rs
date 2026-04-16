mod document;
mod server;
mod transport;

fn main() {
    let mut language_server = server::LanguageServer::new();
    if let Err(error) = language_server.run() {
        eprintln!("Language server error: {}", error);
        std::process::exit(1);
    }
}
