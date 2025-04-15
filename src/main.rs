use actix_web::{web, App, HttpServer, Responder};
use actix_cors::Cors;
use rustls::{ServerConfig, Certificate, PrivateKey};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::fs::File;
use std::io::BufReader;
use std::process::Command;
use std::borrow::Cow;
use rand::Rng;
use serde_json::{Value, from_str, json};

mod api_key;

const API_KEY: &str = api_key::API_KEY;

async fn get_result() -> impl Responder {
    let mut rng = rand::thread_rng();

    let random_page = rng.gen_range(1..=100);
    let page_str: &str = &random_page.to_string();

    let curl_cmd = format!(
        "curl 'https://api.themoviedb.org/3/movie/top_rated?api_key={}&language=en-US&page={}'",
        API_KEY,
        page_str);

    let output = Command::new("sh")
        .arg("-c")
        .arg(curl_cmd)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout_str: &str = &stdout;
    let result: Cow<'_, str> = Cow::Borrowed(&stdout_str);

    format!("Result: {}", result)
}

async fn get_credits(movie_id: web::Path<String>) -> impl Responder {
    let id = movie_id.into_inner();
    let curl_cmd = format!(
        "curl https://api.themoviedb.org/3/movie/{}/credits?api_key={}",
        id,
        API_KEY
        );

    let output = Command::new("sh")
        .arg("-c")
        .arg(curl_cmd)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout_str: &str = &stdout;
    let result: Cow<'_, str> = Cow::Borrowed(&stdout_str);
    println!("{}", result);
    format!("Result: {}", result)
}

fn load_tls_config() -> ServerConfig {
    let cert_file = &mut BufReader::new(File::open("cert.pem").unwrap());
    let key_file = &mut BufReader::new(File::open("key.pem").unwrap());

    let cert_chain = certs(cert_file)
        .unwrap()
        .into_iter()
        .map(Certificate)
        .collect();
    let mut keys = pkcs8_private_keys(key_file).unwrap();
    let key = PrivateKey(keys.pop().unwrap());

    ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)
        .unwrap()
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let tls_config = load_tls_config();

    HttpServer::new(|| {
        App::new()
            //.wrap(Cors::permissive())
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allowed_header(actix_web::http::header::CONTENT_TYPE)
                    .allowed_header("ngrok-skip-browser-warning")
                    .max_age(3600)
            )
            .route("/credits/{id}", web::get().to(get_credits))
            .route("/movie", web::get().to(get_result))
    })
    .bind_rustls_021("0.0.0.0:8443", tls_config)?
        .run()
        .await
}


