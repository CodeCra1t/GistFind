use std::thread;
use sysinfo::System;

pub struct Config {
    pub scan_path: String,
    pub storage_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scan_path: "./test_docs".to_string(),
            storage_path: "./storage_data".to_string(),
        }
    }
}

const TANTIVY_MIN_MEMORY_PER_THREAD: usize = 15_000_000;
const TANTIVY_MAX_THREADS: usize = 8;

pub fn calculate_writer_config() -> (usize, usize) {
    let mut sys = System::new_all();
    sys.refresh_memory();

    let available_ram = sys.available_memory() as usize;
    let budget_total = (available_ram / 10).clamp(200_000_000, 2_000_000_000);
    let raw_threads = thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    let cpu_threads = raw_threads.saturating_sub(1).max(1);
    let max_threads_by_budget = budget_total / TANTIVY_MIN_MEMORY_PER_THREAD;
    let num_threads = TANTIVY_MAX_THREADS
        .min(cpu_threads)
        .min(max_threads_by_budget)
        .max(1);

    let memory_per_thread = budget_total / num_threads;
    println!("Total RAM that will be used: {} Mb", (budget_total / 1024) / 1024);
    println!("Total CPU threads that will be used: {}", cpu_threads);
    println!("Memory per thread: {} Mb", (memory_per_thread/ 1024) / 1024);
    (num_threads, memory_per_thread)
}

pub fn calculate_commit_threshold(memory_per_thread: usize) -> usize {
    let safety_margin = 15_000_000;
    let effective_memory = memory_per_thread.saturating_sub(safety_margin);
    (effective_memory / 3).max(1_000_000)
}