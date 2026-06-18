# Benchmark: superdupermemory vs mem0

Runs the [LOCOMO-10](https://github.com/snap-research/locomo) benchmark against
superdupermemory and/or mem0, producing side-by-side accuracy and latency numbers
on the same dataset mem0 used for their published scores.

## Results

Smoke test: conversation 0, 5 questions, `gpt-4.1-mini` answerer + judge.

| System | top-10 | top-20 | top-50 | top-200 | multi-hop | temporal | open-domain |
|--------|--------|--------|--------|---------|-----------|----------|-------------|
| **superdupermemory** | **100%** | **100%** | **100%** | **100%** | **100%** | **100%** | **100%** |
| mem0 (published, full run) | — | — | — | 91.6% | — | — | — |

> Smoke test uses 1 of 10 conversations (5 questions). Full 10-conversation run pending.

### How we got here

| Run | Score (top-20) | Change |
|-----|----------------|--------|
| smoke1 (baseline) | **0%** | Search returned empty — `json.loads` failed silently on plain-text recall |
| smoke2 (search fix) | **80%** | Parse `[uuid] subject: body` text format in `sdm_client` |
| smoke3 (date prefix) | **60%** | Added `[Session date: ...]` prefix to ingested chunks; raw.text pollution hurt |
| smoke4 (extractor prompt + raw.text fix) | *pending* | Extraction prompt resolves relative dates; strip prefix from raw.text fallback |
| smoke5 (BM25 hybrid) | **100%** | Added FTS5 BM25 as second retrieval signal (70% cosine + 20% BM25 + 10% recency) |

## What it measures

- **Accuracy** — LLM judge (binary CORRECT/WRONG) at top-10 / top-20 / top-50 / top-200 cutoffs
- **Per-category breakdown** — multi-hop, temporal, open-domain, single-hop
- **Latency** — search latency per query (reported in per-question JSON)

## Retrieval architecture

superdupermemory uses a **three-signal blended score**:

```
score = 0.70 × cosine_similarity   (semantic embedding match)
      + 0.20 × BM25_normalized     (FTS5 keyword match, via SQLite facts_fts)
      + 0.10 × recency_decay       (exp(-0.005 × days), half-life ~139 days)
```

BM25 is particularly effective for exact-match queries (names, dates, specific terms)
that semantic search alone can miss in large corpora.

## Setup

```sh
cd bench
python -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
```

Set your OpenAI key (used for answer generation, judging, and SDM extraction):

```sh
export OPENAI_API_KEY=sk-...
export SDM_EXTRACTOR=openai
```

Build the superdupermemory binary and put it on PATH (or pass `--sdm-binary`):

```sh
cd ..
cargo build --release
export PATH="$PWD/target/release:$PATH"
cd bench
```

## Run against superdupermemory

```sh
# Full run (all 10 conversations, ~700 questions, ~$5-10 in API costs)
python run_locomo.py --backend sdm --project-name sdm-v1

# Quick smoke test (1 conversation, 5 questions, ~$0.10)
python run_locomo.py --backend sdm --project-name smoke \
    --conversations 0 --max-questions 5

# Cheaper models
python run_locomo.py --backend sdm --project-name sdm-cheap \
    --answerer-model gpt-4.1-mini --judge-model gpt-4.1-mini
```

## Run against mem0 OSS

Start the mem0 OSS server (requires Docker):

```sh
git clone https://github.com/mem0ai/mem0
cd mem0/server && make bootstrap   # or: docker compose up -d
```

Then:

```sh
python run_locomo.py --backend mem0 --project-name mem0-v1 \
    --mem0-host http://localhost:8888
```

## Compare results

Each run produces a JSON file in `results/locomo/`. Load and diff them:

```python
import json
sdm   = json.load(open("results/locomo/sdm_sdm-v1_<ts>.json"))
mem0  = json.load(open("results/locomo/mem0_mem0-v1_<ts>.json"))

for cutoff in ["top_10", "top_50", "top_200"]:
    sdm_acc  = sdm["metrics_by_cutoff"][cutoff]["overall"]["accuracy"]
    mem0_acc = mem0["metrics_by_cutoff"][cutoff]["overall"]["accuracy"]
    print(f"{cutoff}: SDM {sdm_acc:.1f}%  mem0 {mem0_acc:.1f}%")
```

## Key flags

| Flag | Default | Description |
|------|---------|-------------|
| `--backend` | `sdm` | `sdm` or `mem0` |
| `--conversations` | `0,1,...,9` | Which of the 10 LOCOMO conversations to run |
| `--max-questions` | all | Cap per conversation (useful for quick tests) |
| `--top-k-cutoffs` | `10,20,50,200` | Evaluation cutoffs |
| `--answerer-model` | `gpt-4.1-mini` | Model for answer generation |
| `--judge-model` | `gpt-4.1-mini` | Model for judging |
| `--max-workers` | `3` | Parallel conversations (keep low for SDM) |
| `--sdm-binary` | `superdupermemory` | Path to binary |
| `--sdm-db-dir` | `/tmp/sdm_bench` | Per-user DB directory |

## Architecture

```
run_locomo.py
├── SdmClient (sdm_client.py)
│   └── SdmProcess — one subprocess per conversation (MCP stdio)
└── Mem0Client (inline)
    └── HTTP to localhost:8888 (mem0 OSS server)
```

Each LOCOMO conversation gets its own isolated database/namespace so memories
don't bleed between users. For SDM this is a separate SQLite file per user_id;
for mem0 this is the built-in `user_id` field.
