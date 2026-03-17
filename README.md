# meal-planner

An LLM-powered meal planning agent that integrates with [Mealie](https://mealie.io) for recipe management, meal plans, and shopping list generation.

## Architecture

```
┌──────────────┐     ┌───────────────┐     ┌──────────────┐
│   REPL CLI   │────▸│  Agent Loop   │────▸│  LLM Client  │
│  (rustyline)  │◂────│  (tool calls) │◂────│ (async-openai)│
└──────────────┘     └──────┬────────┘     └──────────────┘
                            │                  ▲
                   ┌────────┴────────┐         │
                   │  Tool Executor  │     local (llama.cpp)
                   └───┬────────┬───┘     or cloud (OpenAI)
                       │        │
              ┌────────┴┐  ┌───┴────────┐
              │ Mealie  │  │  SQLite    │
              │  REST   │  │  Memory    │
              │  API    │  │  Store     │
              └─────────┘  └────────────┘
```

## Features

- **Meal planning** — Proposes a week of dinners based on your Mealie recipes
- **Memory** — Tracks what was planned, what was actually cooked, and learns preferences
- **No repeats** — Configurable cooldown period to keep meals varied
- **Rating-aware** — Favours higher-rated recipes in Mealie
- **Shopping lists** — Auto-generates from the approved plan
- **Conversational** — Chat to swap meals, mark busy days, or adjust preferences
- **Local/cloud LLM** — Switch between llama.cpp and a cloud provider via config

## Setup

### Prerequisites

- Rust 1.75+ (for async trait support)
- A running Mealie instance with API access
- For local inference: llama.cpp server running with a tool-calling capable model

### Configuration

```bash
cp config/config.example.toml config.toml
# Edit config.toml with your Mealie URL, LLM settings, etc.
```

### Environment variables

```bash
export MEALIE_API_TOKEN="your-mealie-api-token"
# Only needed for cloud provider:
export OPENAI_API_KEY="your-key"
```

### Run

```bash
# Create data directory for SQLite
mkdir -p data

# Run with local LLM
cargo run

# Or specify a config path
cargo run -- path/to/config.toml
```

## Usage

```
you> Plan my meals for this week. Tuesday and Thursday are busy days.

assistant> Let me check what you've had recently and find some options...
[searches recipes, checks history]

Here's my proposal for this week:
- Monday: Chicken Tikka Masala (★4.5)
- Tuesday: 15-Min Pesto Pasta (★4.0) — quick for a busy day
- Wednesday: Beef & Broccoli Stir Fry (★4.5)
- Thursday: Sheet Pan Sausages (★4.0) — minimal prep
- Friday: Homemade Pizza (★5.0)
- Saturday: Lamb Tagine (★4.5)
- Sunday: Roast Chicken & Veg (★4.5)

Shall I go ahead and create this plan?

you> Swap Wednesday for something vegetarian

you> Looks good, go ahead

you> /reset
```

## Roadmap

- [ ] Axum web server + lightweight frontend for mobile access
- [ ] Anthropic Messages API as a native cloud backend
- [ ] Scoring model that weighs rating × cook-through × recency
- [ ] Calendar integration (auto-detect busy days)
- [ ] Nutritional balance across the week
