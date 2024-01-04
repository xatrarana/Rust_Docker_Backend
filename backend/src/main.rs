use postgres::{Client, NoTls};
use postgres::Error as PostgresError;
use std::fmt::format;
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};


// macros
#[macro_use]
extern crate serde_derive;

// Model: User struct with id, name and email
#[derive(Serialize, Deserialize)]
struct User {
    id: Option<i32>, // Option means optional
    name: String,
    email: String,
}

// Database URL
const DB_URL: &str = "postgres://postgres:5358@localhost:5432/mydb";

// constants
const OK_RESPONSES: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, PUT, DELETE\r\nAccess-Control-Allow-Headers: Content-Type\r\n\r\n";

const NOT_FOUND_RESPONSES: &str = "HTTP/1.1 404 NOT FOUND\r\nContent-Type: application/json\r\n\r\n";

const BAD_REQUEST_RESPONSES: &str = "HTTP/1.1 400 BAD REQUEST\r\nContent-Type: application/json\r\n\r\n";

const INTERNAL_SERVER_ERROR_RESPONSES: &str = "HTTP/1.1 500 INTERNAL SERVER ERROR\r\nContent-Type: application/json\r\n\r\n";

// main function

fn main(){
    // set Database
    if let Err(_) = set_database(){
        println!("Error setting up database");
        return;
    }

    // set up server and listen to port 8080
    let listner = TcpListener::bind(format!("0.0.0.0:8080")).unwrap();
    println!("Server listening on port 8080");

    for stream in listner.incoming(){
        match stream {
            Ok(stream) => {
                handle_client(stream);
            }
            Err(e) => {
                println!("Unable to connect: {}", e);
            }
        }
    }
}

// set up database utility function
fn set_database() -> Result<(), PostgresError>{
    let mut client = Client::connect(DB_URL, NoTls)?;
    client.batch_execute(
        "
        CREATE TABLE IF NOT EXISTS users (
            id SERIAL PRIMARY KEY,
            name VARCHAR NOT NULL,
            email VARCHAR NOT NULL
        )
    ")?;
    Ok(())
}


// Get id form the URL
fn get_id(request: &str) -> &str {
    request.split("/").nth(4).unwrap_or_default().split_whitespace().next().unwrap_or_default()
}

// Deserialize user from the request body without id
fn get_user_request_body(request: &str) -> Result<User, serde_json::Error>{
    serde_json::from_str(request.split("\r\n\r\n").last().unwrap_or_default())
}

// handle requests
fn handle_client(mut stream: TcpStream){
    let mut buffer = [0; 1024];
    let mut request = String::new();

    match stream.read(&mut buffer) {
        Ok(size) => {
            request.push_str(String::from_utf8_lossy(&buffer[..size]).as_ref());

            let (status_line, content) = match &*request {
                r if r.starts_with("OPTIONS") => (OK_RESPONSES.to_string(),"".to_string()) ,
                r if r.starts_with("POST /api/rust/users") => handle_post_request(r),
                r if r.starts_with("GET /api/rust/users") => handle_get_request(r),
                r if r.starts_with("GET /api/rust/users") => handle_get_all_request(r),
                r if r.starts_with("PUT /api/rust/users") => handle_put_request(r),
                r if r.starts_with("DELETE /api/rust/users") => handle_delete_request(r),
                _ => (NOT_FOUND_RESPONSES.to_string(), "404 not found".to_string()),
            };

            stream.write_all(format!("{}{}", status_line, content).as_bytes()).unwrap();

        }
        Err(e) => {
            eprintln!("Unable to read stream: {}", e);
        }

        
    }
}

// handle post request
fn handle_post_request(request: &str) -> (String, String){
    match (get_user_request_body(request), Client::connect(DB_URL, NoTls)) {
        (Ok(user), Ok(mut client)) => {
            // Insert user into database and return the id
            let row = client.query_one(
                "
                INSERT INTO users (name, email) VALUES ($1, $2) RETURNING id
                ", &[&user.name, &user.email]).unwrap();
            let user_id: i32 = row.get(0);

            // Fetch the created user data

            match client.query_one("
            SELECT id, name, email FROM users WHERE id = $1
            ", &[&user_id]){
                Ok(row) => {
                    let user = User {
                        id: row.get(0),
                        name: row.get(1),
                        email: row.get(2),
                    };
                    (OK_RESPONSES.to_string(), serde_json::to_string(&user).unwrap()) 
                }
                Err(_) => {
                    (INTERNAL_SERVER_ERROR_RESPONSES.to_string(), "500 internal server error".to_string())
                }
            }
        }
        _ => {
            (BAD_REQUEST_RESPONSES.to_string(), "400 bad request".to_string())
        }
    }
}

fn handle_get_request(request: &str) -> (String, String){
    match (get_id(&request).parse::<i32>(), Client::connect(DB_URL, NoTls)) {
        (Ok(id), Ok(mut client)) => {
            match client.query_one("
            SELECT * FROM users WHERE id = $1"
            , &[&id]) {
                Ok(row) => {
                    let user = User {
                        id: row.get(0),
                        name: row.get(1),
                        email: row.get(2),
                    };
                    (OK_RESPONSES.to_string(), serde_json::to_string(&user).unwrap())
                    
                }
                Err(_) => {
                    (NOT_FOUND_RESPONSES.to_string(), "user not found".to_string())
                }
            }
        }
        _ => {
            (INTERNAL_SERVER_ERROR_RESPONSES.to_string(), "500 internal server error".to_string())
        }
    }
}

// handle all the get users request
fn handle_get_all_request(_request: &str) -> (String, String){
    match Client::connect(DB_URL, NoTls) {
        Ok(mut client) => {
            let mut users = Vec::new();
            for row in client.query("SELECT * FROM users", &[]).unwrap(){
                let user = User {
                    id: row.get(0),
                    name: row.get(1),
                    email: row.get(2),
                };
                users.push(user);
            }
            (OK_RESPONSES.to_string(), serde_json::to_string(&users).unwrap())
        }
        _ =>  (INTERNAL_SERVER_ERROR_RESPONSES.to_string(), "500 internal server error".to_string())
        
        
    }
}

// handle put request
fn handle_put_request(request: &str) -> (String, String){
    match 
    (
        get_id(&request).parse::<i32>(),
        get_user_request_body(&request),
        Client::connect(DB_URL, NoTls)
    )
    {
        (Ok(id),Ok(user), Ok(mut client)) => {
        client
        .execute(
                "UPDATE users SET name = $1, email = $2 WHERE id = $3",
                &[&user.name, &user.email, &id]
        )
        .unwrap();

            (OK_RESPONSES.to_string(), "user updated sucessfully".to_string())
        }

        _ => (INTERNAL_SERVER_ERROR_RESPONSES.to_string(), "500 internal server error".to_string())
    }
}

// handle delete request
fn handle_delete_request(request: &str) -> (String,String){
    match (get_id(&request).parse::<i32>(),Client::connect(DB_URL, NoTls)) {
        (Ok(id), Ok(mut client)) => {
        let row_affected = client.execute("DELETE FROM users WHERE id $1",&[&id]).unwrap();
            if row_affected == 0 {
                return (NOT_FOUND_RESPONSES.to_string(), "user not found".to_string());
            }
        (OK_RESPONSES.to_string(), "user deleted sucessfully".to_string())
        }
        _ => (INTERNAL_SERVER_ERROR_RESPONSES.to_string(), "500 internal server error".to_string())
    }
}
