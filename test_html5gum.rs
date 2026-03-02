use html5gum::{Tokenizer, Token};

fn main() {
    let html = "<script type=\"application/ld+json\">{\"a\": 1}</script>";
    for token in Tokenizer::new(html).infallible() {
        match token {
            Token::StartTag(tag) => println!("Start: {:?}", String::from_utf8_lossy(&tag.name)),
            Token::String(s) => println!("String: {:?}", String::from_utf8_lossy(&s)),
            Token::EndTag(tag) => println!("End: {:?}", String::from_utf8_lossy(&tag.name)),
            _ => {}
        }
    }
}
