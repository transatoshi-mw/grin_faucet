use std::process::Command;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use warp::Filter;
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};

#[derive(Deserialize)]
struct Request {
    address: String,
}

#[derive(Serialize)]
struct Response {
    message: String,
}

struct RateLimiter {
    last_sent: HashMap<String, chrono::DateTime<Utc>>,
}

#[tokio::main]
async fn main() {
    let rate_limiter = Arc::new(Mutex::new(RateLimiter {
        last_sent: HashMap::new(),
    }));

    let rate_limiter_filter = warp::any().map(move || rate_limiter.clone());

    let send_faucet = warp::post()
        .and(warp::path("send"))
        .and(warp::body::json())
        .and(rate_limiter_filter)
        .map(|request: Request, rate_limiter: Arc<Mutex<RateLimiter>>| {
            let address = request.address;

            let mut rate_limiter = rate_limiter.lock().unwrap();
            let now = Utc::now();

            // Check if the address has been sent funds in the last 24 hours
            if let Some(last_sent) = rate_limiter.last_sent.get(&address) {
                if now - *last_sent < Duration::hours(24) {
                    return warp::reply::json(&Response {
                        message: "You can only request funds every 24 hours.".to_string(),
                    });
                }
            }

            // Execute the command
            let output = Command::new("bash")
                .arg("-c")
                .arg(format!("echo '<PASSWORD>' | grin-wallet send -d {} 1000", address))
                .output()
                .expect("Failed to execute command");

            // Update the last sent time
            rate_limiter.last_sent.insert(address.clone(), now);

            // Return the command output
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            warp::reply::json(&Response {
                message: format!("Output: {}\nError: {}", stdout, stderr),
            })
        });

    // Enable CORS
    let cors = warp::cors()
        .allow_any_origin() 
        .allow_methods(vec!["POST"]) 
        .allow_headers(vec!["Content-Type"]); 

    // Start the warp server with CORS
    warp::serve(send_faucet.with(cors))
    .tls()
        .cert_path("/etc/ssl/cert.pem")
        .key_path("/etc/ssl/cert.key")
        .run(([0, 0, 0, 0], 3030)) // 
        .await;
}
