//! Opt-in evidence of the actual text returned by every event reader in a
//! decision. This records reads, not host execution. The default path writes
//! nothing. Normal log growth is stored once, as parent-linked byte deltas;
//! replacement/truncation starts a new root without losing older snapshots.
use std::cell::RefCell;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use serde::Serialize;

thread_local! {
    static ACTIVE: RefCell<Option<Rc<RefCell<State>>>> = const { RefCell::new(None) };
}

#[derive(Serialize, PartialEq)]
struct ReadRef {
    #[serde(skip_serializing_if = "Option::is_none")]
    snapshot_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    count: u64,
}

struct State {
    directory: PathBuf,
    source: PathBuf,
    archive: File,
    index: File,
    offset: u64,
    next_id: u64,
    previous: Option<(u64, Rc<String>)>,
    reads: Vec<ReadRef>,
    failure: Option<String>,
}

/// Owns a new, exclusive capture directory. Never overwrites existing evidence.
pub struct InputCapture(Rc<RefCell<State>>);

/// An active decision's capture. Dropping it also detaches the reader hook.
pub struct RequestCapture(Rc<RefCell<State>>);

impl InputCapture {
    pub fn create(directory: &Path, source: &Path) -> io::Result<Self> {
        let absolute = |path: &Path| -> io::Result<PathBuf> {
            Ok(if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir()?.join(path)
            })
        };
        let directory = absolute(directory)?;
        std::fs::create_dir(&directory)?;
        let open = |name| {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(directory.join(name))
        };
        Ok(Self(Rc::new(RefCell::new(State {
            archive: open("bytes.bin")?,
            index: open("snapshots.jsonl")?,
            directory,
            source: absolute(source)?,
            offset: 0,
            next_id: 0,
            previous: None,
            reads: Vec::new(),
            failure: None,
        }))))
    }

    pub fn begin(&self) -> io::Result<RequestCapture> {
        ACTIVE.with(|active| {
            let mut slot = active.borrow_mut();
            if slot.is_some() {
                return Err(io::Error::other(
                    "an input capture is already active on this thread",
                ));
            }
            self.0.borrow_mut().reads.clear();
            *slot = Some(self.0.clone());
            Ok(RequestCapture(self.0.clone()))
        })
    }
}

impl RequestCapture {
    /// Flush before publishing the decision. Failed capture is explicit and
    /// does not turn a valid gameplay decision into an empty order list.
    pub fn finish(self) -> serde_json::Value {
        let mut state = self.0.borrow_mut();
        if state.failure.is_none() {
            if let Err(error) = state
                .archive
                .sync_data()
                .and_then(|_| state.index.sync_data())
            {
                state.failure = Some(error.to_string());
            }
        }
        serde_json::json!({
            "schema": 1,
            "directory": state.directory,
            "source": state.source,
            "archive": "bytes.bin",
            "index": "snapshots.jsonl",
            "complete": state.failure.is_none(),
            "error": state.failure,
            "reads": state.reads,
        })
    }
}

impl Drop for RequestCapture {
    fn drop(&mut self) {
        ACTIVE.with(|active| {
            let mut slot = active.borrow_mut();
            if slot
                .as_ref()
                .is_some_and(|state| Rc::ptr_eq(state, &self.0))
            {
                *slot = None;
            }
        });
    }
}

pub(super) fn observe(path: &Path, result: &io::Result<Rc<String>>) {
    ACTIVE.with(|active| {
        let slot = active.borrow();
        let Some(shared) = slot.as_ref() else { return };
        let mut state = shared.borrow_mut();
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd.join(path)
        } else {
            state.failure = Some("cannot resolve input path".to_string());
            return;
        };
        // A request can consult several logs: do not pretend those reads were
        // captured by this single-source archive.
        if path != state.source {
            state.failure = Some(format!("unexpected input source: {}", path.display()));
            return;
        }
        if state.failure.is_some() {
            return;
        }
        let read = match result {
            Ok(text) => match state.snapshot(text) {
                Ok(id) => ReadRef {
                    snapshot_id: Some(id),
                    error: None,
                    count: 1,
                },
                Err(error) => {
                    state.failure = Some(error.to_string());
                    return;
                }
            },
            Err(error) => ReadRef {
                snapshot_id: None,
                error: Some(error.to_string()),
                count: 1,
            },
        };
        if let Some(last) = state
            .reads
            .last_mut()
            .filter(|last| last.snapshot_id == read.snapshot_id && last.error == read.error)
        {
            last.count += 1;
        } else {
            state.reads.push(read);
        }
    });
}

impl State {
    fn snapshot(&mut self, text: &Rc<String>) -> io::Result<u64> {
        if let Some((id, previous)) = &self.previous {
            if Rc::ptr_eq(previous, text) || previous.as_str() == text.as_str() {
                return Ok(*id);
            }
        }
        let (parent, start) = self
            .previous
            .as_ref()
            .filter(|(_, old)| text.starts_with(old.as_str()))
            .map_or((None, 0), |(id, old)| (Some(*id), old.len()));
        let bytes = &text.as_bytes()[start..];
        let id = self.next_id;
        self.archive.write_all(bytes)?;
        let row = serde_json::json!({"id": id, "parent": parent, "offset": self.offset,
            "append_bytes": bytes.len(), "total_bytes": text.len()});
        writeln!(self.index, "{row}")?;
        self.offset += bytes.len() as u64;
        self.next_id += 1;
        self.previous = Some((id, text.clone()));
        Ok(id)
    }
}

#[cfg(test)]
mod tests;
