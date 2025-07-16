// main.rs
use actix_web::{web, App, HttpServer, Responder, HttpResponse};
use serde::{Deserialize, Serialize};
use reqwest::{Client, Proxy};
use std::time::Duration;
use std::env; // Import for environment variables

// Define the structure for the incoming POST request
#[derive(Deserialize)]
struct ScrapeRequest {
    url: String,
    // Optional SOCKS5 proxy address in the request body.
    // This will be ignored if the DEFAULT_SOCKS5_PROXY env var is set for the service.
    proxy: Option<String>,
    // Optional timeout in seconds for the request
    timeout_seconds: Option<u64>,
}

// Define the structure for the outgoing JSON response
#[derive(Serialize)]
struct ScrapeResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Handles the POST request to scrape a URL.
///
/// This function takes a `ScrapeRequest` as input, constructs an HTTP client.
/// It prioritizes a SOCKS5 proxy address from the `DEFAULT_SOCKS5_PROXY`
/// environment variable. If that's not set, it falls back to the 'proxy' field
/// in the request body. If neither is set, no proxy is used.
/// It then performs a GET request to the specified URL and returns the scraped
/// content or an error message.
async fn scrape_handler(req: web::Json<ScrapeRequest>) -> impl Responder {
    // Create a new HTTP client builder
    let mut client_builder = Client::builder();

    // Set a default timeout if none is provided, or use the user-specified one
    let timeout = req.timeout_seconds.unwrap_or(30); // Default to 30 seconds
    client_builder = client_builder.timeout(Duration::from_secs(timeout));

    // Determine the proxy address to use:
    // 1. Check for DEFAULT_SOCKS5_PROXY environment variable (highest precedence).
    //    This is how Kubernetes will inject the specific Tor proxy for each service.
    // 2. Fallback to 'proxy' field in the request body (if no default env var is set).
    let proxy_to_use = env::var("DEFAULT_SOCKS5_PROXY").ok().or_else(|| req.proxy.clone());

    if let Some(proxy_addr) = proxy_to_use {
        match Proxy::all(&proxy_addr) {
            Ok(proxy) => {
                client_builder = client_builder.proxy(proxy);
                println!("Using proxy: {}", proxy_addr); // Log proxy usage
            },
            Err(e) => {
                // If proxy parsing fails, return an error response
                eprintln!("Failed to parse proxy URL '{}': {}", proxy_addr, e);
                return HttpResponse::BadRequest().json(ScrapeResponse {
                    content: None,
                    error: Some(format!("Invalid proxy URL: {}", proxy_addr)),
                });
            }
        }
    } else {
        println!("No proxy configured for this request.");
    }

    // Build the HTTP client
    let client = match client_builder.build() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to build HTTP client: {}", e);
            return HttpResponse::InternalServerError().json(ScrapeResponse {
                content: None,
                error: Some(format!("Failed to initialize HTTP client: {}", e)),
            });
        }
    };

    println!("Attempting to scrape URL: {}", req.url); // Log the URL being scraped

    // Perform the GET request
    match client.get(&req.url).send().await {
        Ok(response) => {
            // Check if the response status is successful (2xx)
            if response.status().is_success() {
                match response.text().await {
                    Ok(text) => {
                        println!("Successfully scraped URL: {}", req.url);
                        HttpResponse::Ok().json(ScrapeResponse {
                            content: Some(text),
                            error: None,
                        })
                    }
                    Err(e) => {
                        eprintln!("Failed to read response body for {}: {}", req.url, e);
                        HttpResponse::InternalServerError().json(ScrapeResponse {
                            content: None,
                            error: Some(format!("Failed to read response body: {}", e)),
                        })
                    }
                }
            } else {
                let status = response.status();
                let status_text = response.status().canonical_reason().unwrap_or("Unknown Status");
                eprintln!("Failed to scrape URL {}: Status {} {}", req.url, status, status_text);
                HttpResponse::build(status).json(ScrapeResponse {
                    content: None,
                    error: Some(format!("HTTP request failed with status: {} {}", status, status_text)),
                })
            }
        }
        Err(e) => {
            eprintln!("Request to {} failed: {}", req.url, e);
            HttpResponse::InternalServerError().json(ScrapeResponse {
                content: None,
                error: Some(format!("Failed to make HTTP request: {}", e)),
            })
        }
    }
}

/// Main function to set up and run the Actix-Web server.
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging (optional, but good for debugging)
    // env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Define the address and port to bind to
    // This makes it accessible from outside the container in a Kubernetes environment
    let host = "0.0.0.0";
    let port = 8282; // Consistent with the Containerfile

    println!("Starting server on http://{}:{}", host, port);

    // Start the HTTP server
    HttpServer::new(|| {
        App::new()
            // Register the POST route for scraping
            .service(
                web::resource("/scrape")
                    .route(web::post().to(scrape_handler))
            )
    })
    .bind(format!("{}:{}", host, port))? // Bind to the specified host and port
    .run() // Run the server
    .await
}

