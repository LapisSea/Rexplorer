use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::thread;

use lazy_static::lazy_static;
use threadpool::ThreadPool;

lazy_static! {
	static ref COMMON_POOL:Mutex<ThreadPool> =Mutex::new(threadpool::Builder::new().thread_name("Worker".into()).build());
}

pub fn execute<F>(job: F)
	where F: FnOnce() + Send + 'static,
{
	let commonPool = COMMON_POOL.lock().unwrap();
	commonPool.execute(job);
}

