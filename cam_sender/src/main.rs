use reqwest::{multipart, Body};

#[tokio::main]
async fn main() {
    println!("Let's send some stuff!");

    let chunks: Vec<Result<_, std::io::Error>> = vec![Ok("hello"), Ok(" "), Ok("you"), Ok("coder")];

    let stream = futures_util::stream::iter(chunks);

    let chunk = multipart::Part::stream(Body::wrap_stream(stream));

    let form = multipart::Form::new()
        .text("session", "1")
        .part("chunk", chunk);

    reqwest::Client::new()
        .post("http://127.0.0.1:3000/chunks")
        .multipart(form)
        .send()
        .await
        .unwrap();
}
