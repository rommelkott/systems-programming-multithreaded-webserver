use std::{
    fs,
    io::prelude::*,
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

// import the threadpool that we created
use server::ThreadPool;

fn main() {
    // the listerner is of type TcpListener, and calls the bind function.
    // this bind function will listen to the address and port we pass in
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

    // webservers are better when they are multithreaded, as that means they can handle multiple requests at once
    // but some management is needed. we want to make sure that we do not create an unlimited number of threads.
    // this is important to prevent the server from running out of resources
    // so we will construct a thread pool to manage the threads
    // this is nice because we can limit the number of threads (or workers) that are created

    // in this case, we are creating a thread pool with 5 threads
    let pool = ThreadPool::new(5);

    // using the tcp listener, we will loop through each incoming request
    // we want to assign each request to a thread in the thread pool
    for stream in listener.incoming() {
        // when we get a request, it is of type result, we must unwrap it
        let stream = stream.unwrap();

        // finally, we will respond to the request
        // using the threadpool's execute method, we will pass in an anonymous function that will execute the handle_connection function
        // the execute function will pass our anonymous function to the threadpool, which will then send the job to an available worker.
        // this worker will then execute the anonymous function, which will then execute the handle_connection function
        // aka. a worker will execute the handle_connection function
        pool.execute(|| {
            handle_connection(stream);
        });
    }

    println!("Shutting down.");
}

// handle a connection to the server
fn handle_connection(mut stream: TcpStream) {
    // we can read the request from the stream into a buffer we create
    let mut buffer = [0; 1024];

    // read the request into the buffer
    stream.read(&mut buffer).unwrap();

    // we can now create a few responses to expect from a client
    // we will use these to compare them to an incoming request

    // notice that we are using byte strings, this is because the request is a byte string
    // we can create a byte string by using the b"" syntax

    // GET /
    let get = b"GET / HTTP/1.1\r\n";
    // GET /sleep
    let sleep = b"GET /sleep HTTP/1.1\r\n";

    // we can now compare the request to the expected responses
    // if the request matches one of the expected responses, we will get the corresponding filename and status
    // if the request does not match any of the expected responses, we will get 404.html and a 404 status
    let (status_line, filename) = if buffer.starts_with(get) {
        ("HTTP/1.1 200 OK", "hello.html")
    } else if buffer.starts_with(sleep) {
        // this will sleep the thread for 20 seconds, which will simulate a slow request
        thread::sleep(Duration::from_secs(20));
        ("HTTP/1.1 200 OK", "hello.html")
    } else {
        ("HTTP/1.1 404 NOT FOUND", "404.html")
    };

    // we can now read the contents of the file into a string
    let contents = fs::read_to_string(filename).unwrap();

    // given the status and contents, we can now create a response
    let response = format!(
        "{}\r\nContent-Length: {}\r\n\r\n{}",
        status_line,
        contents.len(),
        contents
    );

    // finally, we can write the response to the stream
    stream.write_all(response.as_bytes()).unwrap();
    stream.flush().unwrap();
}
