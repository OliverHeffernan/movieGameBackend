use actix_web::{web, App, HttpServer, Responder};
use actix_cors::Cors;
use rustls::{ServerConfig, Certificate, PrivateKey};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::fs::File;
use std::io::BufReader;
use std::process::Command;
use rand::Rng;
use serde_json::{Value, /*from_str,*/ json};
//use tokio::runtime::Runtime;

mod api_key;

const API_KEY: &str = api_key::API_KEY;

/// requests a page of movies from the TMDb API, returns it as a Value.
/// Takes no props, and doesn't alter any part of the response from the API.
async fn get_page() -> Value {

    // getting a random number to decide what page should be used
    let mut rng = rand::thread_rng();
    let random_page = rng.gen_range(1..=100);

    // converting that number to a string
    let page_str: &str = &random_page.to_string();

    // preparing the curl command to make the API request
    let curl_cmd = format!(
        "curl 'https://api.themoviedb.org/3/movie/top_rated?api_key={}&language=en&page={}'",
        API_KEY,
        page_str);

    // making the API request
    let output = Command::new("sh")
        .arg("-c")
        .arg(curl_cmd)
        .output()
        .expect("Failed to execute command");

    // converting the respone from the API request to JSON/Value format from a String
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_rsp: Value = serde_json::from_str(&stdout).expect("Invalid JSON");

    return json_rsp;
}

/// filters out movies that are too obscure to be suitable for a guess the movie game
/// also ensures that the movies are appropriate
/// takes in a Movie, and returns a bool, stating whether that movie should be included
fn filter(movie: &Value) -> bool {
    return movie == &Value::Null
    // filtering out adult movies
    || movie
        .get("adult")
        .and_then(|v| v.as_bool()).unwrap_or(true)
    // making sure that the movie actually has a poster to be used in the game
    || !movie.get("poster_path").is_some()
    // ensuring the movie is in english, otherwise it will be very hard to guess the name of it
    || movie
        .get("original_language")
        .and_then(|v| v.as_str())
        .map(|s| s != "en")
        .unwrap_or(false)
    // check the vote count as a measure of its popularity
    || movie
        .get("vote_count")
        .and_then(|v| v.as_i64())
        .map(|n| n < 1500)
        .unwrap_or(false)
    // only include movies that were released after 1965
    || movie
        .get("release_date")
        .and_then(|v| v.as_str())
        .and_then(|s| s.split('-').next())
        .and_then(|year_str| year_str.parse::<i32>().ok())
        .map(|year| year < 1965)
        .unwrap_or(true)
}

/// gets the results, filters them, and packages the top 3 cast members and the director
/// Uses methods:
///     get_page
///     get_credits
///     include_cast_members
///     find_director_name
async fn get_result() -> impl Responder {
    // get the response from the API request
    let json_rsp: Value = get_page().await;

    // prepare the Value which will be returned
    let mut returning: Value = json!({
        "results": []
    });

    // count the number of movies that get through the filters
    let mut num_loaded = 0;


    if let Some(results) = json_rsp.get("results").and_then(|v| v.as_array()) {
        // cycle through the movies in the API response
        for movie in results {
            // if the movie is filtered out, skip it
            if filter(movie) {
                continue;
            }

            // get the movie's id as a string
            let id = movie.get("id").map(|v| v.to_string()).unwrap_or("unknown".to_string());
            // get a clone of the movie, so that we can add the cast and the director as keys
            let mut _movie: Value = movie.clone();
            // get the credits, using the id
            let credits: Value = get_credits(id).await;
            // if no credits, skip the movie
            if credits.is_null() {
                continue;
            }
            // add the cast members to the _movie object
            _movie.include_cast_members(&credits);
            // if there is no crew data, we skip the movie
            if credits.get("crew").and_then(|v| v.as_array()).is_none() {
                continue;
            }
            // if the director is not found, the movie should be skipped
            // if that fails, we skip this movie
            let Some(director_name) = find_director_name(&credits) else {
                continue;
            };
            // add the director to the object
            if let Some(obj) = _movie.as_object_mut() {
                obj.insert("director".to_string(), Value::String(director_name.to_string()));
            }
            // add the movie to the returning object
            if let Some(results_array) = returning.get_mut("results").and_then(|v| v.as_array_mut()) {
                results_array.push(_movie);
            }
            // if we got this far, we know the movie has been added, so we can count it
            num_loaded += 1;
        }
    }
    // if there was at least 1 movie loaded, we can send the response
    if num_loaded > 0 {
        format!("{}", serde_json::to_string(&returning).expect("ERROR"))
    } else {
        // otherwise, if nothing was loaded, try again
        format!("{}", get_page().await)
    }
}

/// setup the trait for the include_cast_members method
trait MovieExt {
    fn include_cast_members(&mut self, credits: &Value) -> Option<&mut Value>;
}

impl MovieExt for Value {
    /// adds the top 3 cast members to the movie object
    fn include_cast_members(&mut self, credits: &Value) -> Option<&mut Value> {
        if let Some(cast_array) = credits.get("cast").and_then(|v| v.as_array()) {
            // add the cast key to the object
            if let Some(mov) = self.as_object_mut() {
                mov.insert("cast".to_string(), Value::Array(vec![]));
            }
            // add 3 cast members names to the vector
            for i in 0..3 {
                if let Some(name) = cast_array.get(i).and_then(|r| r.get("name")).and_then(|n| n.as_str()) {
                    self.get_mut("cast")
                        .and_then(|v| v.as_array_mut())
                        .map(|arr| arr.push(Value::String(name.to_string())));
                }
            }
            return Some(self)
        }
        // if that failed, return None
        return None
    }
}

/// finds the name of the director. Takes the crew Value as a prop.
/// Returns an <Option>String with the name of the director.
/// If not found, returns None
fn find_director_name(crew: &Value) -> Option<String> {
    if let Some(crew_array) = crew.get("crew").and_then(|v| v.as_array()) {
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

/// API request to get credits. Takes the id of the movie as a String.
/// Returns a Value, of the credits
async fn get_credits(id: String) -> Value {
    // preparing the curl command
    let curl_cmd = format!(
        "curl https://api.themoviedb.org/3/movie/{}/credits?api_key={}",
        id,
        API_KEY
    );

    // complete the curl command
    let output = Command::new("sh")
        .arg("-c")
        .arg(curl_cmd)
        .output()
        .expect("Failed to execute command");

    // get the response as a string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // recursively check until a good response is found
    match serde_json::from_str(&stdout) {
        Ok(result) => result,
        Err(_) => {
            Box::pin(get_credits(id)).await
        }
    }
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

/*
//just testing the backend function specifically, leaving the server code for now
fn main() {
    let rt = Runtime::new().unwrap();
    let result = rt.block_on(get_result());
    println!("{}", result);
}
*/
