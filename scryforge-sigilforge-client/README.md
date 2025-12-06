# scryforge-sigilforge-client

Client library for communicating with the [Sigilforge](https://github.com/raibid-labs/sigilforge) auth daemon.

## Overview

This crate provides a Rust client for fetching OAuth tokens from the Sigilforge daemon over Unix sockets. It's designed to be used by Scryforge providers that need to authenticate with external services.

## Features

- **SigilforgeClient** - Connect to Sigilforge daemon and fetch tokens
- **TokenFetcher trait** - Abstract interface for token retrieval
- **MockTokenFetcher** - Mock implementation for testing
- Graceful error handling when daemon is unavailable
- Configurable socket path

## Usage

### Basic Example

```rust
use scryforge_sigilforge_client::{SigilforgeClient, TokenFetcher};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to Sigilforge daemon
    let client = SigilforgeClient::with_default_path();

    if client.is_available() {
        // Fetch a token
        let token = client.fetch_token("spotify", "personal").await?;
        println!("Token: {}", token);
    } else {
        eprintln!("Sigilforge daemon not available");
    }

    Ok(())
}
```

### Provider Example

```rust
use scryforge_sigilforge_client::TokenFetcher;
use std::sync::Arc;

struct MyProvider {
    token_fetcher: Arc<dyn TokenFetcher>,
}

impl MyProvider {
    pub fn new(token_fetcher: Arc<dyn TokenFetcher>) -> Self {
        Self { token_fetcher }
    }

    pub async fn make_api_call(&self) -> Result<(), Box<dyn std::error::Error>> {
        let token = self.token_fetcher.fetch_token("spotify", "personal").await?;
        // Use token for API calls...
        Ok(())
    }
}
```

### Testing with MockTokenFetcher

```rust
use scryforge_sigilforge_client::{MockTokenFetcher, TokenFetcher};
use std::collections::HashMap;

#[tokio::test]
async fn test_provider() {
    let fetcher = MockTokenFetcher::empty()
        .with_token("spotify".into(), "personal".into(), "test_token".into());

    let token = fetcher.fetch_token("spotify", "personal").await.unwrap();
    assert_eq!(token, "test_token");
}
```

## Configuration

The default socket path is platform-dependent:

- **Unix**: `$XDG_RUNTIME_DIR/sigilforge.sock` or `/tmp/sigilforge.sock`
- **Windows**: `\\.\pipe\sigilforge`

You can override the path when creating the client:

```rust
let client = SigilforgeClient::new(PathBuf::from("/custom/path/sigilforge.sock"));
```

## Error Handling

The client provides graceful error handling:

```rust
use scryforge_sigilforge_client::{SigilforgeClient, SigilforgeError};

let client = SigilforgeClient::with_default_path();

match client.get_token("spotify", "personal").await {
    Ok(token) => println!("Token: {}", token),
    Err(SigilforgeError::Unavailable(msg)) => {
        eprintln!("Daemon not available: {}", msg);
    },
    Err(SigilforgeError::TokenNotFound { service, account }) => {
        eprintln!("Token not found for {}/{}", service, account);
    },
    Err(e) => eprintln!("Error: {}", e),
}
```

## Integration with fusabi-streams-core

Providers can use this crate through `fusabi-streams-core` by enabling the `sigilforge` feature:

```toml
[dependencies]
fusabi-streams-core = { version = "0.1", features = ["sigilforge"] }
```

Then use the `auth` module:

```rust
use fusabi_streams_core::auth::{TokenFetcher, SigilforgeClient};
```

## License

MIT OR Apache-2.0
