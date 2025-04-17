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
        "curl 'https://api.themoviedb.org/3/movie/top_rated?api_key={}&language=en&page={}'",
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

async fn get_result() -> impl Responder {
    let json_rsp: Value = get_page().await;

    let mut returning: Value = json!({
        "results": []
    });

    //for x in 0..JSON_RSP["results"]
    //json_rsp.get("results").iter().for_each(|movie| {
    let mut num_loaded = 0;
    if let Some(results) = json_rsp.get("results").and_then(|v| v.as_array()) {
        for movie in results {
            if movie == &Value::Null
                || movie
                    .get("adult")
                    .and_then(|v| v.as_bool()).unwrap_or(true)
                || !movie.get("poster_path").is_some()
                || movie
                    .get("original_language")
                    .and_then(|v| v.as_str())
                    .map(|s| s != "en")
                    .unwrap_or(false)
                || movie
                    .get("vote_count")
                    .and_then(|v| v.as_i64())
                    .map(|n| n < 1500)
                    .unwrap_or(false)
                || movie
                    .get("release_date")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.split('-').next())
                    .and_then(|year_str| year_str.parse::<i32>().ok())
                    .map(|year| year < 1965)
                    .unwrap_or(true)
            {
                continue;
            }
            num_loaded += 1;

            println!("{}", movie.get("original_language").and_then(|v| v.as_str()).unwrap_or("yo"));
            let id = movie.get("id").map(|v| v.to_string()).unwrap_or("unknown".to_string());
            let mut _movie: Value = movie.clone();
            let credits: Value = get_credits(id).await;
            //returning.get_mut("results").push(movie.clone());
            if credits.is_null() {
                continue;
            }

            _movie.include_cast_members(&credits);

            if credits.get("crew").and_then(|v| v.as_array()).is_none() {
                continue;
            }

            let Some(director_name) = find_director_name(&credits) else {
                continue
            };

            if let Some(obj) = _movie.as_object_mut() {
                obj.insert("director".to_string(), Value::String(director_name.to_string()));
            }

            if let Some(results_array) = returning.get_mut("results").and_then(|v| v.as_array_mut()) {
                results_array.push(_movie);
            }
        }
    }
    if num_loaded > 0 {
        format!("{}", serde_json::to_string(&returning).expect("ERROR"))
    } else {
        // if nothing was loaded, try again
        format!("{}", get_page().await)
    }
}

trait MovieExt {
    fn include_cast_members(&mut self, credits: &Value) -> Option<&mut Value>;
}

impl MovieExt for Value {
    fn include_cast_members(&mut self, credits: &Value) -> Option<&mut Value> {
        if let Some(cast_array) = credits.get("cast").and_then(|v| v.as_array()) {
            if let Some(mov) = self.as_object_mut() {
                mov.insert("cast".to_string(), Value::Array(vec![]));
            }
            for i in 0..3 {
                if let Some(name) = cast_array.get(i).and_then(|r| r.as_str()) {
                    self.get_mut("cast")
                        .and_then(|v| v.as_array_mut())
                        .map(|arr| arr.push(Value::String(name.to_string())));
                }
            }
            return Some(self)
        }
        return None
    }
}

fn find_director_name(value: &Value) -> Option<String> {
    if let Some(crew_array) = value.get("crew").and_then(|v| v.as_array()) {
        for member in crew_array {
            if let Some(role) = member.get("job").and_then(|r| r.as_str()) {
                if role == "Director" {
                    if let Some(name) = member.get("name").and_then(|n| n.as_str()) {
                        return Some(name.to_string());
                    }
                }
            }
        }
    }
    return None
}

async fn get_credits(id: String) -> Value {
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


    // recursively check until a good response is found
    match serde_json::from_str(&stdout) {
        Ok(result) => result,
        Err(_) => {
            Box::pin(get_credits(id)).await
        }
    }
    /*
    let result: Value = serde_json::from_str(&stdout).expect("Invalid JSON");
    if result == "Invalid JSON" {
        return get_credits(id).await;
    } else {
        return serde_json::from_str(&stdout).expect("Invalid JSON");
    }
    */
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
