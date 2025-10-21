# cubby-foundationmodels

apple foundationmodels sdk bridge for cubby - enabling on-device ai text generation with structured outputs

## ⚠️ platform requirements

**this crate ONLY works on macOS 26.0+ (sdk 26.0+)**

the apple foundationmodels framework is:
- **macOS exclusive** - not available on linux, windows, or older macOS versions
- **requires macOS 26.0+** (sequoia) - will not compile on older versions
- **requires apple intelligence models downloaded** - see setup below

## what this does

bridges rust ↔ swift to call the foundationmodels framework, enabling structured json generation from rust code with proper async handling, streaming support, and type safety.

## architecture

```
┌─────────────────────────────────────────────────────────────┐
│ rust (async)                                                 │
│  ├─ generate_person(prompt) -> Result<Value>                │
│  └─ tokio::spawn_blocking                                    │
│      ↓                                                        │
│ ┌──────────────────────────────────────────────────────┐    │
│ │ swift-bridge ffi layer                               │    │
│ │  └─ fm_generate_person(RustStr) -> String            │    │
│ └──────────────────────────────────────────────────────┘    │
│      ↓                                                        │
│ ┌──────────────────────────────────────────────────────┐    │
│ │ swift (blocking wrapper)                             │    │
│ │  ├─ converts RustStr -> String                       │    │
│ │  └─ runAsyncAndWait (semaphore pattern)              │    │
│ │      ↓                                                │    │
│ │ ┌──────────────────────────────────────────────────┐ │    │
│ │ │ swift (async)                                    │ │    │
│ │ │  ├─ SystemLanguageModel()                        │ │    │
│ │ │  ├─ LanguageModelSession(model)                  │ │    │
│ │ │  └─ respond(generating: PersonInfo.self)         │ │    │
│ │ │      ↓                                            │ │    │
│ │ │  apple foundationmodels framework                │ │    │
│ │ └──────────────────────────────────────────────────┘ │    │
│ └──────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## setup

1. **update to macOS 26.0+** (sequoia or later)

2. **check your macOS version**:
   ```bash
   cargo run -p cubby-foundationmodels --example check_version
   ```
   
   or programmatically:
   ```rust
   use cubby_foundationmodels::version::{MacOSVersion, is_macos_26_or_newer};
   
   if let Some(version) = MacOSVersion::current() {
       println!("macOS version: {}", version);
       println!("supports foundationmodels: {}", version.supports_foundationmodels());
   }
   println!("runtime meets requirement: {}", is_macos_26_or_newer());
   ```

3. **download apple intelligence models**:
   ```
   system settings → apple intelligence & siri → enable features
   ```
   models will download in background (several gb)

4. **install xcode with swift 6.0+**:
   ```bash
   xcode-select --install
   ```

5. **verify foundationmodels availability**:
   ```bash
   cd cubby-foundationmodels
   cargo test --test integration_test -- --nocapture
   ```

## usage in multi-platform cubby code

since this crate is macOS-only, you must conditionally use it in cross-platform code:

### option 1: cfg-gated imports (recommended)

```rust
// in cubby-core/src/lib.rs or cubby-server/src/main.rs

#[cfg(target_os = "macos")]
use cubby_foundationmodels::{generate, schema::PersonInfo};

pub async fn generate_text(prompt: &str) -> anyhow::Result<String> {
    #[cfg(target_os = "macos")]
    {
        // check macos version at runtime
        if is_macos_15_2_or_later() {
            let person: PersonInfo = generate(prompt).await?;
            return Ok(format!("name: {}, age: {}", person.name, person.age));
        }
    }
    
    // fallback for non-macos or older macos
    use_alternative_model(prompt).await
}

#[cfg(target_os = "macos")]
fn is_macos_15_2_or_later() -> bool {
    // check if foundationmodels framework is available
    // you can try to load the framework or check system version
    // for now, assume if it compiles, it's available
    true
}
```

### option 2: feature flags (for optional dependency)

in `cubby-core/Cargo.toml`:

```toml
[features]
default = []
macos-foundationmodels = ["cubby-foundationmodels"]

[dependencies]
cubby-foundationmodels = { path = "../cubby-foundationmodels", optional = true }
```

then in code:

```rust
#[cfg(feature = "macos-foundationmodels")]
use cubby_foundationmodels::generate;

pub async fn generate_text(prompt: &str) -> anyhow::Result<String> {
    #[cfg(feature = "macos-foundationmodels")]
    {
        let result = generate::<PersonInfo>(prompt).await?;
        return Ok(serde_json::to_string(&result)?);
    }
    
    #[cfg(not(feature = "macos-foundationmodels"))]
    {
        use_fallback_model(prompt).await
    }
}
```

build with feature on macOS:
```bash
cargo build --features macos-foundationmodels
```

### option 3: runtime check with conditional compilation

```rust
pub async fn generate_with_best_available_model(prompt: &str) -> anyhow::Result<String> {
    // on macOS 26.0+, use foundationmodels
    #[cfg(target_os = "macos")]
    {
        match cubby_foundationmodels::generate::<PersonInfo>(prompt).await {
            Ok(result) => return Ok(serde_json::to_string(&result)?),
            Err(e) => {
                eprintln!("foundationmodels unavailable: {}, falling back", e);
                // fall through to alternatives
            }
        }
    }
    
    // on linux/windows, or if macos foundationmodels fails
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        use_llama_cpp(prompt).await
    }
    
    // older macos fallback
    #[cfg(target_os = "macos")]
    {
        use_coreml_model(prompt).await
    }
}
```

## api reference

### typed api (recommended)

```rust
use cubby_foundationmodels::{generate, schema::PersonInfo};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let person: PersonInfo = generate(
        "generate info for a software engineer named alice, age 28"
    ).await?;
    
    println!("name: {}, age: {}", person.name, person.age);
    
    Ok(())
}
```

### streaming api

```rust
use cubby_foundationmodels::{generate_stream, schema::PersonInfo};
use futures::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut stream = generate_stream::<PersonInfo>("generate bob, age 35");
    
    while let Some(result) = stream.next().await {
        let person = result?;
        println!("partial: {} (age {})", person.name, person.age);
    }
    
    Ok(())
}
```

### untyped json api

```rust
use cubby_foundationmodels::generate_person;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let result = generate_person("generate charlie").await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
```

## cli examples

the crate includes a binary demonstrating all modes:

```bash
# typed generation
cargo run -- typed "software engineer alice, 28"

# streaming
cargo run -- stream "bob the designer, 35"

# untyped json
cargo run -- person "charlie"

# article summary
cargo run -- article "rust programming language"
```

## key design decisions

### 1. blocking ffi bridge (swift → rust)

**problem**: swift-bridge does not support importing async swift functions into rust (only rust async → swift is supported)

**solution**: create a synchronous swift wrapper that internally blocks on the async operation using a semaphore pattern

**trade-offs**:
- ✅ only viable approach given swift-bridge limitations
- ✅ works reliably with proper sendable constraints
- ⚠️ blocks the calling thread during execution
- ⚠️ must be called from background threads (not ui/main)

### 2. async rust api with spawn_blocking

**problem**: blocking ffi call would stall tokio runtime if called directly

**solution**: wrap ffi call in `tokio::task::spawn_blocking` to run on dedicated thread pool

**benefits**:
- ✅ tokio runtime remains responsive
- ✅ safe for concurrent async workloads
- ✅ idiomatic rust async api
- ✅ no ui blocking in applications

### 3. swift 6 concurrency safety

**problem**: swift 6 strict concurrency checking requires sendable constraints across async boundaries

**solution**: 
```swift
func runAsyncAndWait<T: Sendable>(_ operation: @Sendable @escaping () async -> T) -> T
```

**benefits**:
- ✅ compile-time data race prevention
- ✅ thread-safe by construction
- ✅ compatible with swift 6 strict mode

## project structure

```
cubby-foundationmodels/
├── Cargo.toml                 # rust manifest (macos-only, workspace member)
├── build.rs                   # codegen + swift compilation + linking
├── src/
│   ├── lib.rs                 # bridge module + async/sync/streaming apis
│   ├── schema.rs              # GenerableSchema trait + schema types
│   └── main.rs                # cli example with multiple modes
├── swift/
│   ├── Package.swift          # swift package (macos 26.0, static lib)
│   └── Sources/FoundationModelsBridge/
│       ├── bridging-header.h           # c header includes
│       ├── FoundationModelsBridge.swift # swift wrapper implementation
│       └── generated/                   # swift-bridge generated code
└── tests/
    ├── integration_test.rs    # non-streaming tests
    ├── streaming_test.rs      # streaming tests
    └── generic_schema_test.rs # generic typed api tests
```

## building

```bash
cargo build -p cubby-foundationmodels
```

the build script:
1. checks target_os == "macos" (compile_error! if not)
2. parses bridge module with swift-bridge-build
3. generates c/swift ffi glue
4. compiles swift package into static lib
5. links swift runtime + foundationmodels framework

## testing

```bash
# run all tests
cargo test -p cubby-foundationmodels

# run specific test with output
cargo test --test integration_test -- --nocapture

# run streaming tests
cargo test --test streaming_test -- --nocapture

# run generic schema tests
cargo test --test generic_schema_test -- --nocapture
```

**note**: tests will skip if foundationmodels assets are not downloaded, with a clear message.

## extending to other schemas

to add new structured output types:

1. **define rust schema** in `src/schema.rs`:
```rust
use serde::{Deserialize, Serialize};
use super::GenerableSchema;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleSummary {
    pub title: String,
    pub summary: String,
    pub key_points: Vec<String>,
}

impl GenerableSchema for ArticleSummary {
    fn schema_name() -> &'static str {
        "ArticleSummary"
    }
    
    fn from_json(value: serde_json::Value) -> anyhow::Result<Self> {
        serde_json::from_value(value).map_err(Into::into)
    }
    
    fn to_json(&self) -> anyhow::Result<serde_json::Value> {
        serde_json::to_value(self).map_err(Into::into)
    }
}
```

2. **define swift struct** in `swift/Sources/FoundationModelsBridge/FoundationModelsBridge.swift`:
```swift
@Generable
struct ArticleSummary: Codable {
    var title: String
    var summary: String
    var key_points: [String]
}
```

3. the generic `generate<T: GenerableSchema>` and `generate_stream<T>` apis will automatically work with your new schema!

## references

- [swift-bridge book](https://chinedufn.github.io/swift-bridge)
- [swift-bridge async functions](https://chinedufn.github.io/swift-bridge/bridge-module/functions/index.html#async-rust-functions)
- apple foundationmodels documentation (xcode docsets)
- [swift 6 concurrency](https://docs.swift.org/swift-book/documentation/the-swift-programming-language/concurrency/)

## known issues

- **"model assets are unavailable"**: foundationmodels requires downloading models via system settings → apple intelligence & siri
- **dyld library errors**: ensure xcode command line tools are installed and up to date
- **compile errors on non-macos**: expected! this crate is macos 26.0+ only

## performance considerations

- first call may be slow (model initialization)
- subsequent calls are faster (model stays loaded)
- each call blocks a thread pool worker during execution
- suitable for moderate workloads (not high-frequency streaming)

## license

see root license file
