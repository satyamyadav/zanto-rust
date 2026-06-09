mod tools;
mod chat;

#[tokio::main]
async fn main() {
    match chat::chat().await {
        Ok(_) => println!("Done."),
        Err(e) => eprintln!("Error: {}", e),
    }
}
