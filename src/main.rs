use actix_web::{web, App, HttpServer, Responder};
use actix_cors::Cors;
use rustls::{ServerConfig, Certificate, PrivateKey};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::fs::File;
use std::io::BufReader;
use std::process::Command;
//use std::borrow::Cow;
use rand::Rng;
use serde_json::{Value, /*from_str,*/ json};
//use tokio::runtime::Runtime;

mod api_key;

const API_KEY: &str = api_key::API_KEY;

async fn get_page() -> Value {
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
    let json_rsp: Value = serde_json::from_str(&stdout).expect("Invalid JSON");

    return json_rsp;
}

async fn get_result() -> String {
    let json_rsp: Value = get_page().await;

    let mut returning: Value = json!({
        "results": []
    });

    //for x in 0..JSON_RSP["results"]
    //json_rsp.get("results").iter().for_each(|movie| {
    if let Some(results) = json_rsp.get("results").and_then(|v| v.as_array()) {
        for movie in results {
            if movie != &Value::Null
                && !movie
                    .get("adult")
                    .and_then(|v| v.as_bool()).unwrap_or(true)
                && movie.get("poster_path").is_some()
                && movie
                    .get("original_language")
                    .and_then(|v| v.as_str())
                    .map(|s| s == "en")
                    .unwrap_or(false)
                && movie
                    .get("vote_count")
                    .and_then(|v| v.as_i64())
                    .map(|n| n > 1500)
                    .unwrap_or(false)
                && movie
                    .get("release_date")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.split('-').next())
                    .and_then(|year_str| year_str.parse::<i32>().ok())
                    .map(|year| year < 1965)
                    .unwrap_or(false)
            {
                let id = movie.get("id").map(|v| v.to_string()).unwrap_or("unknown".to_string());
                let mut _movie: Value = movie.clone();
                let credits: Value = get_credits(id).await;
                //returning.get_mut("results").push(movie.clone());
                if !credits.is_null() {
                    // preparing an empty json object which will be inserted for the movie
                    let mut _credits: Value = json!({
                        "crew": [],
                        "cast": []
                    });

                    // putting the top 3 cast members into the returning
                    let mut nuf_info = true;
                    if let Some(array) = credits.get("cast").and_then(|v| v.as_array()) {
                        for i in 0..3 {
                            if let Some(item) = array.get(i) {
                                //let item: Value = array.get(i).expect("No element at index").clone();
                                _credits
                                    .get_mut("cast")
                                    .and_then(|v| v.as_array_mut())
                                    .map(|arr| arr.push(item.clone()));
                            } else {
                                nuf_info = false;
                            }
                        }
                    } else {
                        nuf_info = false;
                    }

                    if let Some(crew) = credits.get("crew").and_then(|v| v.as_array()) {
                        let mut director: Option<Value> = None;
                        let mut found = false;
                        let mut maybe_found = false;
                        for member in crew {
                            if let Some("Directing") = member.get("department").and_then(|v| v.as_str()) {
                                // this guy is definitely the director, so we should put him
                                // straight into returning.
                                _credits.get_mut("crew")
                                    .and_then(|v| v.as_array_mut())
                                    .map(|arr| arr.push(member.clone()));
                                found = true;
                                break;
                            } else if let Some("Directing") = member.get("known_for_department").and_then(|v| v.as_str()) {
                                // this guy could be the director, so we save him, but we hope we
                                // can find a definite director
                                director = Some(member.clone());
                                maybe_found = true;
                            }
                        }
                        // if we don't have a definite director, but we do have a maybe director,
                        // we push the maybe director
                        // otherwise we don't have enough info to use this movie
                        if !found && maybe_found {
                            if let Some(director) = director {
                                _credits.get_mut("crew")
                                    .and_then(|v| v.as_array_mut())
                                    .map(|arr| arr.push(director.clone()));
                            }
                        } else if !found {
                            nuf_info = false;
                            println!("nuf info 3");
                        }
                    } else {
                        nuf_info = false;
                    }

                    if let Some(movie_obj) = _movie.as_object_mut() {
                        if nuf_info {
                            movie_obj.insert("credits".to_string(), credits);
                            returning
                                .get_mut("results")
                                .and_then(|v| v.as_array_mut())
                                .map(|arr| arr.push(Value::Object(movie_obj.clone())));

                            if let Some(arr) = returning.get_mut("results").and_then(|v| v.as_array_mut()) {
                                arr.push(Value::Object(movie_obj.clone()));
                            }
                        }
                    }
                }
            }
        }
        if let Some(arr) = returning.get_mut("results").and_then(|v| v.as_array_mut()) {
            arr.push(json!("hello"));
        }
    }

    format!("Result: {}", serde_json::to_string(&returning).expect("ERROR"))
}

async fn get_credits(movie_id: String) -> Value {
    let id = movie_id;
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
    //let stdout_str: &str = &stdout;
    //let result: Cow<'_, str> = Cow::Borrowed(&stdout_str);
    //println!("{}", result);
    //format!("Result: {}", result)
    //
    return serde_json::from_str(&stdout).expect("Invalid JSON");
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
            //.route("/credits/{id}", web::get().to(get_credits))
            .route("/movie", web::get().to(get_result))
    })
        .bind_rustls_021("0.0.0.0:8443", tls_config)?
        .run()
    .await
}


//just testing the backend function specifically, leaving the server code for now
/*
fn main() {
    let rt = Runtime::new().unwrap();
    let result = rt.block_on(get_result());
    println!("{}", result);
}
*/
