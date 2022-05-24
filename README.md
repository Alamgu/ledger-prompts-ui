# Ledger Prompts UI

Ledger Prompts UI is a library for writing interactive prompts for ledger apps.

## Basic usage

```rust
use prompts_ui::{write_scroller, final_accept_prompt};

write_scroller(
    "Provide Public Key",
    |w| Ok(write!(w, "{}", pub_key_hash)?))?;

final_accept_prompt(&[])?;
```
