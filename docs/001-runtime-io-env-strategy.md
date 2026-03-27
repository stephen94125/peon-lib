# ADR 001: Runtime I/O and Configuration Strategy (Container vs. Wasm)

## 1. Background and Context

For the underlying architecture of this project (Chatbot / AI Agent), we evaluated "Traditional Containers (GCP Cloud Run / Firebase)" against "Wasm/Edge Runtimes (Cloudflare Workers / Fermyon)".

Given that the Time to First Byte (TTFB) for LLM APIs typically involves 1 to 2 seconds of network and inference latency, the hundreds of milliseconds required for a Container cold start do not constitute a performance bottleneck for this specific use case. Conversely, prematurely adopting a Wasm/Edge solution would introduce the following low-level technical constraints:

1. **CPU Time Limits**: The free tier of Cloudflare Workers enforces a strict 10ms limit on actual CPU execution time. Rust's serialization/deserialization or cryptographic operations can easily trigger `Error 1102`.
2. **Lack of Async I/O**: The standard WASI specification currently lacks support for OS-level asynchronous I/O (e.g., Linux `epoll`), preventing runtimes heavily reliant on this mechanism, like `tokio`, from functioning correctly in a Wasm environment.

Based on these objective constraints, the project currently targets **Containers (GCP Cloud Run)** for deployment. However, the codebase architecture must maintain interface isolation to ensure a low-cost migration path to a pure Wasm execution environment in the future.

---

## 2. Environment Variables Strategy

For sensitive configurations such as API Keys and database connection strings, different injection and retrieval strategies must be adopted based on the compilation target.

### 2.1 Standard Backend Environments (Container / VM / WASI)
* **Applicable Scenarios**: GCP Cloud Run, Firebase Cloud Functions, and WASI-compliant pure Wasm runtimes (e.g., Wasmtime).
* **Implementation**: All support standard OS-level environment variable injection.
* **Retrieval Method**: Rely directly on the Rust standard library.
  ```rust
  // Read dynamically at runtime
  let api_key = std::env::var("API_KEY").expect("API_KEY must be set");
  ```

### 2.2 Browser Frontend Environments (Browser Wasm)
* **Applicable Scenarios**: Compiled to `wasm32-unknown-unknown` and executed within a browser's V8 engine.
* **Constraints**: The browser sandbox lacks the concept of OS-level environment variables. Calling `std::env::var` will trigger a Panic.
* **Retrieval Method**:
  1. **Compile-time Injection**: Use the `env!` macro to hardcode variables into the binary (strictly for non-sensitive constants).
     ```rust
     let api_url = env!("API_URL");
     ```
  2. **Runtime Injection (Recommended)**: Passed in by external JavaScript during Wasm module instantiation via function arguments or shared memory.

---

## 3. File System and I/O Strategy

File read/write logic must be highly decoupled to handle the varying degrees of physical file system support across different runtimes.

### 3.1 Standard Backend Environments (Container / VM)
* **Applicable Scenarios**: GCP Cloud Run (equipped with a default `tmpfs` in-memory file system or mounted Cloud Storage FUSE).
* **Implementation**: Supports complete POSIX file systems and Linux asynchronous system calls.
* **Handling Strategy**: Allows direct use of `tokio::fs` for efficient asynchronous file I/O.
  ```rust
  use tokio::fs;
  
  async fn read_config() -> Result<Vec<u8>, std::io::Error> {
      fs::read("./config/data.json").await
  }
  ```

### 3.2 Wasm / Edge Environments (WASI / Cloudflare)
* **Applicable Scenarios**: Potential future deployments to Edge nodes.
* **Constraints**:
  * WASI lacks support for asynchronous file I/O; using `tokio::fs` will result in compilation failures or runtime errors.
  * V8 Isolates (e.g., Cloudflare Workers) do not possess a physical VFS and can only access KV or object storage via specific external APIs.
* **Handling Strategy (Stateless Refactoring)**: Deprecate all local `fs` operations. Migrate target files to external Object Storage (e.g., S3 / Cloudflare R2) and entirely switch to an HTTP Client (`reqwest`) for asynchronous network retrieval.
  ```rust
  use reqwest;

  async fn read_config_from_edge() -> Result<Vec<u8>, reqwest::Error> {
      let client = reqwest::Client::new();
      let res = client.get("[https://storage.example.com/data.json](https://storage.example.com/data.json)").send().await?;
      Ok(res.bytes().await?.to_vec())
  }
  ```

---

## 4. Conclusion

In the initial phases of the project, I/O operations will retain `tokio::fs` to maximize development velocity, while configuration files will strictly use `std::env` for retrieval. If a migration to Edge/Wasm is triggered in the future, we simply need to implement a new I/O Trait, replacing the underlying `tokio::fs` with HTTP calls via `reqwest` for a seamless transition.
