"""Read text positions from a screenshot with macOS's built-in Vision API.

The Civ VI leader picker is populated from installed content and therefore has
no stable row number.  Vision gives the launcher the rendered label and its
position without adding a Homebrew or Python OCR dependency.
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


class OCRUnavailable(RuntimeError):
    """Raised when the host cannot run the native text recognizer."""


_SWIFT_SOURCE = r'''
import Foundation
import Vision

let args = Array(CommandLine.arguments.dropFirst())
guard args.count == 1 else { exit(64) }

let url = URL(fileURLWithPath: args[0])
var output: [[String: Any]] = []
var recognitionError: Error? = nil
let request = VNRecognizeTextRequest { request, error in
    if let error = error {
        recognitionError = error
        return
    }
    guard let observations = request.results as? [VNRecognizedTextObservation] else {
        return
    }
    for observation in observations {
        guard let candidate = observation.topCandidates(1).first else { continue }
        let box = observation.boundingBox
        output.append([
            "text": candidate.string,
            "confidence": candidate.confidence,
            "x": box.origin.x,
            "y": 1.0 - box.origin.y - box.height,
            "width": box.width,
            "height": box.height
        ])
    }
}
request.recognitionLevel = .accurate
request.recognitionLanguages = ["en-US"]
request.usesLanguageCorrection = true

do {
    try VNImageRequestHandler(url: url, options: [:]).perform([request])
    if let error = recognitionError { throw error }
    let data = try JSONSerialization.data(withJSONObject: output)
    FileHandle.standardOutput.write(data)
} catch {
    FileHandle.standardError.write(Data(String(describing: error).utf8))
    exit(1)
}
'''.strip()

_NATIVE_BINARY: Path | None = None


def _native_binary() -> Path:
    global _NATIVE_BINARY

    if _NATIVE_BINARY and _NATIVE_BINARY.is_file():
        return _NATIVE_BINARY
    if sys.platform != "darwin":
        raise OCRUnavailable("native screenshot OCR requires macOS")
    compiler = shutil.which("swiftc")
    if not compiler:
        raise OCRUnavailable("Apple Command Line Tools (swiftc) are required")

    digest = hashlib.sha256(_SWIFT_SOURCE.encode()).hexdigest()[:16]
    cache = Path(tempfile.gettempdir()) / "civvis-ocr"
    cache.mkdir(mode=0o700, parents=True, exist_ok=True)
    binary = cache / f"vision-ocr-{digest}"
    if binary.is_file() and os.access(binary, os.X_OK):
        _NATIVE_BINARY = binary
        return binary

    source = cache / f"vision-ocr-{digest}.swift"
    source.write_text(_SWIFT_SOURCE + "\n")
    temporary = cache / f"vision-ocr-{digest}-{os.getpid()}"
    result = subprocess.run(
        [compiler, "-O", str(source), "-o", str(temporary)],
        capture_output=True,
        text=True,
        timeout=90,
    )
    if result.returncode:
        temporary.unlink(missing_ok=True)
        detail = (result.stderr or result.stdout).strip()
        raise OCRUnavailable(f"could not compile native screenshot OCR: {detail}")
    os.replace(temporary, binary)
    _NATIVE_BINARY = binary
    return binary


def recognize(path: Path) -> list[dict]:
    """Return text observations in top-left normalized image coordinates."""
    result = subprocess.run(
        [str(_native_binary()), str(path)],
        capture_output=True,
        text=True,
        timeout=45,
    )
    if result.returncode:
        raise OCRUnavailable(result.stderr.strip() or "native screenshot OCR failed")
    try:
        observations = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise OCRUnavailable(f"native screenshot OCR returned invalid JSON: {error}") from error
    if not isinstance(observations, list):
        raise OCRUnavailable("native screenshot OCR returned a non-list result")
    return [item for item in observations if isinstance(item, dict)]
