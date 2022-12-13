use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
};

/*
 * A job is the function that will be executed by the worker
 * FnOnce means that the function can only be called once
 * Send is required because the function will be sent to another thread
 * 'static means that the function can live for the entire duration of the program instead of just the duration of the function when spawned
 */
type Job = Box<dyn FnOnce() + Send + 'static>;

// this is the threadpool struct that will manage the workers
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
        // ensure that the size is greater than 0, as we cannot have a threadpool with 0 workers
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
        // im not compleatly sure why we need to box the function, but i believe it has to do with the fact that the function is unknown at compile time
        let job = Box::new(f);

        // finally, we will send the job to the workers using the sender
        // the workers will then receive the job and execute it
        // the sender is an option, and if it is none, we will do nothing
        // this helps us to gracefully shutdown the threadpool and prevent any more jobs from being sent to the workers
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
// it will have an id and a thread handle
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

            // we have a message, aka a job. (hopefully)
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
                    // if we receive an error, lets shutdown the thread by breaking out of the loop
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
