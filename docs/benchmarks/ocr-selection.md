# OCR 候选模型基准

本报告由 `benchmarks/ocr/run_candidate.py` 与固定合成截图生成。原始逐行识别结果位于 Git 忽略目录 `benchmarks/results/raw/`。

| 候选                    | 字符准确率 | 坐标有效率 | 平均耗时 | 峰值内存 | 模型体积 |
| ----------------------- | ---------: | ---------: | -------: | -------: | -------: |
| rapidocr-ppocrv3-mobile |    100.00% |    100.00% |   789 ms | 261.1 MB |  13.1 MB |
| rapidocr-ppocrv4-mobile |    100.00% |    100.00% |  1035 ms | 216.5 MB |  15.4 MB |

## 结论

按预先声明的排序规则，推荐 `rapidocr-ppocrv3-mobile`。最终锁定仍需与语义模型组合完成 8 GB、CPU-only 全集验证。

## 复现

1. 在仓库外建立候选 Python 目录并安装 `candidates.json` 中的固定版本。
2. 分别运行 `run_candidate.py --candidate-id <id> --runtime-path <dir> --output benchmarks/results/raw/<id>.json`。
3. 使用 `summarize.py` 重新生成本报告及 JSON 汇总。
