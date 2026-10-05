use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "civvis-input-capture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn capture(&self) -> InputCapture {
        InputCapture::create(&self.0.join("capture"), &self.0.join("events.jsonl")).unwrap()
    }
    fn write(&self, text: &str) {
        std::fs::write(self.0.join("events.jsonl"), text).unwrap();
    }
    fn read(&self) -> Rc<String> {
        crate::mirror::read_events(&self.0.join("events.jsonl")).unwrap()
    }
    fn rows(&self) -> Vec<serde_json::Value> {
        std::fs::read_to_string(self.0.join("capture/snapshots.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn unchanged_reads_are_reused_and_default_reads_write_nothing() {
    let fixture = Fixture::new();
    fixture.write("one\n");
    fixture.read();
    assert!(!fixture.0.join("capture").exists());
    let capture = fixture.capture();
    let request = capture.begin().unwrap();
    for _ in 0..3 {
        assert_eq!(&*fixture.read(), "one\n");
    }
    let receipt = request.finish();
    assert_eq!(
        receipt["reads"],
        serde_json::json!([{"snapshot_id":0,"count":3}])
    );
    assert_eq!(fixture.rows().len(), 1);
    assert_eq!(
        std::fs::read(fixture.0.join("capture/bytes.bin")).unwrap(),
        b"one\n"
    );
}

#[test]
fn growth_captures_each_actual_frontier_once_not_the_later_disk_tail() {
    let fixture = Fixture::new();
    let capture = fixture.capture();
    let request = capture.begin().unwrap();
    fixture.write("é\n");
    assert_eq!(&*fixture.read(), "é\n");
    fixture.write("é\nsecond\npartial");
    // The incremental reader deliberately withholds an incomplete line.
    assert_eq!(&*fixture.read(), "é\nsecond\n");
    fixture.write("é\nsecond\npartial\n");
    assert_eq!(&*fixture.read(), "é\nsecond\npartial\n");
    let receipt = request.finish();
    assert_eq!(
        receipt["reads"],
        serde_json::json!([
        {"snapshot_id":0,"count":1},{"snapshot_id":1,"count":1},{"snapshot_id":2,"count":1}])
    );
    let rows = fixture.rows();
    assert!(rows[0]["parent"].is_null());
    assert_eq!(rows[1]["parent"], 0);
    assert_eq!(rows[1]["offset"], 3);
    assert_eq!(rows[1]["append_bytes"], 7);
    assert_eq!(rows[2]["total_bytes"], 18);
    assert_eq!(
        std::fs::read(fixture.0.join("capture/bytes.bin")).unwrap(),
        "é\nsecond\npartial\n".as_bytes()
    );
}

#[test]
fn full_read_partial_unicode_bytes_are_preserved() {
    let fixture = Fixture::new();
    fixture.write("東京 partial");
    let capture = fixture.capture();
    let request = capture.begin().unwrap();
    let returned = fixture.read();
    assert_eq!(returned.as_str(), "東京 partial");
    assert_eq!(request.finish()["complete"], true);
    assert_eq!(
        std::fs::read(fixture.0.join("capture/bytes.bin")).unwrap(),
        returned.as_bytes()
    );
}

#[test]
fn replacements_and_truncation_keep_prior_roots_and_request_boundaries() {
    let fixture = Fixture::new();
    let capture = fixture.capture();
    let first = capture.begin().unwrap();
    fixture.write("old\n");
    fixture.read();
    fixture.write("new\n");
    fixture.read();
    fixture.write("x\n");
    fixture.read();
    assert_eq!(first.finish()["reads"].as_array().unwrap().len(), 3);
    let next = capture.begin().unwrap();
    fixture.read();
    assert_eq!(
        next.finish()["reads"],
        serde_json::json!([{"snapshot_id":2,"count":1}])
    );
    assert!(fixture.rows().iter().all(|row| row["parent"].is_null()));
    assert_eq!(
        std::fs::read(fixture.0.join("capture/bytes.bin")).unwrap(),
        b"old\nnew\nx\n"
    );
}

#[test]
fn existing_evidence_is_never_overwritten() {
    let fixture = Fixture::new();
    let _capture = fixture.capture();
    assert_eq!(
        InputCapture::create(&fixture.0.join("capture"), &fixture.0.join("events.jsonl"))
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::AlreadyExists
    );
    assert!(fixture.rows().is_empty());
}

#[test]
fn nesting_is_refused_and_drop_detaches_capture() {
    let fixture = Fixture::new();
    let capture = fixture.capture();
    let request = capture.begin().unwrap();
    assert!(capture.begin().is_err());
    drop(request);
    fixture.write("not captured\n");
    fixture.read();
    assert!(fixture.rows().is_empty());
    assert_eq!(
        capture.begin().unwrap().finish()["reads"],
        serde_json::json!([])
    );
}

#[test]
fn source_read_errors_are_recorded_in_order_without_fake_snapshot() {
    let fixture = Fixture::new();
    let capture = fixture.capture();
    let request = capture.begin().unwrap();
    assert!(crate::mirror::read_events(&fixture.0.join("events.jsonl")).is_err());
    fixture.write("now available\n");
    fixture.read();
    let receipt = request.finish();
    assert_eq!(receipt["complete"], true);
    assert!(receipt["reads"][0]["error"].is_string());
    assert!(receipt["reads"][0].get("snapshot_id").is_none());
    assert_eq!(receipt["reads"][1]["snapshot_id"], 0);
}

#[test]
fn archive_write_failure_is_explicit_and_latched_not_a_success() {
    let fixture = Fixture::new();
    let capture = fixture.capture();
    fixture.write("text\n");
    // Inject a read-only archive descriptor instead of relying on platform
    // permission bits (tests can run as root).
    capture.0.borrow_mut().archive = File::open(fixture.0.join("capture/bytes.bin")).unwrap();
    let request = capture.begin().unwrap();
    assert_eq!(fixture.read().as_str(), "text\n");
    let receipt = request.finish();
    assert_eq!(receipt["complete"], false);
    assert!(receipt["error"].is_string());
    assert_eq!(capture.begin().unwrap().finish()["complete"], false);
}

#[test]
fn unexpected_sources_fail_closed_for_capture_without_changing_read_result() {
    let fixture = Fixture::new();
    let capture = fixture.capture();
    let request = capture.begin().unwrap();
    let other = fixture.0.join("other.jsonl");
    std::fs::write(&other, "other\n").unwrap();
    assert_eq!(
        crate::mirror::read_events(&other).unwrap().as_str(),
        "other\n"
    );
    assert_eq!(request.finish()["complete"], false);
}
