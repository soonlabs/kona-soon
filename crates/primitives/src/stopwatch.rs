use std::time::{Duration, Instant};

/// StopWatch is a utility struct for measuring and recording elapsed time at various points.
pub struct StopWatch {
    /// The name of this StopWatch instance
    name: String,
    /// The time when this StopWatch was created or reset
    start_time: Instant,
    /// A vector of tuples containing the elapsed time and a note for each click
    clicks: Vec<(Duration, String)>,
}

/// guard allow register some defer functions.
pub struct StopWatchGuard<F>
where
    F: FnOnce(&StopWatch),
{
    stopwatch: StopWatch,
    callback: Option<F>,
}

impl<F> Drop for StopWatchGuard<F>
where
    F: FnOnce(&StopWatch),
{
    fn drop(&mut self) {
        if let Some(callback) = self.callback.take() {
            callback(&self.stopwatch);
        }
    }
}

impl<F> std::ops::Deref for StopWatchGuard<F>
where
    F: FnOnce(&StopWatch),
{
    type Target = StopWatch;

    fn deref(&self) -> &Self::Target {
        &self.stopwatch
    }
}

impl<F> std::ops::DerefMut for StopWatchGuard<F>
where
    F: FnOnce(&StopWatch),
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stopwatch
    }
}

impl StopWatch {
    /// Creates a new StopWatch with a given name
    pub fn new(name: impl Into<String>) -> Self {
        StopWatch { name: name.into(), start_time: Instant::now(), clicks: Vec::new() }
    }

    pub fn with_guard<F>(name: impl Into<String>, callback: F) -> StopWatchGuard<F>
    where
        F: FnOnce(&StopWatch),
    {
        StopWatchGuard { stopwatch: Self::new(name), callback: Some(callback) }
    }

    /// Records a new click with the given note and the current elapsed time
    pub fn click(&mut self, note: impl Into<String>) {
        let elapsed = self.start_time.elapsed();
        self.clicks.push((elapsed, note.into()));
    }

    /// Generates a summary of all recorded clicks
    pub fn summary(&self) -> String {
        let mut result = String::new();
        result.push_str(&format!("{} Summary:", self.name));

        if self.clicks.is_empty() {
            result.push_str("No clicks recorded.\n");
        } else {
            let mut last_time = Duration::from_secs(0);
            for (time, note) in self.clicks.iter() {
                let duration = *time - last_time;
                result.push_str(&format!("[{}]: {:?};", note, duration));
                last_time = *time;
            }
            result.push_str(&format!("Total time: {:?}", self.clicks.last().unwrap().0));
        }

        result
    }

    pub fn is_empty(&self) -> bool {
        self.clicks.is_empty()
    }
}
