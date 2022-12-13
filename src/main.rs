use std::{
    fs,
    io::prelude::*,
    net::{TcpListener, TcpStream},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};
fn main() {
    // tcp listener will listen to incoming requests
    // in this case, we are binding or listening to port 7878 on localhost
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

    // lets make this multithreaded :D
    // because we do not want to make an unlimited number of threads, we will use a thread pool
    // threadpool is implemenented below

    // this is nice because we can limit the number of threads (or workers) that are created
    // the thread pool will have a number of resuable threads that will handle incoming requests

    // in this case, we are creating a thread pool with 5 threads
    let pool = ThreadPool::new(5);

    // using the tcp listener, we will loop through each incoming request
    // we want to assign each request to a thread in the thread pool
    // we will use the threadpool's execute method to do this
    for stream in listener.incoming() {
        // when we get a request, it is of type result, we must unwrap it
        let stream = stream.unwrap();

        // finally, we will execute the request
        // using the threadpool's execute method, we will pass in an anonymous function
        // this function will call the handle_connection function, that will handle the request
        pool.execute(|| {
            handle_connection(stream);
        });
    }

    println!("Shutting down.");
}

fn handle_connection(mut stream: TcpStream) {
    // we can read the request from the stream into a buffer we create
    let mut buffer = [0; 1024];

    // I think this can return an error, so we should handle it properly later
    stream.read(&mut buffer).unwrap();

    // we can now create a few responses to expect from a client
    // we will use these to compare them to an incoming request

    // notice that we are using byte strings, this is because we are comparing the request to the byte string
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
    // note again that this can return an error, so we should handle it properly later
    stream.write_all(response.as_bytes()).unwrap();
    stream.flush().unwrap();
}

/**
 * A job is the function that will be executed by the worker
 * FnOnce means that the function can only be called once
 * Send is required because the function will be sent to another thread
 * 'static means that the function can live for the entire duration of the program instead of just the duration of the function
 */
type Job = Box<dyn FnOnce() + Send + 'static>;
pub struct ThreadPool {
    // an array of workers or threads that will handle the requests
    workers: Vec<Worker>,

    // the sender that will be used to send jobs to the workers
    sender: Option<mpsc::Sender<Job>>,
}

impl ThreadPool {
    // this function will initialize a new thread pool
    // it will take the number of workers to spawn and initialize them
    pub fn new(size: usize) -> ThreadPool {
        // ensure that the size is greater than 0
        assert!(size > 0);

        // using mpsc, we will create a channel that will send jobs to the workers
        // the channel will have a sender and a receiver
        // the reciever will be shared among the workers and the sender will be used by the threadpool
        let (sender, receiver) = mpsc::channel();

        // because the same receiver will be shared among the workers, we will need to wrap it in an Arc and Mutex
        // the arc will allow multiple threads to own the receiver
        // the mutex will ensure that only one thread can access the receiver at a time
        // mutual exclusion!
        let receiver = Arc::new(Mutex::new(receiver));

        // initialize the workers array
        let mut workers = Vec::with_capacity(size);

        // we can now make the workers, we will use the Worker::new function to do this
        // we will past the id of the worker from the range
        // and also clone the receiver so that each worker has its own copy of the receiver thanks to arc
        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        // finally, we can return the threadpool
        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }

    // execute will take an anonymous function and send it to the workers
    // the workers will then execute the function
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        // we will create a job from the function
        let job = Box::new(f);

        // finally, we will send the job to the workers using the sender
        // the workers will then receive the job and execute it
        // the sender is an option, and if it is none, we will do nothing
        // this helps us to gracefully shutdown the threadpool
        match self.sender.as_ref() {
            Some(sender) => {
                sender.send(job).unwrap();
            }
            None => {
                println!("There is no sender!");
            }
        }
    }
}

// to gracefully shutdown the threadpool,
// lets implement the drop trait
impl Drop for ThreadPool {
    // the drop function will kill the workers
    fn drop(&mut self) {
        // first, we will make the sender option to none
        // this will prevent any more jobs from being sent to the workers
        drop(self.sender.take());

        // we can now go to each worker and join the thread
        // join will join the thread to the main thread, essentially terminating the thread
        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

// this is the worker struct, aka the thread
// it will have an id and a thread handle (just a thread)
struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    // this will initialize a new worker
    // called from the threadpool when it is initialized
    // it takes the id of the worker and the receiver that will be used to receive jobs
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        // first we must spawn the thread itself, we will pass an anonymous function to the thread
        // move is used to pass the environment to the thread
        // loop will be used to infinitely receive jobs
        let thread = thread::spawn(move || loop {
            // using the receiver, we will attempt to receive a job
            // we use the lock function to acquire the mutex, this will block the thread until its available
            // then we use the recv function to receive the job, which will further block the thread until a job is available
            let message = receiver.lock().unwrap().recv();

            // the message will be a result, so we will match on it
            match message {
                Ok(job) => {
                    println!("Worker {id} got a job; executing.");

                    // at this point, the thread has waited its turn to aquire the mutex and has received a job, so we can execute it!
                    // it can be any function, but in the case of this program, it will call the handle_connection function
                    job();
                }
                Err(_) => {
                    println!("Worker {id} disconnected; shutting down.");
                    // if we receive an error, lets shutdown the thread
                    break;
                }
            }
        });

        Worker {
            id,
            thread: Some(thread),
        }
    }
}
