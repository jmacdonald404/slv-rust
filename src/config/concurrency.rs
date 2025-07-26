use std::sync::Arc;
use tokio::runtime::Runtime;
use rayon::ThreadPoolBuilder;
use crate::config::settings::PerformanceSettings;

/// Core concurrency management system implementing Data-Oriented Design principles
pub struct ConcurrencyManager {
    /// Job-based thread pool for CPU-intensive work (rayon)
    pub job_pool: rayon::ThreadPool,
    /// Async I/O runtime for network and disk operations (tokio)
    pub async_runtime: Arc<Runtime>,
    /// Current thread pool configuration
    pub pool_config: ThreadPoolConfig,
}

#[derive(Debug, Clone)]
pub struct ThreadPoolConfig {
    pub job_threads: usize,
    pub async_threads: usize,
    pub enable_work_stealing: bool,
    pub stack_size: Option<usize>,
}

impl Default for ThreadPoolConfig {
    fn default() -> Self {
        let num_cpus = num_cpus::get();
        Self {
            job_threads: num_cpus,
            async_threads: (num_cpus / 2).max(1),
            enable_work_stealing: true,
            stack_size: Some(2 * 1024 * 1024), // 2MB stack size
        }
    }
}

impl ThreadPoolConfig {
    /// Create thread pool configuration based on performance settings and hardware
    pub fn for_performance_profile(
        settings: &PerformanceSettings,
        cpu_cores: u32,
        total_memory_gb: u32,
    ) -> Self {
        let cpu_cores = cpu_cores as usize;
        
        match settings.profile {
            crate::config::PerformanceProfile::Low => Self {
                job_threads: (cpu_cores / 2).max(1), // Use fewer threads
                async_threads: 1, // Minimal async threads
                enable_work_stealing: false, // Reduce overhead
                stack_size: Some(1024 * 1024), // 1MB stack
            },
            crate::config::PerformanceProfile::Balanced => Self {
                job_threads: cpu_cores,
                async_threads: (cpu_cores / 2).max(1),
                enable_work_stealing: true,
                stack_size: Some(2 * 1024 * 1024), // 2MB stack
            },
            crate::config::PerformanceProfile::High => Self {
                job_threads: cpu_cores + (cpu_cores / 4), // Over-subscribe slightly
                async_threads: cpu_cores / 2,
                enable_work_stealing: true,
                stack_size: Some(4 * 1024 * 1024), // 4MB stack for complex operations
            },
            crate::config::PerformanceProfile::Custom => {
                // Use defaults for custom, user can override
                Self::default()
            }
        }
    }
}

impl ConcurrencyManager {
    /// Initialize the concurrency management system
    pub fn new(config: ThreadPoolConfig) -> anyhow::Result<Self> {
        tracing::info!("Initializing concurrency manager with config: {:?}", config);

        // Build rayon thread pool for CPU-intensive work
        let mut job_pool_builder = ThreadPoolBuilder::new()
            .num_threads(config.job_threads)
            .thread_name(|index| format!("slv-job-{}", index));

        if let Some(stack_size) = config.stack_size {
            job_pool_builder = job_pool_builder.stack_size(stack_size);
        }

        let job_pool = job_pool_builder
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create job thread pool: {}", e))?;

        // Build tokio runtime for async I/O operations
        let async_runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(config.async_threads)
            .thread_name("slv-async")
            .enable_all()
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create async runtime: {}", e))?;

        tracing::info!(
            "Created concurrency pools: {} job threads, {} async threads",
            config.job_threads,
            config.async_threads
        );

        Ok(Self {
            job_pool,
            async_runtime: Arc::new(async_runtime),
            pool_config: config,
        })
    }

    /// Execute CPU-intensive work on the job thread pool
    pub fn execute_job<F, R>(&self, job: F) -> R
    where
        F: FnOnce() -> R + Send,
        R: Send,
    {
        self.job_pool.install(job)
    }

    /// Execute parallel work using rayon's parallel iterators
    pub fn execute_parallel<I, F, R>(&self, iter: I, func: F) -> Vec<R>
    where
        I: rayon::prelude::IntoParallelIterator + Send,
        F: Fn(I::Item) -> R + Sync + Send,
        R: Send,
        I::Item: Send,
    {
        use rayon::prelude::*;
        self.job_pool.install(|| iter.into_par_iter().map(func).collect())
    }

    /// Spawn an async task on the I/O runtime
    pub fn spawn_async<F>(&self, future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.async_runtime.spawn(future)
    }

    /// Block on an async operation (use sparingly)
    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: std::future::Future,
    {
        self.async_runtime.block_on(future)
    }

    /// Get statistics about the thread pools
    pub fn get_stats(&self) -> ConcurrencyStats {
        ConcurrencyStats {
            job_threads: self.pool_config.job_threads,
            async_threads: self.pool_config.async_threads,
            current_jobs: self.job_pool.current_num_threads(),
            pending_jobs: 0, // Rayon doesn't expose this directly
        }
    }

    /// Update the concurrency configuration (requires restart of pools)
    pub fn update_config(&mut self, config: ThreadPoolConfig) -> anyhow::Result<()> {
        tracing::info!("Updating concurrency config: {:?}", config);
        
        // For now, we'd need to recreate the pools entirely
        // In a production system, you might want hot-reloading capabilities
        *self = Self::new(config)?;
        
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ConcurrencyStats {
    pub job_threads: usize,
    pub async_threads: usize,
    pub current_jobs: usize,
    pub pending_jobs: usize,
}

/// Global concurrency manager instance
static mut CONCURRENCY_MANAGER: Option<ConcurrencyManager> = None;
static CONCURRENCY_INIT: std::sync::Once = std::sync::Once::new();

/// Initialize the global concurrency manager
pub fn initialize_concurrency(config: ThreadPoolConfig) -> anyhow::Result<()> {
    CONCURRENCY_INIT.call_once(|| {
        let manager = ConcurrencyManager::new(config)
            .expect("Failed to initialize concurrency manager");
        
        unsafe {
            CONCURRENCY_MANAGER = Some(manager);
        }
    });
    
    Ok(())
}

/// Get reference to the global concurrency manager
pub fn get_concurrency_manager() -> Option<&'static ConcurrencyManager> {
    unsafe { CONCURRENCY_MANAGER.as_ref() }
}

/// Utility functions for common concurrency patterns in DOD
pub mod dod_utils {
    use super::*;
    use rayon::prelude::*;

    /// Process a collection of components in parallel using Data-Oriented Design
    pub fn process_components_parallel<T, F, R>(
        components: &[T],
        processor: F,
    ) -> Vec<R>
    where
        T: Sync,
        F: Fn(&T) -> R + Sync + Send,
        R: Send,
    {
        if let Some(manager) = get_concurrency_manager() {
            manager.execute_parallel(components, processor)
        } else {
            // Fallback to sequential processing if no manager
            components.iter().map(processor).collect()
        }
    }

    /// Process multiple arrays of the same length in parallel (typical DOD pattern)
    pub fn process_multiple_arrays<T1, T2, F, R>(
        array1: &[T1],
        array2: &[T2],
        processor: F,
    ) -> Vec<R>
    where
        T1: Sync,
        T2: Sync,
        F: Fn(&T1, &T2) -> R + Sync,
        R: Send,
    {
        if let Some(manager) = get_concurrency_manager() {
            use rayon::prelude::*;
            manager.execute_parallel(
                array1.par_iter().zip(array2.par_iter()),
                |(a, b)| processor(a, b),
            )
        } else {
            array1.iter().zip(array2.iter())
                .map(|(a, b)| processor(a, b))
                .collect()
        }
    }

    /// Execute a batch of jobs with work-stealing
    pub fn execute_job_batch<F, R>(jobs: Vec<F>) -> Vec<R>
    where
        F: FnOnce() -> R + Send,
        R: Send,
    {
        if let Some(manager) = get_concurrency_manager() {
            manager.execute_parallel(jobs, |job| job())
        } else {
            jobs.into_iter().map(|job| job()).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concurrency_manager_creation() {
        let config = ThreadPoolConfig::default();
        let manager = ConcurrencyManager::new(config);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_job_execution() {
        let config = ThreadPoolConfig {
            job_threads: 2,
            async_threads: 1,
            enable_work_stealing: true,
            stack_size: None,
        };
        
        let manager = ConcurrencyManager::new(config).unwrap();
        let result = manager.execute_job(|| 42);
        assert_eq!(result, 42);
    }

    #[test]
    fn test_parallel_processing() {
        let config = ThreadPoolConfig::default();
        let manager = ConcurrencyManager::new(config).unwrap();
        
        let data = vec![1, 2, 3, 4, 5];
        let results = manager.execute_parallel(&data, |&x| x * 2);
        
        assert_eq!(results, vec![2, 4, 6, 8, 10]);
    }

    #[tokio::test]
    async fn test_async_execution() {
        let config = ThreadPoolConfig::default();
        let manager = ConcurrencyManager::new(config).unwrap();
        
        let handle = manager.spawn_async(async {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            42
        });
        
        let result = handle.await.unwrap();
        assert_eq!(result, 42);
    }
}