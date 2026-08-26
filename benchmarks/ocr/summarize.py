from __future__ import annotations

import argparse
import json
from pathlib import Path
from statistics import mean
from typing import Any


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("inputs", nargs="+", type=Path)
    parser.add_argument("--json-output", type=Path, required=True)
    parser.add_argument("--markdown-output", type=Path, required=True)
    args = parser.parse_args()

    results = [json.loads(path.read_text(encoding="utf-8")) for path in args.inputs]
    summaries: list[dict[str, Any]] = []
    for result in results:
        samples = result["samples"]
        summaries.append(
            {
                "candidateId": result["candidate"]["id"],
                "modelFamily": result["candidate"]["modelFamily"],
                "adapterVersion": result["candidate"]["adapterVersion"],
                "meanCharacterAccuracy": round(mean(item["characterAccuracy"] for item in samples), 6),
                "coordinateValidity": round(mean(item["coordinateValidity"] for item in samples), 6),
                "meanElapsedMs": round(mean(item["elapsedMs"] for item in samples), 3),
                "maxElapsedMs": round(max(item["elapsedMs"] for item in samples), 3),
                "peakMemoryMb": result["peakMemoryMb"],
                "modelSizeMb": result["modelSizeMb"],
                "sampleCount": len(samples),
            }
        )

    ranked = sorted(
        summaries,
        key=lambda item: (
            -item["meanCharacterAccuracy"],
            -item["coordinateValidity"],
            item["meanElapsedMs"],
            item["modelSizeMb"],
        ),
    )
    report = {
        "schemaVersion": 1,
        "datasetId": "xinxiang-campus-notices-synthetic-v1",
        "metricDefinitions": {
            "characterAccuracy": "1 - Levenshtein distance / reference characters after removing whitespace",
            "coordinateValidity": "share of OCR lines containing four non-negative coordinate pairs",
            "elapsedMs": "wall-clock inference time; model initialization is excluded",
            "peakMemoryMb": "maximum process resident memory sampled every 10 ms, including model initialization",
            "modelSizeMb": "sum of detector, recognizer, and direction classifier ONNX files",
        },
        "rankingRule": "character accuracy desc, coordinate validity desc, latency asc, model size asc",
        "candidates": summaries,
        "recommendedCandidateId": ranked[0]["candidateId"],
    }
    args.json_output.parent.mkdir(parents=True, exist_ok=True)
    args.json_output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    rows = [
        "# OCR 候选模型基准",
        "",
        "本报告由 `benchmarks/ocr/run_candidate.py` 与固定合成截图生成。原始逐行识别结果位于 Git 忽略目录 `benchmarks/results/raw/`。",
        "",
        "| 候选 | 字符准确率 | 坐标有效率 | 平均耗时 | 峰值内存 | 模型体积 |",
        "| --- | ---: | ---: | ---: | ---: | ---: |",
    ]
    for item in summaries:
        rows.append(
            f"| {item['candidateId']} | {item['meanCharacterAccuracy']:.2%} | "
            f"{item['coordinateValidity']:.2%} | {item['meanElapsedMs']:.0f} ms | "
            f"{item['peakMemoryMb']:.1f} MB | {item['modelSizeMb']:.1f} MB |"
        )
    rows.extend(
        [
            "",
            "## 结论",
            "",
            f"按预先声明的排序规则，推荐 `{ranked[0]['candidateId']}`。最终锁定仍需与语义模型组合完成 8 GB、CPU-only 全集验证。",
            "",
            "## 复现",
            "",
            "1. 在仓库外建立候选 Python 目录并安装 `candidates.json` 中的固定版本。",
            "2. 分别运行 `run_candidate.py --candidate-id <id> --runtime-path <dir> --output benchmarks/results/raw/<id>.json`。",
            "3. 使用 `summarize.py` 重新生成本报告及 JSON 汇总。",
            "",
        ]
    )
    args.markdown_output.parent.mkdir(parents=True, exist_ok=True)
    args.markdown_output.write_text("\n".join(rows), encoding="utf-8")


if __name__ == "__main__":
    main()
