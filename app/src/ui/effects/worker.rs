//! The disk worker: carries out the core's [`Disk`] effects in order on a thread of its own, so
//! the event loop never waits on the disk (N-UX-01), and sends their results back as messages.

use std::{
    path::Path,
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
};

use super::{Backups, atomic};
use crate::{
    app::{Disk, Message},
    ui::Input,
};

/// The queue of disk work and the thread that does it. Dropping it waits for the queued work.
#[derive(Debug)]
pub struct Io {
    /// `None` only while dropping, to close the queue before waiting for the thread.
    jobs: Option<Sender<Job>>,
    worker: Option<JoinHandle<()>>,
}

#[derive(Debug)]
enum Job {
    Disk(Disk),
    /// Answers once every job before it is done and its result sent.
    Flush(Sender<()>),
}

impl Io {
    /// Starts the worker, which sends each result to `results` as an [`Input::Core`] message and
    /// makes backups with `backups`.
    #[must_use]
    pub fn start(results: Sender<Input>, backups: Backups) -> Self {
        let (jobs, queue) = mpsc::channel();
        let worker = thread::spawn(move || work(&queue, &results, backups));
        Self {
            jobs: Some(jobs),
            worker: Some(worker),
        }
    }

    /// Queues `disk` behind the work queued before it.
    pub fn submit(&self, disk: Disk) {
        self.send(Job::Disk(disk));
    }

    /// Waits until the work queued so far is done and its results are sent.
    ///
    /// # Panics
    ///
    /// Never: the worker runs, answering every flush, until the queue closes on dropping.
    pub fn flush(&self) {
        let (done, finished) = mpsc::channel();
        self.send(Job::Flush(done));
        finished
            .recv()
            .expect("the worker answers every flush before it stops");
    }

    fn send(&self, job: Job) {
        self.jobs
            .as_ref()
            .and_then(|jobs| jobs.send(job).ok())
            .expect("the worker runs until the queue closes, which only dropping does");
    }
}

impl Drop for Io {
    fn drop(&mut self) {
        drop(self.jobs.take());
        if let Some(worker) = self.worker.take() {
            // The worker never panics; if it did, the panic was reported on its own thread and
            // there is nothing left to do here.
            let _ = worker.join();
        }
    }
}

/// Runs the jobs from `queue` until it closes.
fn work(queue: &Receiver<Job>, results: &Sender<Input>, mut backups: Backups) {
    for job in queue {
        let message = match job {
            Job::Disk(Disk::Save { path, text }) => Some(save(&path, &text)),
            Job::Disk(Disk::Backup { path }) => {
                backups
                    .ensure(&path)
                    .err()
                    .map(|error| Message::BackupFailed {
                        reason: error.to_string(),
                    })
            }
            Job::Flush(done) => {
                // The flusher waits for this answer and only stops waiting by being dropped.
                let _ = done.send(());
                None
            }
        };
        if let Some(message) = message {
            // A closed channel means the event loop has ended, and nobody is left to tell.
            let _ = results.send(Input::Core(message));
        }
    }
}

fn save(path: &Path, text: &str) -> Message {
    match atomic::write(path, text.as_bytes()) {
        Ok(()) => Message::Saved,
        Err(error) => Message::SaveFailed {
            reason: error.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use jiff::civil::date;

    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// A worker with backups in `dir/backups`, and the channel its results arrive on.
    fn start(dir: &Path) -> (Io, Receiver<Input>) {
        let (results, inputs) = mpsc::channel();
        let backups = Backups::new(Some(dir.join("backups")), || date(2026, 10, 5));
        (Io::start(results, backups), inputs)
    }

    fn results(inputs: &Receiver<Input>) -> Vec<Input> {
        inputs.try_iter().collect()
    }

    fn save_job(path: &Path, text: &str) -> Disk {
        Disk::Save {
            path: path.to_owned(),
            text: text.to_owned(),
        }
    }

    // F-FILE-04, N-UX-01
    #[test]
    fn saves_in_order_and_reports_each_result() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (io, inputs) = start(dir.path());
        let path = dir.path().join("todo.taskpaper");
        io.submit(save_job(&path, "one"));
        io.submit(save_job(&dir.path().join("missing/x.taskpaper"), "x"));
        io.submit(save_job(&path, "two"));
        io.flush();
        let results = results(&inputs);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], Input::Core(Message::Saved));
        assert!(
            matches!(&results[1], Input::Core(Message::SaveFailed { reason }) if reason.contains("No such file")),
            "{results:?}"
        );
        assert_eq!(results[2], Input::Core(Message::Saved));
        assert_eq!(fs::read_to_string(&path)?, "two");
        Ok(())
    }

    // FR-DOC-07, F-FILE-09
    #[test]
    fn backs_up_before_a_save_queued_after_it() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (io, inputs) = start(dir.path());
        let path = dir.path().join("todo.taskpaper");
        fs::write(&path, "on disk")?;
        io.submit(Disk::Backup { path: path.clone() });
        io.submit(save_job(&path, "saved"));
        drop(io);
        assert_eq!(results(&inputs), [Input::Core(Message::Saved)]);
        let backups = dir.path().join("backups");
        let folder: PathBuf = fs::read_dir(&backups)?.next().unwrap()?.path();
        assert_eq!(
            fs::read_to_string(folder.join("2026-10-05.taskpaper"))?,
            "on disk"
        );
        Ok(())
    }

    #[test]
    fn reports_a_failed_backup() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (io, inputs) = start(dir.path());
        io.submit(Disk::Backup {
            path: dir.path().to_owned(),
        });
        io.flush();
        assert!(matches!(
            results(&inputs).as_slice(),
            [Input::Core(Message::BackupFailed { reason })] if reason.contains("directory")
        ));
        Ok(())
    }

    #[test]
    fn results_after_the_event_loop_ended_are_dropped() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (io, inputs) = start(dir.path());
        drop(inputs);
        let path = dir.path().join("todo.taskpaper");
        io.submit(save_job(&path, "kept"));
        io.flush();
        assert_eq!(fs::read_to_string(&path)?, "kept");
        Ok(())
    }
}
