from __future__ import annotations

import argparse
import json
import os
import sys
import threading
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class OcrLine:
    text: str
    confidence: float
    box: list[list[float]]


def normalized_characters(text: str) -> str:
    punctuation = str.maketrans({"：": ":", "，": ",", "。": ".", "（": "(", "）": ")"})
    return "".join(character for character in text.translate(punctuation) if not character.isspace())


def edit_distance(left: str, right: str) -> int:
    previous = list(range(len(right) + 1))
    for left_index, left_character in enumerate(left, start=1):
        current = [left_index]
        for right_index, right_character in enumerate(right, start=1):
            current.append(
                min(
                    current[-1] + 1,
                    previous[right_index] + 1,
                    previous[right_index - 1] + (left_character != right_character),
                )
            )
        previous = current
    return previous[-1]


def character_accuracy(reference: str, prediction: str) -> float:
    normalized_reference = normalized_characters(reference)
    normalized_prediction = normalized_characters(prediction)
    if not normalized_reference:
        return 1.0 if not normalized_prediction else 0.0
    return max(
        0.0,
        1.0
        - edit_distance(normalized_reference, normalized_prediction)
        / len(normalized_reference),
    )


def valid_box(box: list[list[float]]) -> bool:
    return (
        len(box) == 4
        and all(len(point) == 2 for point in box)
        and all(coordinate >= 0 for point in box for coordinate in point)
    )


class PeakMemorySampler:
    def __init__(self) -> None:
        import psutil

        self.process = psutil.Process(os.getpid())
        self.peak_rss = self.process.memory_info().rss
        self.stop_event = threading.Event()
        self.thread = threading.Thread(target=self._sample, daemon=True)

    def _sample(self) -> None:
        while not self.stop_event.wait(0.01):
            self.peak_rss = max(self.peak_rss, self.process.memory_info().rss)

    def __enter__(self) -> "PeakMemorySampler":
        self.thread.start()
        return self

    def __exit__(self, *_: Any) -> None:
        self.stop_event.set()
        self.thread.join()
        self.peak_rss = max(self.peak_rss, self.process.memory_info().rss)


def load_image_samples(repo_root: Path) -> list[dict[str, Any]]:
    manifest = json.loads(
        (repo_root / "benchmarks/dataset/manifest.json").read_text(encoding="utf-8")
    )
    samples: list[dict[str, Any]] = []
    for file_name in manifest["samples"]:
        sample_path = repo_root / "benchmarks/dataset/samples" / file_name
        sample = json.loads(sample_path.read_text(encoding="utf-8"))
        if sample["input"]["kind"] == "image":
            sample["_path"] = sample_path
            samples.append(sample)
    return samples


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--candidate-id", required=True)
    parser.add_argument("--runtime-path", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    sys.path.insert(0, str(args.runtime_path.resolve()))
    from rapidocr_onnxruntime import RapidOCR

    repo_root = Path(__file__).resolve().parents[2]
    candidates = json.loads(
        (repo_root / "benchmarks/ocr/candidates.json").read_text(encoding="utf-8")
    )["candidates"]
    candidate = next(item for item in candidates if item["id"] == args.candidate_id)
    model_root = args.runtime_path / "rapidocr_onnxruntime/models"
    model_size = sum((model_root / name).stat().st_size for name in candidate["modelFiles"])

    samples = []
    with PeakMemorySampler() as memory:
        initialization_started = time.perf_counter()
        engine = RapidOCR()
        initialization_ms = (time.perf_counter() - initialization_started) * 1000
        for sample in load_image_samples(repo_root):
            image_path = (sample["_path"].parent / sample["input"]["asset"]).resolve()
            started = time.perf_counter()
            result, _ = engine(str(image_path))
            elapsed_ms = (time.perf_counter() - started) * 1000
            lines = [] if result is None else [
                OcrLine(text=item[1], confidence=float(item[2]), box=item[0])
                for item in result
            ]
            prediction = "\n".join(line.text for line in lines)
            boxes_valid = sum(valid_box(line.box) for line in lines)
            samples.append(
                {
                    "sampleId": sample["id"],
                    "characterAccuracy": round(
                        character_accuracy(sample["input"]["referenceText"], prediction), 6
                    ),
                    "coordinateValidity": round(boxes_valid / len(lines), 6) if lines else 0.0,
                    "elapsedMs": round(elapsed_ms, 3),
                    "lineCount": len(lines),
                    "meanConfidence": round(
                        sum(line.confidence for line in lines) / len(lines), 6
                    ) if lines else 0.0,
                    "lines": [asdict(line) for line in lines],
                }
            )

    result = {
        "schemaVersion": 1,
        "candidate": candidate,
        "environment": {
            "platform": sys.platform,
            "pythonVersion": sys.version.split()[0],
            "cpuCount": os.cpu_count(),
        },
        "initializationMs": round(initialization_ms, 3),
        "peakMemoryMb": round(memory.peak_rss / 1024 / 1024, 3),
        "modelSizeMb": round(model_size / 1024 / 1024, 3),
        "samples": samples,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
