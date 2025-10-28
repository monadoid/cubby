# cubby-playground Examples

Each example is a standalone binary under `examples/` and can be executed with:

```bash
cargo run -p cubby-playground --example <name> -- <args>
```

To add your own example: drop a `my_example.rs` file in `examples/`, give it a `main` function (Tokio is already available), then run it with the command above.

## hello

Prints the playground data directory so you can confirm bootstrapping works.

```bash
cargo run -p cubby-playground --example hello
```

## list_languages

Bootstraps the playground with a custom language list and echoes the configured language codes.

```bash
cargo run -p cubby-playground --example list_languages
```

## foundation_models_concurrency

Benchmarks concurrent Apple Intelligence structured generation calls. Requires macOS 15 with Apple Intelligence enabled.

```bash
cargo run -p cubby-playground --example foundation_models_concurrency
```

## daily_live_summary

Generates a JSON day-in-review using stored live summaries for the provided date.

```bash
cargo run -p cubby-playground --example daily_live_summary -- 2025-10-25
```
